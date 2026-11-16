use axum::{
    Router,
    body::Body,
    extract::{Request, State},
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use chrono::{DateTime, Duration, Utc};
use http_body_util::BodyExt;
use poketeam::{
    AppState, Clock,
    config::Config,
    http,
    sync::{self, PokeApi},
};
use serde_json::{Value, json};
use sqlx::{ConnectOptions, Connection, PgPool};
use std::{
    sync::{
        Arc, Mutex,
        atomic::{AtomicI64, AtomicUsize, Ordering},
    },
    time::Duration as StdDuration,
};
use tower::ServiceExt;

struct TestClock(AtomicI64);
impl Clock for TestClock {
    fn now(&self) -> DateTime<Utc> {
        DateTime::from_timestamp(self.0.load(Ordering::SeqCst), 0).unwrap()
    }
}
impl TestClock {
    fn advance(&self, d: Duration) {
        self.0.fetch_add(d.num_seconds(), Ordering::SeqCst);
    }
}
#[derive(Clone, Default)]
struct Fixture {
    changed: bool,
    all_fields: bool,
    remove: bool,
    chart_changed: bool,
    fail: bool,
    incomplete: bool,
    sprite_redirect: bool,
    sprite_local_redirect: bool,
    default_changed: bool,
    null_sprite: bool,
    paginate: bool,
    oversized_sprite: bool,
    malformed_json: bool,
}
#[derive(Clone)]
struct MockState {
    base: String,
    fixture: Arc<Mutex<Fixture>>,
    sprite_fetches: Arc<AtomicUsize>,
    requests: Arc<AtomicUsize>,
    transient_left: Arc<AtomicUsize>,
    transient_responses: Arc<AtomicUsize>,
}
struct Harness {
    state: AppState,
    clock: Arc<TestClock>,
    mock: MockState,
    server: tokio::task::JoinHandle<()>,
}
impl Drop for Harness {
    fn drop(&mut self) {
        self.server.abort();
    }
}
impl Harness {
    async fn new(pool: PgPool) -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let mock = MockState {
            base: base.clone(),
            fixture: Arc::new(Mutex::new(Fixture::default())),
            sprite_fetches: Arc::new(AtomicUsize::new(0)),
            requests: Arc::new(AtomicUsize::new(0)),
            transient_left: Arc::new(AtomicUsize::new(0)),
            transient_responses: Arc::new(AtomicUsize::new(0)),
        };
        let app = Router::new()
            .fallback(get(mock_request))
            .with_state(mock.clone());
        let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let clock = Arc::new(TestClock(AtomicI64::new(1_800_000_000)));
        let config = Config {
            database_url: pool.connect_options().to_url_lossy().to_string(),
            listen: "127.0.0.1:0".into(),
            public_origin: "http://localhost:3000".into(),
            upstream: format!("{base}/api/v2/").parse().unwrap(),
            allowed_hosts: vec!["127.0.0.1".into()],
            allow_insecure_upstream: true,
            secure_cookie: false,
            scan_interval: StdDuration::from_secs(86400),
        };
        Self {
            state: AppState {
                pool,
                config,
                clock: clock.clone(),
            },
            clock,
            mock,
            server,
        }
    }
    async fn sync(&self) -> anyhow::Result<sync::SyncOutcome> {
        sync::run(&self.state, &PokeApi::new(self.state.config.clone())?).await
    }
    async fn request(
        &self,
        method: &str,
        path: &str,
        cookie: Option<&str>,
        body: Value,
    ) -> (StatusCode, axum::http::HeaderMap, Value) {
        let mut builder = axum::http::Request::builder()
            .method(method)
            .uri(path)
            .header(header::ORIGIN, "http://localhost:3000")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(cookie) = cookie {
            builder = builder.header(header::COOKIE, cookie);
        }
        let response = http::router(self.state.clone())
            .oneshot(builder.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
        (status, headers, value)
    }
    async fn owner(&self) -> String {
        let (status, headers, _) = self
            .request("POST", "/api/v1/session", None, json!({}))
            .await;
        assert_eq!(status, StatusCode::OK);
        headers[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .into()
    }
    async fn team(&self, cookie: &str, ids: &[i32]) -> Value {
        let (status, _, team) = self
            .request(
                "POST",
                "/api/v1/teams",
                Some(cookie),
                json!({"name":" My team ","pokemon_ids":ids}),
            )
            .await;
        assert_eq!(status, StatusCode::CREATED, "{team}");
        team
    }
    async fn notifications(&self, cookie: &str) -> Value {
        let (status, _, v) = self
            .request("GET", "/api/v1/notifications", Some(cookie), json!(null))
            .await;
        assert_eq!(status, StatusCode::OK, "{v}");
        v
    }
}
fn resource(base: &str, kind: &str, id: i32, name: &str) -> Value {
    json!({"name":name,"url":format!("{base}/api/v2/{kind}/{id}/")})
}
const TYPES: [&str; 8] = [
    "fire", "water", "grass", "electric", "normal", "ground", "ghost", "flying",
];
async fn mock_request(State(mock): State<MockState>, request: Request) -> Response {
    mock.requests.fetch_add(1, Ordering::SeqCst);
    let f = mock.fixture.lock().unwrap().clone();
    let path = request.uri().path();
    let base = &mock.base;
    if path.starts_with("/sprites/") {
        mock.sprite_fetches.fetch_add(1, Ordering::SeqCst);
        if f.oversized_sprite {
            return (
                [(header::CONTENT_TYPE, "image/png")],
                vec![0u8; 4 * 1024 * 1024 + 1],
            )
                .into_response();
        }
        if f.sprite_local_redirect && path.ends_with("1.png") {
            return (
                StatusCode::FOUND,
                [(header::LOCATION, "/sprites/allowed.png")],
            )
                .into_response();
        }
        if f.sprite_redirect {
            return (
                StatusCode::FOUND,
                [(header::LOCATION, "http://unapproved.invalid/sprite.png")],
            )
                .into_response();
        }
        let bytes = if f.changed {
            include_bytes!("fixtures/changed.png").as_slice()
        } else {
            include_bytes!("fixtures/sprite.png").as_slice()
        };
        return ([(header::CONTENT_TYPE, "image/png")], bytes).into_response();
    }
    let pieces: Vec<_> = path.trim_matches('/').split('/').collect();
    if pieces.len() < 3 {
        return StatusCode::NOT_FOUND.into_response();
    }
    let kind = pieces[2];
    let id = pieces.get(3).and_then(|s| s.parse::<i32>().ok());
    let value = match (kind, id) {
        ("pokemon-species", None) => {
            let n = if f.remove { 7 } else { 8 };
            let page_two = request.uri().query().is_some_and(|q| q.contains("page=2"));
            let start = if f.paginate && page_two { 5 } else { 1 };
            let end = if f.paginate && !page_two { 4 } else { n };
            let results: Vec<_> = (start..=end)
                .map(|i| resource(base, "pokemon-species", i, &format!("species{i}")))
                .collect();
            json!({"count":if f.incomplete {n+1}else{n},"next":if f.paginate && !page_two {Some(format!("{base}/api/v2/pokemon-species?page=2"))} else {None},"results":results})
        }
        ("pokemon-species", Some(i)) => {
            json!({"id":i,"varieties":[{"is_default":false,"pokemon":resource(base,"pokemon",9999,"alternate")},{"is_default":true,"pokemon":resource(base,"pokemon",if i==1 {if f.default_changed {101}else{100}}else{i},&format!("p{i}"))}]})
        }
        ("pokemon", Some(id)) => {
            if f.malformed_json && id == 3 {
                return ([(header::CONTENT_TYPE, "application/json")], "{").into_response();
            }
            if id == 3
                && mock
                    .transient_left
                    .try_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                    .is_ok()
            {
                mock.transient_responses.fetch_add(1, Ordering::SeqCst);
                return (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, "0")])
                    .into_response();
            }
            if f.fail && id == 3 {
                return (
                    StatusCode::SERVICE_UNAVAILABLE,
                    [(header::RETRY_AFTER, "0")],
                )
                    .into_response();
            }
            let species = if id == 100 || id == 101 { 1 } else { id };
            let name = if f.all_fields && id == 100 {
                "renamed".into()
            } else {
                format!("p{species}")
            };
            let stats:Vec<_>=["hp","attack","defense","special-attack","special-defense","speed"].into_iter().enumerate().map(|(i,name)|json!({"base_stat":50+species+if f.changed && id==100 && i==0 {5}else{0},"stat":{"name":name,"url":format!("{base}/stat/{i}")}})).collect();
            let t = if f.all_fields && id == 100 {
                "water"
            } else {
                TYPES[(species - 1) as usize]
            };
            json!({"id":id,"name":name,"is_default":true,"species":resource(base,"pokemon-species",species,&format!("species{species}")),"types":[{"slot":1,"type":{"name":t,"url":format!("{base}/type/{t}")}}],"stats":stats,"sprites":{"front_default":if species==2 || (f.null_sprite && species==1) {None}else{Some(format!("{base}/sprites/{}1.png",if f.all_fields {"new"}else{""}))}}})
        }
        ("type", None) => {
            let page_two = request
                .uri()
                .query()
                .is_some_and(|query| query.contains("page=2"));
            let start = if f.paginate && page_two { 4 } else { 0 };
            let end = if f.paginate && !page_two {
                4
            } else {
                TYPES.len()
            };
            let results: Vec<_> = TYPES
                .iter()
                .enumerate()
                .skip(start)
                .take(end - start)
                .map(|(index, name)| resource(base, "type", index as i32 + 1, name))
                .collect();
            json!({"count":8,"next":if f.paginate && !page_two {Some(format!("{base}/api/v2/type?page=2"))} else {None},"results":results})
        }
        ("type", Some(i)) => {
            let name = TYPES[(i - 1) as usize];
            let strong = match name {
                "fire" => vec!["grass"],
                "water" => vec!["fire", "ground"],
                "grass" => vec!["water", "ground"],
                "electric" => vec!["water", "flying"],
                "ground" => vec!["fire", "electric"],
                "ghost" => vec!["ghost"],
                _ => vec![],
            };
            let strong = if f.chart_changed && name == "fire" {
                vec!["grass", "electric"]
            } else {
                strong
            };
            let no = match name {
                "electric" => vec!["ground"],
                "normal" => vec!["ghost"],
                "ghost" => vec!["normal"],
                "ground" => vec!["flying"],
                _ => vec![],
            };
            let r = |names: Vec<&str>| {
                names
                    .into_iter()
                    .map(|name| json!({"name":name,"url":format!("{base}/type/{name}")}))
                    .collect::<Vec<_>>()
            };
            json!({"name":name,"damage_relations":{"double_damage_to":r(strong),"half_damage_to":[],"no_damage_to":r(no)}})
        }
        _ => return StatusCode::NOT_FOUND.into_response(),
    };
    axum::Json(value).into_response()
}

