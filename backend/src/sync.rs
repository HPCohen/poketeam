use crate::{
    AppState,
    config::Config,
    model::{CatalogPokemon, TypeChart, from_row, pokeapi},
};
use anyhow::{Context, Result, ensure};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use reqwest::{Client, Response};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use sqlx::{Connection, PgConnection, Row};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    pin::Pin,
    time::Duration,
};
use url::Url;
use uuid::Uuid;

const SYNC_LOCK: i64 = 731945110;
const MAX_SPRITE_BYTES: usize = 4 * 1024 * 1024;
#[derive(Clone)]
pub struct Asset {
    pub content_type: String,
    pub bytes: Vec<u8>,
}
pub type Fetch<'a, T> = Pin<Box<dyn Future<Output = Result<T>> + Send + 'a>>;
pub trait Upstream: Send + Sync {
    fn species(&self) -> Fetch<'_, Vec<pokeapi::Resource>>;
    fn chart(&self) -> Fetch<'_, TypeChart>;
    fn pokemon<'a>(
        &'a self,
        resource: &'a pokeapi::Resource,
    ) -> Fetch<'a, (CatalogPokemon, Option<Asset>)>;
}
#[derive(Clone)]
pub struct PokeApi {
    client: Client,
    config: Config,
}
fn validate_url(config: &Config, url: &Url) -> Result<()> {
    ensure!(
        url.username().is_empty() && url.password().is_none(),
        "upstream credentials forbidden"
    );
    ensure!(
        url.scheme() == "https" || (config.allow_insecure_upstream && url.scheme() == "http"),
        "unapproved upstream scheme"
    );
    ensure!(
        url.host_str()
            .is_some_and(|host| config.allowed_hosts.iter().any(|allowed| allowed == host)),
        "unapproved upstream host"
    );
    Ok(())
}
impl PokeApi {
    pub fn new(config: Config) -> Result<Self> {
        let redirect_config = config.clone();
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .connect_timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::custom(move |attempt| {
                if attempt.previous().len() > 5 {
                    return attempt.error("too many upstream redirects");
                }
                match validate_url(&redirect_config, attempt.url()) {
                    Ok(()) => attempt.follow(),
                    Err(error) => attempt.error(error.to_string()),
                }
            }))
            .user_agent("poketeam/0.1 (daily local catalog cache)")
            .build()?;
        Ok(Self { client, config })
    }
    async fn get(&self, url: &Url) -> Result<Response> {
        validate_url(&self.config, url)?;
        self.client
            .get(url.clone())
            .send()
            .await?
            .error_for_status()
            .context("upstream HTTP request failed")
    }
    async fn json<T: DeserializeOwned>(&self, url: Url) -> Result<T> {
        let result = async {
            let response = self.get(&url).await?;
            response.json::<T>().await.context("decoding upstream JSON")
        }
        .await
        .with_context(|| format!("fetching upstream JSON from {url}"));
        if let Err(error) = &result {
            tracing::warn!(url = %url, error = %format!("{error:#}"), "upstream request failed");
        }
        result
    }
    async fn sprite(&self, url: Url) -> Result<Asset> {
        let result = async {
            let response = self.get(&url).await?;
            ensure!(
                response
                    .content_length()
                    .is_none_or(|n| n <= MAX_SPRITE_BYTES as u64),
                "upstream sprite too large"
            );
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or("application/octet-stream")
                .split(';')
                .next()
                .unwrap()
                .to_owned();
            ensure!(
                matches!(
                    content_type.as_str(),
                    "image/png" | "image/jpeg" | "image/gif" | "image/webp"
                ),
                "invalid sprite content type"
            );
            let bytes = response.bytes().await?.to_vec();
            ensure!(
                !bytes.is_empty() && bytes.len() <= MAX_SPRITE_BYTES,
                "invalid sprite size"
            );
            Ok(Asset {
                content_type,
                bytes,
            })
        }
        .await
        .with_context(|| format!("fetching upstream sprite from {url}"));
        if let Err(error) = &result {
            tracing::warn!(url = %url, error = %format!("{error:#}"), "upstream request failed");
        }
        result
    }
    async fn list(&self, path: &str) -> Result<Vec<pokeapi::Resource>> {
        let mut next = Some(self.config.upstream.join(path)?);
        let mut results = Vec::new();
        let mut seen = BTreeSet::new();
        let mut expected = None;
        while let Some(url) = next {
            ensure!(seen.insert(url.to_string()), "pagination repeated a page");
            let page: pokeapi::Page = self.json(url).await?;
            ensure!(
                *expected.get_or_insert(page.count) == page.count,
                "discovery changed during pagination"
            );
            results.extend(page.results);
            next = page.next.map(|url| Url::parse(&url)).transpose()?;
        }
        ensure!(
            Some(results.len()) == expected && !results.is_empty(),
            "incomplete discovery"
        );
        ensure!(
            results
                .iter()
                .map(|resource| &resource.url)
                .collect::<BTreeSet<_>>()
                .len()
                == results.len(),
            "duplicate discovery entries"
        );
        Ok(results)
    }
    async fn fetch_pokemon(
        &self,
        resource: &pokeapi::Resource,
    ) -> Result<(CatalogPokemon, Option<Asset>)> {
        let species: pokeapi::Species = self.json(Url::parse(&resource.url)?).await?;
        ensure!(
            species.id > 0 && species.id == resource.id()?,
            "species identity mismatch"
        );
        let generation = species.generation.id()?;
        ensure!(generation > 0, "invalid introduction generation");
        let mut defaults = species
            .varieties
            .into_iter()
            .filter(|variety| variety.is_default);
        let default = defaults.next().context("species has no default variety")?;
        ensure!(
            defaults.next().is_none(),
            "species has multiple default varieties"
        );
        let raw: pokeapi::Pokemon = self.json(Url::parse(&default.pokemon.url)?).await?;
        ensure!(
            raw.id == default.pokemon.id()? && raw.species.id()? == species.id,
            "default variety/species mismatch"
        );
        let mut row = CatalogPokemon::try_from(raw)?;
        row.generation = Some(generation);
        row.is_starter = pokeapi::is_starter_family(species.id);
        row.is_legendary = species.is_legendary;
        row.is_mythical = species.is_mythical;
        for url in row.sprite_variants.values() {
            validate_url(&self.config, &Url::parse(url)?)?;
        }
        let asset = if let Some(url) = &row.sprite_source {
            let asset = self.sprite(Url::parse(url)?).await?;
            row.sprite_hash = Some(hex::encode(Sha256::digest(&asset.bytes)));
            Some(asset)
        } else {
            None
        };
        Ok((row, asset))
    }
    async fn fetch_chart(&self) -> Result<TypeChart> {
        let mut types = Vec::new();
        for resource in self.list("type?limit=200").await? {
            let id = resource.id()?;
            ensure!(id > 0, "invalid type resource");
            if id >= 10000 {
                continue;
            }
            let row: pokeapi::Type = self.json(Url::parse(&resource.url)?).await?;
            ensure!(row.name == resource.name, "type identity mismatch");
            types.push(row);
        }
        ensure!(
            types
                .iter()
                .map(|row| &row.name)
                .collect::<BTreeSet<_>>()
                .len()
                == types.len(),
            "duplicate types"
        );
        let mut chart = TypeChart::new();
        for row in &types {
            chart.insert(
                row.name.clone(),
                types
                    .iter()
                    .map(|defender| (defender.name.clone(), 1.0))
                    .collect(),
            );
        }
        for row in types {
            let effectiveness = chart.get_mut(&row.name).context("missing type")?;
            for (defenders, multiplier) in [
                (row.damage_relations.no_damage_to, 0.0),
                (row.damage_relations.half_damage_to, 0.5),
                (row.damage_relations.double_damage_to, 2.0),
            ] {
                for defender in defenders {
                    if let Some(value) = effectiveness.get_mut(&defender.name) {
                        *value = multiplier;
                    }
                }
            }
        }
        Ok(chart)
    }
}
impl Upstream for PokeApi {
    fn species(&self) -> Fetch<'_, Vec<pokeapi::Resource>> {
        Box::pin(self.list("pokemon-species?limit=200"))
    }
    fn chart(&self) -> Fetch<'_, TypeChart> {
        Box::pin(self.fetch_chart())
    }
    fn pokemon<'a>(
        &'a self,
        resource: &'a pokeapi::Resource,
    ) -> Fetch<'a, (CatalogPokemon, Option<Asset>)> {
        Box::pin(self.fetch_pokemon(resource))
    }
}

