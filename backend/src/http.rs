use crate::{
    AppState,
    catalog::{self, CatalogPage, CatalogStatus, RefreshRun, TypeResponse, unavailable},
    counters::{CounterResult, Mode},
    identity::{self, Session, cookie, token},
    model::{Pokemon, Stats},
    notifications::{self, Notification, NotificationPage},
    teams::{self, CounterRequest, CreateTeam, Team, UpdateTeam},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, FromRequest, FromRequestParts, Path, Query, Request, State},
    http::{HeaderValue, StatusCode, header, request::Parts},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Serialize, de::DeserializeOwned};
use serde_json::{Value, json};
use tower_http::trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer};
use utoipa::{OpenApi, ToSchema};

#[derive(Debug, Serialize, ToSchema)]
pub struct ErrorBody {
    pub code: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
}
pub struct ApiError(pub(crate) StatusCode, pub(crate) ErrorBody);
impl ApiError {
    pub(crate) fn new(status: StatusCode, code: &str, message: impl Into<String>) -> Self {
        Self(
            status,
            ErrorBody {
                code: code.into(),
                message: message.into(),
                details: None,
            },
        )
    }
    pub(crate) fn invalid(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "validation_error",
            message,
        )
    }
    pub(crate) fn not_found() -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", "Resource not found")
    }
}
impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (self.0, Json(self.1)).into_response()
    }
}
impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        tracing::error!(error=%error,"database request failed");
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "database_unavailable",
            "Database operation failed; retry later",
        )
    }
}
impl From<serde_json::Error> for ApiError {
    fn from(_: serde_json::Error) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            "internal_error",
            "Stored data is invalid",
        )
    }
}

