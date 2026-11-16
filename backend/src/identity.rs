pub mod sessions;

use crate::{
    AppState,
    http::{ApiError, ApiJson, ApiResult},
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
};
use serde::Serialize;
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;

pub(crate) fn token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|h| h.to_str().ok())
        .flat_map(|s| s.split(';'))
        .filter_map(|s| s.trim().split_once('='))
        .find(|(name, _)| *name == "poketeam_owner")
        .map(|(_, v)| v)
        .filter(|t| t.len() == 64 && t.bytes().all(|b| b.is_ascii_hexdigit()))
}
pub(crate) fn cookie(state: &AppState, token: &str) -> HeaderValue {
    HeaderValue::from_str(&format!(
        "poketeam_owner={token}; Path=/; HttpOnly; SameSite=Lax; Max-Age=31536000{}",
        if state.config.secure_cookie {
            "; Secure"
        } else {
            ""
        }
    ))
    .expect("hex token cookie")
}
pub(crate) async fn owner(state: &AppState, headers: &HeaderMap) -> ApiResult<Uuid> {
    let token = token(headers).ok_or_else(|| {
        ApiError::new(
            StatusCode::UNAUTHORIZED,
            "identity_required",
            "Establish a session first",
        )
    })?;
    sessions::authenticate(state, token)
        .await?
        .map(|session| session.owner_id)
        .ok_or_else(|| {
            ApiError::new(
                StatusCode::UNAUTHORIZED,
                "identity_required",
                "Establish a session first",
            )
        })
}
#[derive(Serialize, ToSchema)]
pub struct Session {
    pub owner_id: Uuid,
}
#[utoipa::path(post,path="/api/v1/session",request_body=Object,responses((status=200,body=Session),(status=400,body=crate::http::ErrorBody),(status=503,body=crate::http::ErrorBody)),tag="identity")]
pub(crate) async fn session(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiJson(_): ApiJson<serde_json::Map<String, Value>>,
) -> ApiResult<Response> {
    let (id, session_cookie) = match owner(&state, &headers).await {
        Ok(id) => (
            id,
            cookie(&state, token(&headers).expect("authenticated token")),
        ),
        Err(ApiError(status, _)) if status == StatusCode::UNAUTHORIZED => {
            let id = Uuid::new_v4();
            let now = state.clock.now();
            let mut transaction = state.pool.begin().await?;
            sqlx::query("INSERT INTO owners(id,created_at,last_seen_at) VALUES($1,$2,$2)")
                .bind(id)
                .bind(now)
                .execute(&mut *transaction)
                .await?;
            let issued = sessions::insert(&mut transaction, id, now).await?;
            transaction.commit().await?;
            (id, issued.cookie_header(&state))
        }
        Err(e) => return Err(e),
    };
    let mut response = Json(Session { owner_id: id }).into_response();
    response
        .headers_mut()
        .insert(header::SET_COOKIE, session_cookie);
    response
        .headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}
