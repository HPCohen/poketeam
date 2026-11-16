//! Application sessions shared by anonymous entry and future trusted auth callbacks.
//!
//! Provider proof verification and identity-to-owner resolution belong upstream of
//! this module. Never expose session issuance as an endpoint accepting an owner ID.
use crate::AppState;
use axum::http::HeaderValue;
use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Result};
use uuid::Uuid;

/// A validated device session. The owner is the stable application user.
pub struct AuthenticatedSession {
    pub id: Uuid,
    pub owner_id: Uuid,
}

/// Newly issued cookie material; intentionally neither Debug nor Serialize.
pub struct IssuedSession {
    pub id: Uuid,
    pub owner_id: Uuid,
    token: String,
}
impl IssuedSession {
    pub fn cookie_header(&self, state: &AppState) -> HeaderValue {
        super::cookie(state, &self.token)
    }
}

fn hash(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub(super) async fn insert(
    connection: &mut PgConnection,
    owner_id: Uuid,
    now: DateTime<Utc>,
) -> Result<IssuedSession> {
    let session = IssuedSession {
        id: Uuid::new_v4(),
        owner_id,
        token: hex::encode(rand::random::<[u8; 32]>()),
    };
    sqlx::query("INSERT INTO owner_sessions(id,owner_id,token_hash,created_at,last_seen_at) VALUES($1,$2,$3,$4,$4)")
        .bind(session.id).bind(owner_id).bind(hash(&session.token)).bind(now)
        .execute(connection).await?;
    Ok(session)
}

/// Issue a device credential after trusted backend code has established its owner.
/// This does not validate a provider token or link/merge accounts.
pub async fn issue(state: &AppState, owner_id: Uuid) -> Result<IssuedSession> {
    insert(
        &mut *state.pool.acquire().await?,
        owner_id,
        state.clock.now(),
    )
    .await
}

pub(super) async fn authenticate(
    state: &AppState,
    token: &str,
) -> Result<Option<AuthenticatedSession>> {
    let now = state.clock.now();
    let row: Option<(Uuid, Uuid)> = sqlx::query_as(
        "WITH active_session AS (
            UPDATE owner_sessions SET last_seen_at=$2
            WHERE token_hash=$1 AND revoked_at IS NULL AND last_seen_at >= $3
            RETURNING id,owner_id
        )
        UPDATE owners SET last_seen_at=$2 FROM active_session
        WHERE owners.id=active_session.owner_id
        RETURNING active_session.id,owners.id",
    )
    .bind(hash(token))
    .bind(now)
    .bind(now - Duration::days(365))
    .fetch_optional(&state.pool)
    .await?;
    Ok(row.map(|(id, owner_id)| AuthenticatedSession { id, owner_id }))
}

/// Revoke only a session belonging to the caller's already authenticated owner.
/// Future logout/device-management handlers must supply that owner server-side.
pub async fn revoke(state: &AppState, owner_id: Uuid, session_id: Uuid) -> Result<bool> {
    let result = sqlx::query(
        "UPDATE owner_sessions SET revoked_at=$3 WHERE id=$1 AND owner_id=$2 AND revoked_at IS NULL",
    )
    .bind(session_id)
    .bind(owner_id)
    .bind(state.clock.now())
    .execute(&state.pool)
    .await?;
    Ok(result.rows_affected() == 1)
}
