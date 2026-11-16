use crate::{
    AppState,
    catalog::{chart, revision, snapshot, unavailable},
    counters::{self, CounterResult, Mode},
    http::{ApiError, ApiJson, ApiPath, ApiResult, ErrorBody},
    identity::owner,
    model::{self, Pokemon},
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::{PgConnection, Row};
use std::collections::BTreeSet;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CreateTeam {
    name: String,
    #[serde(default)]
    pokemon_ids: Vec<i32>,
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdateTeam {
    name: String,
    pokemon_ids: Vec<i32>,
    revision: i64,
}
#[derive(Serialize, ToSchema)]
pub struct Team {
    id: Uuid,
    name: String,
    revision: i64,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    pokemon: Vec<Pokemon>,
}
fn validate_team(name: &str, ids: &[i32]) -> ApiResult<String> {
    let name = name.trim();
    if !(1..=100).contains(&name.chars().count()) {
        return Err(ApiError::invalid(
            "Name must contain 1–100 trimmed characters",
        ));
    }
    if ids.len() > 6 || ids.iter().collect::<BTreeSet<_>>().len() != ids.len() {
        return Err(ApiError::invalid("Select zero to six distinct Pokémon"));
    }
    Ok(name.to_owned())
}
async fn load_team(conn: &mut PgConnection, id: Uuid, owner: Uuid) -> ApiResult<Team> {
    let row = sqlx::query("SELECT * FROM teams WHERE id=$1 AND owner_id=$2")
        .bind(id)
        .bind(owner)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let rows=sqlx::query("SELECT p.* FROM team_members m JOIN pokemon p ON p.id=m.pokemon_id WHERE m.team_id=$1 ORDER BY m.position").bind(id).fetch_all(conn).await?;
    Ok(Team {
        id,
        name: row.try_get("name")?,
        revision: row.try_get("revision")?,
        created_at: row.try_get("created_at")?,
        updated_at: row.try_get("updated_at")?,
        pokemon: rows
            .iter()
            .map(model::from_row)
            .map(|p| p.map(|p| p.public()))
            .collect::<Result<_, _>>()?,
    })
}
async fn replace_members(
    conn: &mut PgConnection,
    id: Uuid,
    ids: &[i32],
    retained: &[i32],
) -> ApiResult<()> {
    // Serialize selection validation with catalog publication.
    let revision: i64 =
        sqlx::query_scalar("SELECT revision FROM catalog_state WHERE singleton FOR SHARE")
            .fetch_one(&mut *conn)
            .await?;
    if !ids.is_empty() && revision == 0 {
        return Err(unavailable());
    }
    let found: Vec<(i32, bool)> = sqlx::query_as("SELECT id,active FROM pokemon WHERE id=ANY($1)")
        .bind(ids)
        .fetch_all(&mut *conn)
        .await?;
    if found.len() != ids.len()
        || found
            .iter()
            .any(|(id, active)| !active && !retained.contains(id))
    {
        return Err(ApiError::invalid(
            "Unknown or newly selected unavailable Pokémon",
        ));
    }
    sqlx::query("DELETE FROM team_members WHERE team_id=$1")
        .bind(id)
        .execute(&mut *conn)
        .await?;
    for (position, pokemon_id) in ids.iter().enumerate() {
        sqlx::query("INSERT INTO team_members(team_id,pokemon_id,position) VALUES($1,$2,$3)")
            .bind(id)
            .bind(pokemon_id)
            .bind(position as i16)
            .execute(&mut *conn)
            .await?;
    }
    Ok(())
}
#[utoipa::path(get,path="/api/v1/teams",responses((status=200,body=Vec<Team>),(status=401,body=ErrorBody)),security(("owner_cookie"=[])),tag="teams")]
pub(crate) async fn teams(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Json<Vec<Team>>> {
    let owner = owner(&state, &headers).await?;
    let mut tx = snapshot(&state).await?;
    let ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM teams WHERE owner_id=$1 ORDER BY created_at,id")
            .bind(owner)
            .fetch_all(&mut *tx)
            .await?;
    let mut teams = Vec::new();
    for id in ids {
        teams.push(load_team(&mut tx, id, owner).await?);
    }
    Ok(Json(teams))
}
#[utoipa::path(post,path="/api/v1/teams",request_body=CreateTeam,responses((status=201,body=Team),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=422,body=ErrorBody),(status=503,body=ErrorBody)),security(("owner_cookie"=[])),tag="teams")]
pub(crate) async fn create_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(body): ApiJson<CreateTeam>,
) -> ApiResult<(StatusCode, Json<Team>)> {
    let owner = owner(&state, &headers).await?;
    let name = validate_team(&body.name, &body.pokemon_ids)?;
    let id = Uuid::new_v4();
    let now = state.clock.now();
    let mut tx = state.pool.begin().await?;
    sqlx::query("INSERT INTO teams(id,owner_id,name,created_at,updated_at) VALUES($1,$2,$3,$4,$4)")
        .bind(id)
        .bind(owner)
        .bind(name)
        .bind(now)
        .execute(&mut *tx)
        .await?;
    replace_members(&mut tx, id, &body.pokemon_ids, &[]).await?;
    let team = load_team(&mut tx, id, owner).await?;
    tx.commit().await?;
    Ok((StatusCode::CREATED, Json(team)))
}
#[utoipa::path(get,path="/api/v1/teams/{id}",params(("id"=Uuid,Path)),responses((status=200,body=Team),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=404,body=ErrorBody),(status=503,body=ErrorBody)),security(("owner_cookie"=[])),tag="teams")]
pub(crate) async fn team(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiPath(id): ApiPath<Uuid>,
) -> ApiResult<Json<Team>> {
    let owner = owner(&state, &headers).await?;
    let mut tx = snapshot(&state).await?;
    Ok(Json(load_team(&mut tx, id, owner).await?))
}
#[utoipa::path(put,path="/api/v1/teams/{id}",params(("id"=Uuid,Path)),request_body=UpdateTeam,responses((status=200,body=Team),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=404,body=ErrorBody),(status=409,body=ErrorBody),(status=422,body=ErrorBody),(status=503,body=ErrorBody)),security(("owner_cookie"=[])),tag="teams")]
pub(crate) async fn update_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(body): ApiJson<UpdateTeam>,
) -> ApiResult<Json<Team>> {
    let owner = owner(&state, &headers).await?;
    let name = validate_team(&body.name, &body.pokemon_ids)?;
    let mut tx = state.pool.begin().await?;
    let current: Option<i64> =
        sqlx::query_scalar("SELECT revision FROM teams WHERE id=$1 AND owner_id=$2 FOR UPDATE")
            .bind(id)
            .bind(owner)
            .fetch_optional(&mut *tx)
            .await?;
    let current = current.ok_or_else(ApiError::not_found)?;
    if current != body.revision {
        return Err(ApiError::new(
            StatusCode::CONFLICT,
            "revision_conflict",
            "Reload the team and retry with its current revision",
        ));
    }
    let retained: Vec<i32> =
        sqlx::query_scalar("SELECT pokemon_id FROM team_members WHERE team_id=$1")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
    replace_members(&mut tx, id, &body.pokemon_ids, &retained).await?;
    sqlx::query("UPDATE teams SET name=$2,revision=revision+1,updated_at=$3 WHERE id=$1")
        .bind(id)
        .bind(name)
        .bind(state.clock.now())
        .execute(&mut *tx)
        .await?;
    let team = load_team(&mut tx, id, owner).await?;
    tx.commit().await?;
    Ok(Json(team))
}
#[utoipa::path(delete,path="/api/v1/teams/{id}",params(("id"=Uuid,Path)),request_body=Object,responses((status=204),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=404,body=ErrorBody),(status=503,body=ErrorBody)),security(("owner_cookie"=[])),tag="teams")]
pub(crate) async fn delete_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(_): ApiJson<serde_json::Map<String, Value>>,
) -> ApiResult<StatusCode> {
    let owner = owner(&state, &headers).await?;
    let result = sqlx::query("DELETE FROM teams WHERE id=$1 AND owner_id=$2")
        .bind(id)
        .bind(owner)
        .execute(&state.pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(ApiError::not_found());
    }
    Ok(StatusCode::NO_CONTENT)
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CounterRequest {
    #[serde(default)]
    mode: Mode,
}
static COUNTER_SLOTS: tokio::sync::Semaphore = tokio::sync::Semaphore::const_new(2);
#[utoipa::path(post,path="/api/v1/teams/{id}/counter-team",params(("id"=Uuid,Path)),request_body=CounterRequest,responses((status=200,body=CounterResult),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=404,body=ErrorBody),(status=422,body=ErrorBody),(status=503,body=ErrorBody)),security(("owner_cookie"=[])),tag="counters")]
pub(crate) async fn counter_team(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiPath(id): ApiPath<Uuid>,
    ApiJson(body): ApiJson<CounterRequest>,
) -> ApiResult<Json<CounterResult>> {
    let owner = owner(&state, &headers).await?;
    let permit = COUNTER_SLOTS.try_acquire().map_err(|_| {
        ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "counter_busy",
            "Counter search busy; retry shortly",
        )
    })?;
    let mut tx = snapshot(&state).await?;
    let rev = revision(&mut tx).await?;
    let source = load_team(&mut tx, id, owner).await?.pokemon;
    let candidates = sqlx::query("SELECT * FROM pokemon WHERE active ORDER BY id")
        .fetch_all(&mut *tx)
        .await?
        .iter()
        .map(model::from_row)
        .map(|p| p.map(|p| p.public()))
        .collect::<Result<Vec<_>, _>>()?;
    let chart = chart(&mut tx).await?;
    tx.commit().await?;
    let result = tokio::task::spawn_blocking(move || {
        let _permit = permit;
        counters::generate(&source, &candidates, &chart, body.mode, rev)
    })
    .await
    .map_err(|_| {
        ApiError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "counter_failed",
            "Counter task failed",
        )
    })?;
    result.map(Json).map_err(|code|ApiError::new(StatusCode::UNPROCESSABLE_ENTITY,code,match code {"rival_balance_not_found"=>"The heuristic did not find a team within ±5% total base stats; a qualifying team may still exist","empty_source_team"=>"Add at least one Pokémon before generating a counter team",_=>"Too few active Pokémon to generate an equal-size team"}))
}