#[derive(Debug, serde::Serialize)]
pub struct SyncOutcome {
    pub skipped: bool,
    pub pokemon_count: usize,
    pub change_count: usize,
}
pub async fn run(state: &AppState, upstream: &dyn Upstream) -> Result<SyncOutcome> {
    // A dedicated connection cannot return to the pool while holding a session lock.
    let mut conn = PgConnection::connect(&state.config.database_url).await?;
    let locked: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1)")
        .bind(SYNC_LOCK)
        .fetch_one(&mut conn)
        .await?;
    if !locked {
        return Ok(SyncOutcome {
            skipped: true,
            pokemon_count: 0,
            change_count: 0,
        });
    }
    let run = Uuid::new_v4();
    let now = state.clock.now();
    sqlx::query(r#"
        UPDATE sync_runs SET outcome='abandoned', finished_at=$1, failure_summary='Worker stopped before completion'
        WHERE outcome='running'
        "#).bind(now).execute(&mut conn).await?;
    sqlx::query("INSERT INTO sync_runs(id,started_at,outcome,pokemon_count,change_count) VALUES($1,$2,'running',0,0)")
        .bind(run)
        .bind(now)
        .execute(&mut conn)
        .await?;
    let result = synchronize(state, upstream, &mut conn, run)
        .await
        .context("synchronizing catalog");
    if let Err(error) = &result {
        // Store only a summary; request logs and CLI errors carry upstream URLs.
        tracing::warn!(run_id=%run,error=%error,"synchronization failed");
        sqlx::query(
            "UPDATE sync_runs SET outcome='failed',finished_at=$2,failure_summary=$3 WHERE id=$1",
        )
        .bind(run)
        .bind(state.clock.now())
        .bind(error.to_string())
        .execute(&mut conn)
        .await?;
    }
    sqlx::query("SELECT pg_advisory_unlock($1)")
        .bind(SYNC_LOCK)
        .execute(&mut conn)
        .await?;
    conn.close().await?;
    result
}
fn differences(
    before: Option<&CatalogPokemon>,
    after: &CatalogPokemon,
) -> Result<serde_json::Map<String, Value>> {
    let mut changes = serde_json::Map::new();
    let after = serde_json::to_value(after)?;
    if let Some(before) = before {
        let before = serde_json::to_value(before)?;
        for (key, value) in after.as_object().context("pokemon object")? {
            if before.get(key) != Some(value) {
                changes.insert(key.clone(), json!({"before":before.get(key),"after":value}));
            }
        }
    } else {
        changes.insert("availability".into(), json!({"before":null,"after":after}));
    }
    Ok(changes)
}
async fn synchronize(
    state: &AppState,
    upstream: &dyn Upstream,
    conn: &mut PgConnection,
    run: Uuid,
) -> Result<SyncOutcome> {
    let chart = upstream.chart().await?;
    let chart_changes = publish_chart(conn, &chart, run, state.clock.now()).await?;
    let resources = upstream.species().await?;
    ensure!(!resources.is_empty(), "empty species discovery");
    let mut ids = BTreeSet::new();
    let mut species = BTreeSet::new();
    for resource in resources {
        let (pokemon, asset) = upstream.pokemon(&resource).await?;
        ensure!(
            ids.insert(pokemon.id) && species.insert(pokemon.species_id),
            "duplicate Pokémon or species ID"
        );
        ensure!(
            pokemon.active
                && !pokemon.types.is_empty()
                && pokemon.types.iter().all(|kind| chart.contains_key(kind)),
            "invalid Pokémon types"
        );
        let mut tx = conn.begin().await?;
        write_pokemon(
            &mut tx,
            &pokemon,
            asset,
            run,
            state.clock.now(),
            &chart_changes,
            true,
        )
        .await
        .with_context(|| format!("saving Pokémon {}", pokemon.id))?;
        tx.commit().await?;
        tracing::info!(run_id=%run,pokemon_id=pokemon.id,name=%pokemon.name,pokemon_count=ids.len(),"Pokémon saved");
    }
    finish(conn, &ids, run, state.clock.now(), &chart_changes).await
}