#[sqlx::test(migrations = "./migrations")]
async fn catalog_and_team_contract(pool: PgPool) {
    let h = Harness::new(pool).await;
    assert_eq!(
        h.request("GET", "/health/live", None, json!(null)).await.0,
        StatusCode::OK
    );
    assert_eq!(
        h.request("GET", "/health/ready", None, json!(null)).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    h.sync().await.unwrap();
    assert_eq!(
        h.request("GET", "/health/ready", None, json!(null)).await.0,
        StatusCode::OK
    );
    let (status, headers, page) = h
        .request("GET", "/api/v1/pokemon?limit=2&offset=0", None, json!(null))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["total"], 8);
    assert_eq!(page["pokemon"][0]["id"], 2);
    assert_eq!(page["pokemon"][0]["sprite_url"], Value::Null);
    assert_eq!(page["pokemon"][1]["id"], 3);
    assert!(headers.contains_key(header::ETAG));
    let request = Request::builder()
        .uri("/api/v1/pokemon?limit=2&offset=0")
        .header(header::IF_NONE_MATCH, headers[header::ETAG].clone())
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        http::router(h.state.clone())
            .oneshot(request)
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_MODIFIED
    );
    let (_, _, page) = h
        .request(
            "GET",
            "/api/v1/pokemon?search=P1&type=fire",
            None,
            json!(null),
        )
        .await;
    assert_eq!(page["total"], 1);
    assert_eq!(page["pokemon"][0]["species_id"], 1);
    assert_eq!(page["pokemon"][0]["id"], 100);
    assert_eq!(
        h.request("GET", "/api/v1/pokemon?limit=201", None, json!(null))
            .await
            .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    assert_eq!(
        h.request("GET", "/api/v1/pokemon?limit=bad", None, json!(null))
            .await
            .2["code"],
        "invalid_query"
    );
    assert_eq!(
        h.request("GET", "/api/v1/pokemon/no", None, json!(null))
            .await
            .2["code"],
        "invalid_path"
    );
    assert_eq!(
        h.request("GET", "/api/v1/teams", None, json!(null)).await.0,
        StatusCode::UNAUTHORIZED
    );
    let cookie = h.owner().await;
    let other = h.owner().await;
    let (_, headers, session) = h
        .request("POST", "/api/v1/session", Some(&cookie), json!({}))
        .await;
    let (_, _, again) = h
        .request("POST", "/api/v1/session", Some(&cookie), json!({}))
        .await;
    assert_eq!(session, again);
    assert!(
        headers[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .contains("HttpOnly; SameSite=Lax")
    );
    let empty = h.team(&cookie, &[]).await;
    let created = h.team(&cookie, &[100, 3, 2]).await;
    assert_eq!(created["name"], "My team");
    assert_eq!(created["pokemon"][0]["id"], 100);
    let path = format!("/api/v1/teams/{}", created["id"].as_str().unwrap());
    assert_eq!(
        h.request("GET", &path, Some(&other), json!(null)).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        h.request("DELETE", &path, Some(&other), json!({})).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        h.request(
            "PUT",
            &path,
            Some(&other),
            json!({"name":"stolen","pokemon_ids":[],"revision":1})
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    for ids in [vec![2, 2], vec![1], vec![2, 3, 4, 5, 6, 7, 8]] {
        assert_eq!(
            h.request(
                "POST",
                "/api/v1/teams",
                Some(&cookie),
                json!({"name":"x","pokemon_ids":ids})
            )
            .await
            .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    for name in [" ".into(), "x".repeat(101)] {
        assert_eq!(
            h.request("POST", "/api/v1/teams", Some(&cookie), json!({"name":name}))
                .await
                .0,
            StatusCode::UNPROCESSABLE_ENTITY
        );
    }
    let (_, _, updated) = h
        .request(
            "PUT",
            &path,
            Some(&cookie),
            json!({"name":"Reordered","pokemon_ids":[2,100],"revision":1}),
        )
        .await;
    assert_eq!(updated["revision"], 2);
    assert_eq!(updated["pokemon"][0]["id"], 2);
    assert_eq!(
        h.request(
            "PUT",
            &path,
            Some(&cookie),
            json!({"name":"stale","pokemon_ids":[],"revision":1})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (_, _, loaded) = h.request("GET", &path, Some(&cookie), json!(null)).await;
    assert_eq!(loaded, updated);
    let (_, _, list) = h
        .request("GET", "/api/v1/teams", Some(&cookie), json!(null))
        .await;
    assert_eq!(list.as_array().unwrap().len(), 2);
    let counter_path = format!("{path}/counter-team");
    let (_, _, power) = h
        .request("POST", &counter_path, Some(&cookie), json!({}))
        .await;
    assert_eq!(power["mode"], "power");
    assert_eq!(power["pokemon"].as_array().unwrap().len(), 2);
    let (_, _, rival) = h
        .request(
            "POST",
            &counter_path,
            Some(&cookie),
            json!({"mode":"rival"}),
        )
        .await;
    assert_eq!(rival["mode"], "rival");
    assert!(rival["percentage_stat_difference"].as_f64().unwrap().abs() <= 5.0);
    assert_eq!(
        h.request(
            "POST",
            &format!(
                "/api/v1/teams/{}/counter-team",
                empty["id"].as_str().unwrap()
            ),
            Some(&cookie),
            json!({})
        )
        .await
        .2["code"],
        "empty_source_team"
    );
    assert_eq!(
        h.request(
            "POST",
            &counter_path,
            Some(&cookie),
            json!({"mode":"unknown"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        h.request("DELETE", &path, Some(&cookie), json!({})).await.0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        h.request("GET", &path, Some(&cookie), json!(null)).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, _, doc) = h.request("GET", "/openapi.json", None, json!(null)).await;
    assert_eq!(doc["paths"].as_object().unwrap().len(), 10);
    assert!(doc["components"]["schemas"]["CounterResult"].is_object());
}

#[sqlx::test(migrations = "./migrations")]
async fn change_notification_demo(pool: PgPool) {
    let h = Harness::new(pool).await;
    h.sync().await.unwrap();
    let owner = h.owner().await;
    let team = h.team(&owner, &[100]).await;
    let second = h.team(&owner, &[100]).await;
    assert_eq!(h.notifications(&owner).await["total"], 0);
    let (_, _, before) = h
        .request("GET", "/api/v1/pokemon/100", None, json!(null))
        .await;
    let old_sprite = before["sprite_url"].as_str().unwrap().to_owned();
    let (_, headers, _) = h.request("GET", &old_sprite, None, json!(null)).await;
    assert!(
        headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("immutable")
    );
    println!(
        "Baseline: {}",
        json!({"team_id":team["id"],"pokemon":before,"notifications":0})
    );
    h.clock.advance(Duration::hours(1));
    h.mock.fixture.lock().unwrap().changed = true;
    assert_eq!(h.sync().await.unwrap().change_count, 7); // All seven non-null sprites changed at the same URL.
    let changes = h.notifications(&owner).await;
    assert_eq!(changes["total"], 1);
    let event = &changes["notifications"][0];
    assert_eq!(event["affected_team_ids"].as_array().unwrap().len(), 2);
    assert_ne!(event["pokemon"]["sprite_url"], old_sprite);
    assert!(event["differences"]["stats"].is_object());
    assert!(event["differences"]["sprite_hash"].is_object());
    assert!(event["differences"].get("sprite_source").is_none());
    println!(
        "After normal sync (same upstream sprite URL): {}",
        serde_json::to_string_pretty(&changes).unwrap()
    );
    let old = http::router(h.state.clone())
        .oneshot(
            Request::builder()
                .uri(&old_sprite)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    let new = http::router(h.state.clone())
        .oneshot(
            Request::builder()
                .uri(event["pokemon"]["sprite_url"].as_str().unwrap())
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    assert_ne!(old, new);
    assert_eq!(h.sync().await.unwrap().change_count, 0);
    assert_eq!(h.notifications(&owner).await["total"], 1);
    assert_eq!(h.mock.sprite_fetches.load(Ordering::SeqCst), 21);
    println!("Repeated scan: 0 new changes; 1 deduplicated notification across 2 current teams.");
    h.clock.advance(Duration::days(7));
    assert_eq!(h.notifications(&owner).await["total"], 1); // Inclusive seven-day boundary.
    h.clock.advance(Duration::seconds(1));
    assert_eq!(h.notifications(&owner).await["total"], 0);
    println!("Injected clock advanced seven days + one second: 0 notifications.");
    h.clock.advance(-Duration::days(7));
    for t in [&team, &second] {
        let path = format!("/api/v1/teams/{}", t["id"].as_str().unwrap());
        h.request(
            "PUT",
            &path,
            Some(&owner),
            json!({"name":"empty","pokemon_ids":[],"revision":1}),
        )
        .await;
    }
    assert_eq!(h.notifications(&owner).await["total"], 0); // History follows current membership.
}

#[sqlx::test(migrations = "./migrations")]
async fn consumed_changes_availability_and_retention(pool: PgPool) {
    let h = Harness::new(pool).await;
    h.sync().await.unwrap();
    let owner = h.owner().await;
    let team = h.team(&owner, &[100, 8]).await;
    h.clock.advance(Duration::hours(1));
    *h.mock.fixture.lock().unwrap() = Fixture {
        changed: true,
        all_fields: true,
        remove: true,
        chart_changed: true,
        ..Fixture::default()
    };
    h.sync().await.unwrap();
    let changes = h.notifications(&owner).await;
    assert_eq!(changes["total"], 2);
    let events = changes["notifications"].as_array().unwrap();
    let first = events.iter().find(|e| e["pokemon"]["id"] == 100).unwrap();
    for key in [
        "name",
        "types",
        "stats",
        "sprite_source",
        "sprite_hash",
        "type_effectiveness",
    ] {
        assert!(!first["differences"][key].is_null(), "missing {key}");
    }
    assert!(
        !events.iter().find(|e| e["pokemon"]["id"] == 8).unwrap()["pokemon"]["active"]
            .as_bool()
            .unwrap()
    );
    assert_eq!(
        h.request(
            "POST",
            "/api/v1/teams",
            Some(&owner),
            json!({"name":"inactive","pokemon_ids":[8]})
        )
        .await
        .0,
        StatusCode::UNPROCESSABLE_ENTITY
    );
    let path = format!("/api/v1/teams/{}", team["id"].as_str().unwrap());
    assert_eq!(
        h.request(
            "PUT",
            &path,
            Some(&owner),
            json!({"name":"retained","pokemon_ids":[8,100],"revision":1})
        )
        .await
        .0,
        StatusCode::OK
    );
    let (_, _, page) = h.request("GET", "/api/v1/pokemon", None, json!(null)).await;
    assert_eq!(page["total"], 7);
    assert_eq!(h.sync().await.unwrap().change_count, 0);
    let (status, _, rival) = h
        .request(
            "POST",
            &format!("{path}/counter-team"),
            Some(&owner),
            json!({"mode":"rival"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{rival}");
    assert!(
        rival["pokemon"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["active"] == true)
    );
    let (_, _, page) = h
        .request(
            "GET",
            "/api/v1/notifications?limit=1&offset=1",
            Some(&owner),
            json!(null),
        )
        .await;
    assert_eq!(page["total"], 2);
    assert_eq!(page["notifications"].as_array().unwrap().len(), 1);
    // Every retained event is older than 30 days; normal publication prunes history.
    h.clock.advance(Duration::days(31));
    h.sync().await.unwrap();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM pokemon_changes")
        .fetch_one(&h.state.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn rollback_locking_scheduling_and_upstream_policy(pool: PgPool) {
    let h = Harness::new(pool).await;
    assert!(sync::next_delay(&h.state).await.unwrap().is_zero());
    h.sync().await.unwrap();
    assert_eq!(
        sync::next_delay(&h.state).await.unwrap(),
        StdDuration::from_secs(86400)
    );
    let (_, _, before) = h
        .request("GET", "/api/v1/pokemon/100", None, json!(null))
        .await;
    h.clock.advance(Duration::hours(1));
    h.mock.fixture.lock().unwrap().fail = true;
    assert!(h.sync().await.is_err());
    assert_eq!(
        sync::next_delay(&h.state).await.unwrap(),
        StdDuration::from_secs(900)
    );
    assert_eq!(
        h.request("GET", "/api/v1/pokemon/100", None, json!(null))
            .await
            .2,
        before
    );
    assert_eq!(
        h.request("GET", "/health/ready", None, json!(null)).await.0,
        StatusCode::OK
    );
    h.mock.fixture.lock().unwrap().fail = false;
    h.mock.fixture.lock().unwrap().incomplete = true;
    assert!(h.sync().await.is_err());
    h.mock.fixture.lock().unwrap().incomplete = false;
    h.mock.fixture.lock().unwrap().sprite_local_redirect = true;
    h.sync().await.unwrap(); // Reqwest follows an allowed relative redirect.
    h.mock.fixture.lock().unwrap().sprite_local_redirect = false;
    h.mock.fixture.lock().unwrap().sprite_redirect = true;
    assert!(h.sync().await.is_err());
    h.mock.fixture.lock().unwrap().sprite_redirect = false;
    let mut conn = sqlx::PgConnection::connect(&h.state.config.database_url)
        .await
        .unwrap();
    sqlx::query("SELECT pg_advisory_lock(731945110)")
        .execute(&mut conn)
        .await
        .unwrap();
    let requests = h.mock.requests.load(Ordering::SeqCst);
    assert!(h.sync().await.unwrap().skipped);
    assert_eq!(requests, h.mock.requests.load(Ordering::SeqCst));
    conn.close().await.unwrap();
    // A failure on the first Pokémon rolls back that row and its new sprite asset.
    sqlx::query("ALTER TABLE pokemon ADD CONSTRAINT reject_renamed CHECK(name <> 'renamed')")
        .execute(&h.state.pool)
        .await
        .unwrap();
    h.mock.fixture.lock().unwrap().all_fields = true;
    h.mock.fixture.lock().unwrap().changed = true;
    assert!(h.sync().await.is_err());
    assert_eq!(
        h.request("GET", "/api/v1/pokemon/100", None, json!(null))
            .await
            .2,
        before
    );
    let asset_count: i64 = sqlx::query_scalar("SELECT count(*) FROM sprite_assets")
        .fetch_one(&h.state.pool)
        .await
        .unwrap();
    assert_eq!(asset_count, 1);
    sqlx::query("ALTER TABLE pokemon DROP CONSTRAINT reject_renamed")
        .execute(&h.state.pool)
        .await
        .unwrap();
    h.sync().await.unwrap();
    let (_, _, status) = h
        .request("GET", "/api/v1/catalog/status", None, json!(null))
        .await;
    assert_eq!(status["latest_run"]["outcome"], "success");
    h.clock.advance(Duration::days(2));
    assert!(sync::next_delay(&h.state).await.unwrap().is_zero());
    let mut blocked = h.state.config.clone();
    blocked.allowed_hosts = vec!["example.invalid".into()];
    assert!(
        sync::run(&h.state, &PokeApi::new(blocked).unwrap())
            .await
            .is_err()
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn mutation_security_and_json_errors(pool: PgPool) {
    let h = Harness::new(pool).await;
    for (origin, content_type, body, code) in [
        (
            "https://evil.example",
            "application/json",
            "{}",
            "invalid_origin",
        ),
        ("http://localhost:3000", "text/plain", "{}", "json_required"),
        (
            "http://localhost:3000",
            "application/json",
            "{",
            "invalid_json",
        ),
    ] {
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/session")
            .header(header::ORIGIN, origin)
            .header(header::CONTENT_TYPE, content_type)
            .body(Body::from(body))
            .unwrap();
        let response = http::router(h.state.clone())
            .oneshot(request)
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            serde_json::from_slice::<Value>(&bytes).unwrap()["code"],
            code
        );
    }
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/session")
        .header(header::ORIGIN, "http://localhost:3000")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("x".repeat(17000)))
        .unwrap();
    let response = http::router(h.state.clone())
        .oneshot(request)
        .await
        .unwrap();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap()["code"],
        "invalid_json"
    );
    let mut secure = h.state.clone();
    secure.config.secure_cookie = true;
    let request = Request::builder()
        .method("POST")
        .uri("/api/v1/session")
        .header(header::ORIGIN, "http://localhost:3000")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from("{}"))
        .unwrap();
    let response = http::router(secure).oneshot(request).await.unwrap();
    assert!(
        response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .ends_with("; Secure")
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn chart_only_changes_null_sprites_and_default_replacement(pool: PgPool) {
    let h = Harness::new(pool).await;
    h.sync().await.unwrap();
    let owner = h.owner().await;
    let team = h.team(&owner, &[100, 4]).await;
    h.clock.advance(Duration::minutes(1));
    h.mock.fixture.lock().unwrap().chart_changed = true;
    let result = h.sync().await.unwrap();
    assert_eq!(result.change_count, 2); // Only fire and electric participate in the changed relation.
    let events = h.notifications(&owner).await;
    for event in events["notifications"].as_array().unwrap() {
        assert_eq!(event["differences"].as_object().unwrap().len(), 1);
        assert_eq!(
            event["differences"]["type_effectiveness"][0]["attacking"],
            "fire"
        );
        assert_eq!(
            event["differences"]["type_effectiveness"][0]["defending"],
            "electric"
        );
    }
    h.mock.fixture.lock().unwrap().null_sprite = true;
    assert_eq!(h.sync().await.unwrap().change_count, 1);
    let (_, _, p) = h
        .request("GET", "/api/v1/pokemon/100", None, json!(null))
        .await;
    assert!(p["sprite_url"].is_null());
    h.mock.fixture.lock().unwrap().default_changed = true;
    h.sync().await.unwrap();
    let (_, _, page) = h.request("GET", "/api/v1/pokemon", None, json!(null)).await;
    assert_eq!(page["total"], 8);
    assert_eq!(
        page["pokemon"].as_array().unwrap().last().unwrap()["id"],
        101
    );
    let (_, _, old) = h
        .request("GET", "/api/v1/pokemon/100", None, json!(null))
        .await;
    assert_eq!(old["active"], false); // Previous default remains referencable by teams/history.
    let (_, _, team) = h
        .request(
            "GET",
            &format!("/api/v1/teams/{}", team["id"].as_str().unwrap()),
            Some(&owner),
            json!(null),
        )
        .await;
    assert_eq!(team["pokemon"][0]["id"], 100);
    assert_eq!(team["pokemon"][0]["active"], false);
}

#[sqlx::test(migrations = "./migrations")]
async fn pagination_failure_size_limits_and_restart_recovery(pool: PgPool) {
    let h = Harness::new(pool).await;
    h.mock.fixture.lock().unwrap().paginate = true;
    assert_eq!(h.sync().await.unwrap().pokemon_count, 8);
    let relationships: i64 = sqlx::query_scalar("SELECT count(*) FROM type_effectiveness")
        .fetch_one(&h.state.pool)
        .await
        .unwrap();
    assert_eq!(relationships, 64); // Both species and type discovery followed page two.
    h.mock.transient_left.store(2, Ordering::SeqCst);
    assert!(h.sync().await.is_err());
    assert_eq!(h.mock.transient_responses.load(Ordering::SeqCst), 1); // Stop on the first failed request.
    h.mock.transient_left.store(0, Ordering::SeqCst);
    h.mock.fixture.lock().unwrap().oversized_sprite = true;
    assert!(h.sync().await.is_err());
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM catalog_state")
        .fetch_one(&h.state.pool)
        .await
        .unwrap();
    assert_eq!(revision, 8);
    h.mock.fixture.lock().unwrap().oversized_sprite = false;
    // Simulate a process crash after recording the beginning of a scan.
    let crashed = uuid::Uuid::new_v4();
    sqlx::query("INSERT INTO sync_runs(id,started_at,outcome) VALUES($1,$2,'running')")
        .bind(crashed)
        .bind(h.clock.now())
        .execute(&h.state.pool)
        .await
        .unwrap();
    assert!(sync::next_delay(&h.state).await.unwrap().is_zero());
    h.sync().await.unwrap();
    let outcome: String = sqlx::query_scalar("SELECT outcome FROM sync_runs WHERE id=$1")
        .bind(crashed)
        .fetch_one(&h.state.pool)
        .await
        .unwrap();
    assert_eq!(outcome, "abandoned");
    assert_eq!(
        sync::next_delay(&h.state).await.unwrap(),
        StdDuration::from_secs(86400)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn concurrent_updates_expired_owners_and_rival_failure(pool: PgPool) {
    let h = Harness::new(pool).await;
    h.sync().await.unwrap();
    let owner = h.owner().await;
    let team = h.team(&owner, &[100]).await;
    let path = format!("/api/v1/teams/{}", team["id"].as_str().unwrap());
    let a = h.request(
        "PUT",
        &path,
        Some(&owner),
        json!({"name":"first","pokemon_ids":[2,3],"revision":1}),
    );
    let b = h.request(
        "PUT",
        &path,
        Some(&owner),
        json!({"name":"second","pokemon_ids":[4,5],"revision":1}),
    );
    let (a, b) = tokio::join!(a, b);
    assert!(
        (a.0 == StatusCode::OK && b.0 == StatusCode::CONFLICT)
            || (b.0 == StatusCode::OK && a.0 == StatusCode::CONFLICT)
    );
    let (_, _, current) = h.request("GET", &path, Some(&owner), json!(null)).await;
    let winning = if a.0 == StatusCode::OK { a.2 } else { b.2 };
    assert_eq!(current, winning); // Membership is never an interleaving of both updates.
    // A very weak unavailable source has no qualifying deterministic replacement seed.
    let team = h.team(&owner, &[8]).await;
    h.mock.fixture.lock().unwrap().remove = true;
    h.sync().await.unwrap();
    sqlx::query("UPDATE pokemon SET stats=$1 WHERE id=8")
        .bind(
            json!({"hp":1,"attack":1,"defense":1,"special_attack":1,"special_defense":1,"speed":1}),
        )
        .execute(&h.state.pool)
        .await
        .unwrap();
    let (status, _, error) = h
        .request(
            "POST",
            &format!(
                "/api/v1/teams/{}/counter-team",
                team["id"].as_str().unwrap()
            ),
            Some(&owner),
            json!({"mode":"rival"}),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["code"], "rival_balance_not_found");
    assert!(
        error["message"]
            .as_str()
            .unwrap()
            .contains("may still exist")
    );
    let hashes: Vec<String> = sqlx::query_scalar("SELECT token_hash FROM owners")
        .fetch_all(&h.state.pool)
        .await
        .unwrap();
    assert!(hashes.iter().all(|hash| !owner.contains(hash))); // The raw cookie is never stored.
    h.clock.advance(Duration::days(366));
    assert_eq!(
        h.request("GET", "/api/v1/teams", Some(&owner), json!(null))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    let (_, headers, _) = h
        .request("POST", "/api/v1/session", Some(&owner), json!({}))
        .await;
    assert!(
        !headers[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .starts_with(&owner)
    );
}

#[sqlx::test(migrations = "./migrations")]
async fn partial_initial_import_is_visible_and_finishes_silently(pool: PgPool) {
    let h = Harness::new(pool).await;
    h.mock.fixture.lock().unwrap().fail = true;
    assert!(h.sync().await.is_err());
    let (status, _, page) = h.request("GET", "/api/v1/pokemon", None, json!(null)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["total"], 2);
    assert_eq!(page["catalog_revision"], 2);
    assert_eq!(
        h.request("GET", "/health/ready", None, json!(null)).await.0,
        StatusCode::OK
    );
    let (_, _, status) = h
        .request("GET", "/api/v1/catalog/status", None, json!(null))
        .await;
    assert!(status["last_success_at"].is_null());
    assert_eq!(status["latest_run"]["outcome"], "failed");
    assert_eq!(status["latest_run"]["pokemon_count"], 2);
    assert_eq!(status["latest_run"]["change_count"], 0);
    let owner = h.owner().await;
    h.team(&owner, &[100]).await;
    assert_eq!(h.notifications(&owner).await["total"], 0);
    h.mock.fixture.lock().unwrap().fail = false;
    assert_eq!(h.sync().await.unwrap().pokemon_count, 8);
    let (_, _, status) = h
        .request("GET", "/api/v1/catalog/status", None, json!(null))
        .await;
    assert!(!status["last_success_at"].is_null());
    assert_eq!(status["revision"], 8);
    assert_eq!(status["latest_run"]["change_count"], 0);
    assert_eq!(h.notifications(&owner).await["total"], 0);
    h.clock.advance(Duration::hours(1));
    h.mock.fixture.lock().unwrap().changed = true;
    assert_eq!(h.sync().await.unwrap().change_count, 7);
    assert_eq!(h.notifications(&owner).await["total"], 1);
}

#[sqlx::test(migrations = "./migrations")]
async fn failed_refresh_retains_commits_alerts_and_unvisited_members(pool: PgPool) {
    let h = Harness::new(pool).await;
    h.sync().await.unwrap();
    let owner = h.owner().await;
    h.team(&owner, &[100, 8]).await;
    let (_, old_headers, before) = h
        .request("GET", "/api/v1/pokemon/100", None, json!(null))
        .await;
    h.clock.advance(Duration::hours(1));
    *h.mock.fixture.lock().unwrap() = Fixture {
        changed: true,
        remove: true,
        fail: true,
        ..Fixture::default()
    };
    assert!(h.sync().await.is_err());
    let (_, headers, after) = h
        .request("GET", "/api/v1/pokemon/100", None, json!(null))
        .await;
    assert_ne!(before["stats"], after["stats"]);
    assert_ne!(before["sprite_url"], after["sprite_url"]);
    assert_ne!(headers[header::ETAG], old_headers[header::ETAG]);
    assert_eq!(
        h.request("GET", "/api/v1/pokemon/8", None, json!(null))
            .await
            .2["active"],
        true
    );
    let changes = h.notifications(&owner).await;
    assert_eq!(changes["total"], 1);
    assert!(changes["notifications"][0]["differences"]["stats"].is_object());
    let (_, _, status) = h
        .request("GET", "/api/v1/catalog/status", None, json!(null))
        .await;
    assert_eq!(status["latest_run"]["pokemon_count"], 2);
    assert_eq!(status["latest_run"]["change_count"], 1);
    assert_eq!(status["revision"], 9);
    h.mock.fixture.lock().unwrap().fail = false;
    assert_eq!(h.sync().await.unwrap().pokemon_count, 7);
    assert_eq!(
        h.request("GET", "/api/v1/pokemon/8", None, json!(null))
            .await
            .2["active"],
        false
    );
    let changes = h.notifications(&owner).await;
    assert_eq!(changes["total"], 2);
    assert_eq!(
        changes["notifications"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|event| event["pokemon"]["id"] == 100)
            .count(),
        1
    );
    assert_eq!(h.sync().await.unwrap().change_count, 0);
}

#[sqlx::test(migrations = "./migrations")]
async fn database_failure_rolls_back_current_record_only(pool: PgPool) {
    let h = Harness::new(pool).await;
    h.sync().await.unwrap();
    let (_, _, before) = h
        .request("GET", "/api/v1/pokemon/3", None, json!(null))
        .await;
    let old_hash: String = sqlx::query_scalar("SELECT sprite_hash FROM pokemon WHERE id=3")
        .fetch_one(&h.state.pool)
        .await
        .unwrap();
    // Only the third Pokémon rejects the changed sprite. Earlier records can commit.
    let constraint = format!(
        "ALTER TABLE pokemon ADD CONSTRAINT reject_third_sprite CHECK(id<>3 OR sprite_hash='{old_hash}')"
    );
    sqlx::query(&constraint)
        .execute(&h.state.pool)
        .await
        .unwrap();
    h.mock.fixture.lock().unwrap().changed = true;
    assert!(h.sync().await.is_err());
    assert_eq!(
        h.request("GET", "/api/v1/pokemon/3", None, json!(null))
            .await
            .2,
        before
    );
    let (_, _, first) = h
        .request("GET", "/api/v1/pokemon/100", None, json!(null))
        .await;
    assert_ne!(first["sprite_url"], before["sprite_url"]);
    let (_, _, status) = h
        .request("GET", "/api/v1/catalog/status", None, json!(null))
        .await;
    assert_eq!(status["latest_run"]["pokemon_count"], 2);
    assert_eq!(status["latest_run"]["change_count"], 1);
    assert_eq!(status["revision"], 9);
}

struct PausedUpstream {
    api: PokeApi,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
impl sync::Upstream for PausedUpstream {
    fn species(&self) -> sync::Fetch<'_, Vec<poketeam::model::pokeapi::Resource>> {
        self.api.species()
    }
    fn chart(&self) -> sync::Fetch<'_, poketeam::model::TypeChart> {
        self.api.chart()
    }
    fn pokemon<'a>(
        &'a self,
        resource: &'a poketeam::model::pokeapi::Resource,
    ) -> sync::Fetch<'a, (poketeam::model::CatalogPokemon, Option<sync::Asset>)> {
        Box::pin(async move {
            if resource.id()? == 3 {
                self.entered.notify_one();
                self.release.notified().await;
            }
            self.api.pokemon(resource).await
        })
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn overlapping_scan_and_cancellation_release_dedicated_lock(pool: PgPool) {
    let h = Harness::new(pool).await;
    let upstream = Arc::new(PausedUpstream {
        api: PokeApi::new(h.state.config.clone()).unwrap(),
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    let state = h.state.clone();
    let paused = upstream.clone();
    let running = tokio::spawn(async move { sync::run(&state, paused.as_ref()).await });
    tokio::time::timeout(StdDuration::from_secs(2), upstream.entered.notified())
        .await
        .unwrap();
    let before = h.mock.requests.load(Ordering::SeqCst);
    assert!(h.sync().await.unwrap().skipped);
    assert_eq!(h.mock.requests.load(Ordering::SeqCst), before);
    assert_eq!(
        h.request("GET", "/api/v1/pokemon", None, json!(null))
            .await
            .0,
        StatusCode::OK
    );
    let (_, _, partial) = h.request("GET", "/api/v1/pokemon", None, json!(null)).await;
    assert_eq!(partial["total"], 2); // Two commits are visible while the next fetch is paused.
    running.abort();
    assert!(running.await.unwrap_err().is_cancelled());
    let saved: i64 = sqlx::query_scalar("SELECT count(*) FROM pokemon")
        .fetch_one(&h.state.pool)
        .await
        .unwrap();
    assert_eq!(saved, 2); // Cancellation retains completed transactions.
    let recovered = tokio::time::timeout(StdDuration::from_secs(2), async {
        loop {
            let result = h.sync().await.unwrap();
            if !result.skipped {
                break result;
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(recovered.change_count, 0);
    let abandoned: i64 =
        sqlx::query_scalar("SELECT count(*) FROM sync_runs WHERE outcome='abandoned'")
            .fetch_one(&h.state.pool)
            .await
            .unwrap();
    assert_eq!(abandoned, 1);
}

fn command(h: &Harness, name: &str) -> tokio::process::Command {
    let mut command = tokio::process::Command::new(env!("CARGO_BIN_EXE_poketeam"));
    command
        .arg(name)
        .env("DATABASE_URL", &h.state.config.database_url)
        .env("PUBLIC_ORIGIN", &h.state.config.public_origin)
        .env("UPSTREAM_URL", h.state.config.upstream.as_str())
        .env("UPSTREAM_ALLOWED_HOSTS", "127.0.0.1")
        .env("ALLOW_INSECURE_UPSTREAM", "true")
        .env("RUST_LOG", "error")
        .kill_on_drop(true);
    command
}

#[sqlx::test(migrations = "./migrations")]
async fn sync_cli_reports_failed_url_and_error(pool: PgPool) {
    let h = Harness::new(pool).await;
    h.mock.fixture.lock().unwrap().fail = true;
    let output = command(&h, "sync")
        .env("RUST_LOG", "poketeam=warn")
        .output()
        .await
        .unwrap();
    assert!(!output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let stderr = String::from_utf8(output.stderr).unwrap();
    let failures: Vec<Value> = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .filter(|event| event["fields"]["message"] == "upstream request failed")
        .collect();
    assert_eq!(failures.len(), 1, "{stdout}");
    let url = format!("{}pokemon/3/", h.state.config.upstream);
    assert_eq!(failures[0]["fields"]["url"], url);
    assert!(
        failures[0]["fields"]["error"]
            .as_str()
            .unwrap()
            .contains("503")
    );
    assert!(stderr.contains(&url), "{stderr}");
    assert!(stderr.contains("503"), "{stderr}");
    let summary: String = sqlx::query_scalar("SELECT failure_summary FROM sync_runs")
        .fetch_one(&h.state.pool)
        .await
        .unwrap();
    assert_eq!(summary, "synchronizing catalog");
}

#[sqlx::test(migrations = "./migrations")]
async fn json_decode_failure_keeps_catalog_and_logs_url(pool: PgPool) {
    let h = Harness::new(pool).await;
    h.sync().await.unwrap();
    let before = h
        .request("GET", "/api/v1/pokemon/100", None, json!(null))
        .await
        .2;
    h.mock.fixture.lock().unwrap().malformed_json = true;
    let output = command(&h, "sync")
        .env("RUST_LOG", "poketeam=warn")
        .output()
        .await
        .unwrap();
    assert!(!output.status.success());
    let stderr = String::from_utf8(output.stderr).unwrap();
    let stdout = String::from_utf8(output.stdout).unwrap();
    let url = format!("{}pokemon/3/", h.state.config.upstream);
    assert!(
        stderr.contains(&url) && stderr.contains("decoding upstream JSON"),
        "{stderr}"
    );
    let event: Value = stdout
        .lines()
        .filter_map(|line| serde_json::from_str::<Value>(line).ok())
        .find(|event| event["fields"]["message"] == "upstream request failed")
        .unwrap();
    assert_eq!(event["fields"]["url"], url);
    assert!(event["fields"]["error"].as_str().unwrap().contains("EOF"));
    assert_eq!(
        h.request("GET", "/api/v1/pokemon/100", None, json!(null))
            .await
            .2,
        before
    );
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM catalog_state")
        .fetch_one(&h.state.pool)
        .await
        .unwrap();
    assert_eq!(revision, 8);
}

async fn stop(child: &mut tokio::process::Child) {
    #[cfg(unix)]
    {
        let status = tokio::process::Command::new("kill")
            .arg("-TERM")
            .arg(child.id().unwrap().to_string())
            .status()
            .await
            .unwrap();
        assert!(status.success());
        assert!(
            tokio::time::timeout(StdDuration::from_secs(2), child.wait())
                .await
                .unwrap()
                .unwrap()
                .success()
        );
    }
    #[cfg(not(unix))]
    {
        child.kill().await.unwrap();
    }
}

#[sqlx::test(migrations = "./migrations")]
async fn executable_commands_health_worker_restart_and_shutdown(pool: PgPool) {
    let h = Harness::new(pool).await;
    assert!(
        command(&h, "migrate")
            .output()
            .await
            .unwrap()
            .status
            .success()
    );
    let reserved = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let address = reserved.local_addr().unwrap();
    drop(reserved);
    let mut server = command(&h, "serve")
        .env("LISTEN_ADDR", address.to_string())
        .spawn()
        .unwrap();
    let client = reqwest::Client::new();
    let base = format!("http://{address}");
    tokio::time::timeout(StdDuration::from_secs(3), async {
        loop {
            if client
                .get(format!("{base}/health/live"))
                .send()
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                break;
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        client
            .get(format!("{base}/health/ready"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
    let mut worker = command(&h, "worker").spawn().unwrap();
    tokio::time::timeout(StdDuration::from_secs(3), async {
        loop {
            if client
                .get(format!("{base}/health/ready"))
                .send()
                .await
                .unwrap()
                .status()
                == StatusCode::OK
            {
                break;
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    // Readiness now precedes a completed scan; wait for success before testing daily scheduling.
    tokio::time::timeout(StdDuration::from_secs(3), async {
        loop {
            let completed: bool =
                sqlx::query_scalar("SELECT last_success_at IS NOT NULL FROM catalog_state")
                    .fetch_one(&h.state.pool)
                    .await
                    .unwrap();
            if completed {
                break;
            }
            tokio::time::sleep(StdDuration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    stop(&mut worker).await;
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM sync_runs")
        .fetch_one(&h.state.pool)
        .await
        .unwrap();
    let mut restarted = command(&h, "worker").spawn().unwrap();
    tokio::time::sleep(StdDuration::from_millis(100)).await;
    stop(&mut restarted).await;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM sync_runs")
            .fetch_one(&h.state.pool)
            .await
            .unwrap(),
        count
    );
    let output = command(&h, "sync").output().await.unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["change_count"],
        0
    );
    let response = client
        .post(format!("{base}/api/v1/session"))
        .header(header::ORIGIN, "http://localhost:3000")
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(response.headers().contains_key(header::SET_COOKIE));
    let output = command(&h, "openapi").output().await.unwrap();
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        serde_json::from_str::<Value>(include_str!("../openapi.json")).unwrap()
    );
    stop(&mut server).await;
}