pub(crate) type ApiResult<T> = Result<T, ApiError>;
pub struct ApiJson<T>(pub T);
impl<S: Send + Sync, T: DeserializeOwned + Send> FromRequest<S> for ApiJson<T> {
    type Rejection = ApiError;
    async fn from_request(req: Request, state: &S) -> ApiResult<Self> {
        Json::<T>::from_request(req, state)
            .await
            .map(|Json(v)| Self(v))
            .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "invalid_json", e.body_text()))
    }
}
pub struct ApiQuery<T>(pub T);
impl<S: Send + Sync, T: DeserializeOwned + Send> FromRequestParts<S> for ApiQuery<T> {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> ApiResult<Self> {
        Query::<T>::from_request_parts(parts, state)
            .await
            .map(|Query(v)| Self(v))
            .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "invalid_query", e.body_text()))
    }
}
pub struct ApiPath<T>(pub T);
impl<S: Send + Sync, T: DeserializeOwned + Send> FromRequestParts<S> for ApiPath<T> {
    type Rejection = ApiError;
    async fn from_request_parts(parts: &mut Parts, state: &S) -> ApiResult<Self> {
        Path::<T>::from_request_parts(parts, state)
            .await
            .map(|Path(v)| Self(v))
            .map_err(|e| ApiError::new(StatusCode::BAD_REQUEST, "invalid_path", e.body_text()))
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/openapi.json", get(openapi))
        .route("/api/v1/session", post(identity::session))
        .route("/api/v1/pokemon", get(catalog::catalog))
        .route("/api/v1/pokemon/{id}", get(catalog::pokemon))
        .route("/api/v1/types", get(catalog::types))
        .route("/api/v1/sprites/{hash}", get(catalog::sprite))
        .route("/api/v1/catalog/status", get(catalog::status))
        .route("/api/v1/teams", get(teams::teams).post(teams::create_team))
        .route(
            "/api/v1/teams/{id}",
            get(teams::team)
                .put(teams::update_team)
                .delete(teams::delete_team),
        )
        .route("/api/v1/teams/{id}/counter-team", post(teams::counter_team))
        .route("/api/v1/notifications", get(notifications::notifications))
        .fallback(|| async { ApiError::not_found() })
        .method_not_allowed_fallback(|| async {
            ApiError::new(
                StatusCode::METHOD_NOT_ALLOWED,
                "method_not_allowed",
                "Method not allowed",
            )
        })
        .layer(DefaultBodyLimit::max(16 * 1024))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            request_policy,
        ))
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(tracing::Level::INFO))
                .on_response(DefaultOnResponse::new().level(tracing::Level::INFO)),
        )
        .with_state(state)
}
async fn request_policy(State(state): State<AppState>, request: Request, next: Next) -> Response {
    if matches!(
        *request.method(),
        axum::http::Method::POST
            | axum::http::Method::PUT
            | axum::http::Method::DELETE
            | axum::http::Method::PATCH
    ) {
        if request
            .headers()
            .get(header::ORIGIN)
            .and_then(|s| s.to_str().ok())
            != Some(state.config.public_origin.as_str())
        {
            return ApiError::new(
                StatusCode::BAD_REQUEST,
                "invalid_origin",
                "Origin must match PUBLIC_ORIGIN",
            )
            .into_response();
        }
        if request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|s| s.to_str().ok())
            .and_then(|s| s.split(';').next())
            .map(str::trim)
            != Some("application/json")
        {
            return ApiError::new(
                StatusCode::BAD_REQUEST,
                "json_required",
                "Mutations require Content-Type: application/json",
            )
            .into_response();
        }
    }
    let refresh = if request.uri().path().starts_with("/api/v1/teams")
        || request.uri().path() == "/api/v1/notifications"
    {
        token(request.headers()).map(str::to_owned)
    } else {
        None
    };
    let mut response = next.run(request).await;
    if response.status().is_success()
        && let Some(token) = refresh
    {
        response
            .headers_mut()
            .insert(header::SET_COOKIE, cookie(&state, &token));
    }
    if !response.headers().contains_key(header::CACHE_CONTROL) {
        response
            .headers_mut()
            .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    }
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
}
async fn live() -> Json<Value> {
    Json(json!({"status":"ok"}))
}
async fn ready(State(state): State<AppState>) -> ApiResult<Json<Value>> {
    let revision: i64 = sqlx::query_scalar("SELECT revision FROM catalog_state WHERE singleton")
        .fetch_one(&state.pool)
        .await?;
    if revision == 0 {
        return Err(unavailable());
    }
    Ok(Json(json!({"status":"ready"})))
}
struct CookieSecurity;
impl utoipa::Modify for CookieSecurity {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        if let Some(components) = openapi.components.as_mut() {
            components.add_security_scheme(
                "owner_cookie",
                utoipa::openapi::security::SecurityScheme::ApiKey(
                    utoipa::openapi::security::ApiKey::Cookie(
                        utoipa::openapi::security::ApiKeyValue::new("poketeam_owner"),
                    ),
                ),
            );
        }
    }
}
#[derive(OpenApi)]
#[openapi(info(title="Poketeam API",version="1.0.0",description="Same-origin JSON REST API. All mutations require Origin matching PUBLIC_ORIGIN and Content-Type application/json. Establish identity with POST /api/v1/session and send the HttpOnly cookie on subsequent requests. DELETE accepts {}. Errors use {code,message,details?}. Counter search uses deterministic heuristics, not battle simulation."),paths(identity::session,catalog::catalog,catalog::pokemon,catalog::types,catalog::sprite,catalog::status,teams::teams,teams::create_team,teams::team,teams::update_team,teams::delete_team,teams::counter_team,notifications::notifications),components(schemas(Stats,Pokemon,Session,ErrorBody,CatalogPage,TypeResponse,CatalogStatus,RefreshRun,Team,CreateTeam,UpdateTeam,CounterRequest,CounterResult,Mode,Notification,NotificationPage)),modifiers(&CookieSecurity))]
pub struct ApiDoc;
async fn openapi() -> Json<utoipa::openapi::OpenApi> {
    Json(ApiDoc::openapi())
}
pub async fn shutdown_signal() {
    #[cfg(unix)]
    {
        let mut terminate =
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("signal handler");
        tokio::select! {_=tokio::signal::ctrl_c()=>{},_=terminate.recv()=>{}}
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}