fn related_changes(
    pokemon: &CatalogPokemon,
    before: Option<&CatalogPokemon>,
    changes: &[Value],
) -> Vec<Value> {
    changes
        .iter()
        .filter(|change| {
            pokemon
                .types
                .iter()
                .chain(before.into_iter().flat_map(|row| row.types.iter()))
                .any(|kind| {
                    change["attacking"].as_str() == Some(kind)
                        || change["defending"].as_str() == Some(kind)
                })
        })
        .cloned()
        .collect()
}

// One sync holds the advisory lock, so this update/insert cannot race another writer.
// A chart change may already have created the event before this Pokémon is fetched.
async fn record_change(
    conn: &mut PgConnection,
    pokemon: i32,
    run: Uuid,
    now: DateTime<Utc>,
    changes: serde_json::Map<String, Value>,
) -> Result<()> {
    if changes.is_empty() {
        return Ok(());
    }
    let changes = Value::Object(changes);
    let updated = sqlx::query("UPDATE pokemon_changes SET differences=differences || $3 WHERE run_id=$1 AND pokemon_id=$2")
        .bind(run).bind(pokemon).bind(&changes).execute(&mut *conn).await?.rows_affected();
    if updated == 0 {
        sqlx::query("INSERT INTO pokemon_changes(pokemon_id,run_id,detected_at,differences) VALUES($1,$2,$3,$4)")
            .bind(pokemon).bind(run).bind(now).bind(changes).execute(conn).await?;
    }
    Ok(())
}

