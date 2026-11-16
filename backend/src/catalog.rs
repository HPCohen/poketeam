use crate::{
    AppState,
    http::{ApiError, ApiPath, ApiQuery, ApiResult, ErrorBody},
    model::{self, Pokemon, TypeChart},
};
use axum::{
    Json,
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Postgres, Row, Transaction};
use utoipa::ToSchema;
use uuid::Uuid;

pub(crate) fn unavailable() -> ApiError {
    ApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "catalog_unavailable",
        "Catalog has not been initialized",
    )
}
pub(crate) async fn snapshot(state: &AppState) -> ApiResult<Transaction<'_, Postgres>> {
    let mut tx = state.pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}
pub(crate) async fn revision(conn: &mut PgConnection) -> ApiResult<i64> {
    let rev: i64 = sqlx::query_scalar("SELECT revision FROM catalog_state WHERE singleton")
        .fetch_one(conn)
        .await?;
    if rev == 0 {
        return Err(unavailable());
    }
    Ok(rev)
}
fn cached<T: Serialize>(headers: &HeaderMap, key: &str, value: T) -> ApiResult<Response> {
    let bytes = serde_json::to_vec(&value)?;
    let etag = format!("\"{key}-{}\"", hex::encode(Sha256::digest(&bytes)));
    let matched = headers
        .get(header::IF_NONE_MATCH)
        .and_then(|h| h.to_str().ok())
        .is_some_and(|h| {
            h.split(',').any(|x| {
                let x = x.trim().trim_start_matches("W/");
                x == etag || x == "*"
            })
        });
    let mut response = if matched {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        ([(header::CONTENT_TYPE, "application/json")], bytes).into_response()
    };
    response.headers_mut().insert(
        header::ETAG,
        HeaderValue::from_str(&etag).expect("hash ETag"),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("public, max-age=0, must-revalidate"),
    );
    Ok(response)
}
#[derive(Deserialize)]
pub struct CatalogQuery {
    #[serde(default = "page_size")]
    limit: u32,
    #[serde(default)]
    offset: u32,
    search: Option<String>,
    #[serde(rename = "type")]
    kind: Option<String>,
}
pub(crate) fn page_size() -> u32 {
    60
}
pub(crate) fn validate_page(limit: u32, offset: u32) -> ApiResult<()> {
    if !(1..=200).contains(&limit) || offset > i32::MAX as u32 {
        return Err(ApiError::invalid(
            "limit must be 1–200 and offset must be at most 2147483647",
        ));
    }
    Ok(())
}
#[derive(Serialize, ToSchema)]
pub struct CatalogPage {
    pub pokemon: Vec<Pokemon>,
    pub total: i64,
    pub limit: u32,
    pub offset: u32,
    pub catalog_revision: i64,
}
#[utoipa::path(get,path="/api/v1/pokemon",params(("limit"=Option<u32>,Query),("offset"=Option<u32>,Query),("search"=Option<String>,Query),("type"=Option<String>,Query)),responses((status=200,body=CatalogPage),(status=304),(status=503,body=ErrorBody)),tag="catalog")]
pub(crate) async fn catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiQuery(q): ApiQuery<CatalogQuery>,
) -> ApiResult<Response> {
    validate_page(q.limit, q.offset)?;
    let mut tx = snapshot(&state).await?;
    let rev = revision(&mut tx).await?;
    let search = q.search.unwrap_or_default().to_lowercase();
    let kind = q.kind.map(|s| s.to_lowercase());
    let total:i64=sqlx::query_scalar("SELECT count(*) FROM pokemon WHERE active AND strpos(lower(name),$1)>0 AND ($2::text IS NULL OR $2=ANY(types))").bind(&search).bind(&kind).fetch_one(&mut *tx).await?;
    let rows = sqlx::query(
        r#"
        SELECT *
        FROM pokemon
        WHERE active
        AND strpos(lower(name),$1)>0
        AND ($2::text IS NULL OR $2=ANY(types))
        ORDER BY id
        LIMIT $3
        OFFSET $4
        "#,
    )
    .bind(search)
    .bind(kind)
    .bind(i64::from(q.limit))
    .bind(i64::from(q.offset))
    .fetch_all(&mut *tx)
    .await?;
    let pokemon = rows
        .iter()
        .map(model::from_row)
        .map(|p| p.map(|p| p.public()))
        .collect::<Result<_, _>>()?;
    cached(
        &headers,
        &rev.to_string(),
        CatalogPage {
            pokemon,
            total,
            limit: q.limit,
            offset: q.offset,
            catalog_revision: rev,
        },
    )
}
#[utoipa::path(get,path="/api/v1/pokemon/{id}",params(("id"=i32,Path)),responses((status=200,body=Pokemon),(status=304),(status=400,body=ErrorBody),(status=404,body=ErrorBody),(status=503,body=ErrorBody)),tag="catalog")]
pub(crate) async fn pokemon(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiPath(id): ApiPath<i32>,
) -> ApiResult<Response> {
    let mut tx = snapshot(&state).await?;
    let rev = revision(&mut tx).await?;
    let row = sqlx::query("SELECT * FROM pokemon WHERE id=$1")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or_else(ApiError::not_found)?;
    cached(&headers, &rev.to_string(), model::from_row(&row)?.public())
}
#[derive(Serialize, ToSchema)]
pub struct TypeResponse {
    pub types: Vec<String>,
    pub effectiveness: TypeChart,
    pub catalog_revision: i64,
}
pub(crate) async fn chart(conn: &mut PgConnection) -> ApiResult<TypeChart> {
    let rows:Vec<(String,String,f64)>=sqlx::query_as("SELECT attacking,defending,multiplier FROM type_effectiveness ORDER BY attacking,defending").fetch_all(conn).await?;
    let mut chart = TypeChart::new();
    for (a, d, m) in rows {
        chart.entry(a).or_default().insert(d, m);
    }
    Ok(chart)
}
#[utoipa::path(get,path="/api/v1/types",responses((status=200,body=TypeResponse),(status=304),(status=503,body=ErrorBody)),tag="catalog")]
pub(crate) async fn types(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> ApiResult<Response> {
    let mut tx = snapshot(&state).await?;
    let rev = revision(&mut tx).await?;
    let effectiveness = chart(&mut tx).await?;
    cached(
        &headers,
        &rev.to_string(),
        TypeResponse {
            types: effectiveness.keys().cloned().collect(),
            effectiveness,
            catalog_revision: rev,
        },
    )
}
#[utoipa::path(get,path="/api/v1/sprites/{hash}",params(("hash"=String,Path)),responses((status=200,description="Image bytes; immutable for one year"),(status=404,body=ErrorBody)),tag="catalog")]
pub(crate) async fn sprite(
    State(state): State<AppState>,
    ApiPath(hash): ApiPath<String>,
) -> ApiResult<Response> {
    let row = sqlx::query("SELECT content_type,bytes FROM sprite_assets WHERE hash=$1")
        .bind(hash)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let content_type: String = row.try_get("content_type")?;
    let bytes: Vec<u8> = row.try_get("bytes")?;
    Ok((
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable".into(),
            ),
        ],
        Body::from(bytes),
    )
        .into_response())
}
#[derive(Serialize, ToSchema)]
pub struct RefreshRun {
    id: Uuid,
    started_at: DateTime<Utc>,
    finished_at: Option<DateTime<Utc>>,
    outcome: String,
    pokemon_count: Option<i32>,
    change_count: Option<i32>,
    failure_summary: Option<String>,
}
#[derive(Serialize, ToSchema)]
pub struct CatalogStatus {
    revision: i64,
    last_success_at: Option<DateTime<Utc>>,
    latest_run: Option<RefreshRun>,
}
#[utoipa::path(get,path="/api/v1/catalog/status",responses((status=200,body=CatalogStatus),(status=503,body=ErrorBody)),tag="catalog")]
pub(crate) async fn status(State(state): State<AppState>) -> ApiResult<Json<CatalogStatus>> {
    let mut tx = snapshot(&state).await?;
    let (revision, last_success_at) =
        sqlx::query_as("SELECT revision,last_success_at FROM catalog_state WHERE singleton")
            .fetch_one(&mut *tx)
            .await?;
    let row = sqlx::query("SELECT * FROM sync_runs ORDER BY sequence DESC LIMIT 1")
        .fetch_optional(&mut *tx)
        .await?;
    let latest_run = row
        .map(|r| {
            Ok::<_, sqlx::Error>(RefreshRun {
                id: r.try_get("id")?,
                started_at: r.try_get("started_at")?,
                finished_at: r.try_get("finished_at")?,
                outcome: r.try_get("outcome")?,
                pokemon_count: r.try_get("pokemon_count")?,
                change_count: r.try_get("change_count")?,
                failure_summary: r.try_get("failure_summary")?,
            })
        })
        .transpose()?;
    Ok(Json(CatalogStatus {
        revision,
        last_success_at,
        latest_run,
    }))
}