async fn update_progress(conn: &mut PgConnection, run: Uuid, processed: bool) -> Result<()> {
    sqlx::query("UPDATE sync_runs SET pokemon_count=COALESCE(pokemon_count,0)+$2,change_count=(SELECT count(*)::integer FROM pokemon_changes WHERE run_id=$1) WHERE id=$1")
        .bind(run).bind(i32::from(processed)).execute(conn).await?;
    Ok(())
}

async fn publish_chart(
    conn: &mut PgConnection,
    chart: &TypeChart,
    run: Uuid,
    now: DateTime<Utc>,
) -> Result<Vec<Value>> {
    ensure!(!chart.is_empty(), "empty type chart");
    let mut tx = conn.begin().await?;
    let initialized: bool = sqlx::query_scalar(
        "SELECT last_success_at IS NOT NULL FROM catalog_state WHERE singleton FOR UPDATE",
    )
    .fetch_one(&mut *tx)
    .await?;
    let old: Vec<(String, String, f64)> = sqlx::query_as("SELECT attacking,defending,multiplier FROM type_effectiveness ORDER BY attacking,defending").fetch_all(&mut *tx).await?;
    let new: Vec<_> = chart
        .iter()
        .flat_map(|(attacker, defenders)| {
            defenders.iter().map(move |(defender, multiplier)| {
                (attacker.clone(), defender.clone(), *multiplier)
            })
        })
        .collect();
    let old_rel: BTreeMap<_, _> = old.iter().map(|(a, d, m)| ((a, d), m)).collect();
    let new_rel: BTreeMap<_, _> = new.iter().map(|(a, d, m)| ((a, d), m)).collect();
    let mut changes = Vec::new();
    for (attacker, defender) in old_rel
        .keys()
        .chain(new_rel.keys())
        .collect::<BTreeSet<_>>()
    {
        if old_rel.get(&(attacker, defender)) != new_rel.get(&(attacker, defender)) {
            changes.push(json!({"attacking":attacker,"defending":defender,"before":old_rel.get(&(attacker,defender)),"after":new_rel.get(&(attacker,defender))}));
        }
    }
    if !changes.is_empty() {
        sqlx::query("DELETE FROM type_effectiveness")
            .execute(&mut *tx)
            .await?;
        for (attacker, defender, multiplier) in new {
            sqlx::query("INSERT INTO type_effectiveness VALUES($1,$2,$3)")
                .bind(attacker)
                .bind(defender)
                .bind(multiplier)
                .execute(&mut *tx)
                .await?;
        }
        if initialized {
            let rows = sqlx::query("SELECT * FROM pokemon ORDER BY id")
                .fetch_all(&mut *tx)
                .await?;
            for row in rows {
                let pokemon = from_row(&row)?;
                let relevant = related_changes(&pokemon, None, &changes);
                if !relevant.is_empty() {
                    let mut diff = serde_json::Map::new();
                    diff.insert("type_effectiveness".into(), json!(relevant));
                    record_change(&mut tx, pokemon.id, run, now, diff).await?;
                }
            }
        }
        // A chart alone is not a usable initial catalog; the first Pokémon makes it ready.
        sqlx::query("UPDATE catalog_state SET revision=revision+1 WHERE singleton AND EXISTS(SELECT 1 FROM pokemon)").execute(&mut *tx).await?;
        update_progress(&mut tx, run, false).await?;
    }
    tx.commit().await?;
    Ok(changes)
}

// Caller owns a short transaction: the row, sprite, revision, alert and progress commit together.
async fn write_pokemon(
    conn: &mut PgConnection,
    pokemon: &CatalogPokemon,
    asset: Option<Asset>,
    run: Uuid,
    now: DateTime<Utc>,
    chart_changes: &[Value],
    processed: bool,
) -> Result<()> {
    let (revision, initialized): (i64, bool) = sqlx::query_as(
        "SELECT revision,last_success_at IS NOT NULL FROM catalog_state WHERE singleton FOR UPDATE",
    )
    .fetch_one(&mut *conn)
    .await?;
    let before = sqlx::query("SELECT * FROM pokemon WHERE id=$1")
        .bind(pokemon.id)
        .fetch_optional(&mut *conn)
        .await?
        .as_ref()
        .map(from_row)
        .transpose()?;
    if let Some(asset) = asset {
        let hash = hex::encode(Sha256::digest(&asset.bytes));
        ensure!(
            pokemon.sprite_hash.as_deref() == Some(hash.as_str()),
            "invalid sprite asset hash"
        );
        sqlx::query("INSERT INTO sprite_assets(hash,content_type,bytes) VALUES($1,$2,$3) ON CONFLICT DO NOTHING")
            .bind(hash).bind(asset.content_type).bind(asset.bytes).execute(&mut *conn).await?;
    }
    let mut diff = differences(before.as_ref(), pokemon)?;
    let changed = !diff.is_empty();
    sqlx::query(r#"
        INSERT INTO pokemon(id,species_id,name,types,stats,sprite_source,sprite_hash,active,sprite_variants,generation,is_starter,is_legendary,is_mythical) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13)
        ON CONFLICT(id) DO UPDATE SET species_id=excluded.species_id,name=excluded.name,types=excluded.types,stats=excluded.stats,sprite_source=excluded.sprite_source,sprite_hash=excluded.sprite_hash,active=excluded.active,sprite_variants=excluded.sprite_variants,generation=excluded.generation,is_starter=excluded.is_starter,is_legendary=excluded.is_legendary,is_mythical=excluded.is_mythical
        "#).bind(pokemon.id).bind(pokemon.species_id).bind(&pokemon.name).bind(&pokemon.types).bind(sqlx::types::Json(&pokemon.stats)).bind(&pokemon.sprite_source).bind(&pokemon.sprite_hash).bind(pokemon.active).bind(sqlx::types::Json(&pokemon.sprite_variants)).bind(pokemon.generation).bind(pokemon.is_starter).bind(pokemon.is_legendary).bind(pokemon.is_mythical).execute(&mut *conn).await?;
    if initialized {
        let relevant = related_changes(pokemon, before.as_ref(), chart_changes);
        if !relevant.is_empty() {
            diff.insert("type_effectiveness".into(), json!(relevant));
        }
        record_change(conn, pokemon.id, run, now, diff).await?;
    }
    if changed || revision == 0 {
        sqlx::query("UPDATE catalog_state SET revision=revision+1 WHERE singleton")
            .execute(&mut *conn)
            .await?;
    }
    update_progress(conn, run, processed).await?;
    Ok(())
}

async fn finish(
    conn: &mut PgConnection,
    ids: &BTreeSet<i32>,
    run: Uuid,
    now: DateTime<Utc>,
    chart_changes: &[Value],
) -> Result<SyncOutcome> {
    let mut tx = conn.begin().await?;
    let ids: Vec<_> = ids.iter().copied().collect();
    let missing = sqlx::query("SELECT * FROM pokemon WHERE active AND NOT(id=ANY($1)) ORDER BY id")
        .bind(&ids)
        .fetch_all(&mut *tx)
        .await?;
    for row in missing {
        let mut pokemon = from_row(&row)?;
        pokemon.active = false;
        write_pokemon(&mut tx, &pokemon, None, run, now, chart_changes, false).await?;
    }
    sqlx::query("UPDATE catalog_state SET last_success_at=$1 WHERE singleton")
        .bind(now)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM pokemon_changes WHERE detected_at < $1")
        .bind(now - ChronoDuration::days(30))
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM sync_runs WHERE finished_at < $1 AND id NOT IN (SELECT run_id FROM pokemon_changes)").bind(now - ChronoDuration::days(30)).execute(&mut *tx).await?;
    let (pokemon_count, change_count): (i32, i32) = sqlx::query_as("UPDATE sync_runs SET outcome='success',finished_at=$2 WHERE id=$1 RETURNING pokemon_count,change_count").bind(run).bind(now).fetch_one(&mut *tx).await?;
    tx.commit().await?;
    tracing::info!(run_id=%run,pokemon_count,change_count,"catalog synchronized");
    Ok(SyncOutcome {
        skipped: false,
        pokemon_count: pokemon_count as usize,
        change_count: change_count as usize,
    })
}

pub async fn next_delay(state: &AppState) -> Result<Duration> {
    let latest =
        sqlx::query("SELECT outcome,finished_at FROM sync_runs ORDER BY sequence DESC LIMIT 1")
            .fetch_optional(&state.pool)
            .await?;
    if let Some(row) = latest {
        let outcome: String = row.try_get("outcome")?;
        if outcome == "running" {
            return Ok(Duration::ZERO);
        }
        let finished: DateTime<Utc> = row.try_get("finished_at")?;
        let interval = if outcome == "success" {
            state.config.scan_interval
        } else {
            Duration::from_secs(900)
        };
        Ok(
            (finished + ChronoDuration::from_std(interval)? - state.clock.now())
                .to_std()
                .unwrap_or_default(),
        )
    } else {
        Ok(Duration::ZERO)
    }
}
pub async fn worker(state: AppState, upstream: &dyn Upstream) -> Result<()> {
    loop {
        let delay = next_delay(&state).await?;
        tokio::select! { _=crate::http::shutdown_signal()=>return Ok(()), _=tokio::time::sleep(delay)=>{} }
        // Cancellation rolls back the current transaction and retains earlier committed records.
        tokio::select! {
            _=crate::http::shutdown_signal()=>return Ok(()),
            result=run(&state,upstream)=>match result {
                Ok(outcome) if outcome.skipped=> {
                    tokio::select! { _=crate::http::shutdown_signal()=>return Ok(()), _=tokio::time::sleep(Duration::from_secs(60))=>{} }
                },
                Err(error)=> {
                    tracing::error!(error=%error,"worker scan failed");
                    tokio::select! { _=crate::http::shutdown_signal()=>return Ok(()), _=tokio::time::sleep(Duration::from_secs(900))=>{} }
                },
                _=>{}
            }
        }
    }
}
