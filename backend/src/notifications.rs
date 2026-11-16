use crate::{
    AppState,
    catalog::{page_size, snapshot, validate_page},
    http::{ApiQuery, ApiResult},
    identity::owner,
    model::{self, Pokemon},
};
use axum::{Json, extract::State, http::HeaderMap};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Deserialize)]
pub struct NotificationQuery {
    #[serde(default = "page_size")]
    limit: u32,
    #[serde(default)]
    offset: u32,
}
#[derive(Serialize, ToSchema)]
pub struct Notification {
    id: i64,
    pokemon: Pokemon,
    affected_team_ids: Vec<Uuid>,
    detected_at: DateTime<Utc>,
    differences: Value,
    descriptions: Vec<String>,
}
#[derive(Serialize, ToSchema)]
pub struct NotificationPage {
    notifications: Vec<Notification>,
    total: i64,
    limit: u32,
    offset: u32,
}
#[utoipa::path(get,path="/api/v1/notifications",params(("limit"=Option<u32>,Query),("offset"=Option<u32>,Query)),responses((status=200,body=NotificationPage),(status=400,body=crate::http::ErrorBody),(status=401,body=crate::http::ErrorBody),(status=422,body=crate::http::ErrorBody),(status=503,body=crate::http::ErrorBody)),security(("owner_cookie"=[])),tag="notifications")]
pub(crate) async fn notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiQuery(q): ApiQuery<NotificationQuery>,
) -> ApiResult<Json<NotificationPage>> {
    let owner = owner(&state, &headers).await?;
    validate_page(q.limit, q.offset)?;
    let now = state.clock.now();
    let since = now - Duration::days(7);
    let mut tx = snapshot(&state).await?;
    let total: i64 = sqlx::query_scalar(
        r#"
        SELECT count(*)
        FROM pokemon_changes c
        WHERE c.detected_at >= $2
          AND c.detected_at <= $3
          AND EXISTS (
              SELECT 1
              FROM team_members m
              JOIN teams t ON t.id = m.team_id
              WHERE m.pokemon_id = c.pokemon_id AND t.owner_id = $1
          )
        "#,
    )
    .bind(owner)
    .bind(since)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;
    let rows = sqlx::query(
        r#"
        SELECT c.id AS event_id, c.detected_at, c.differences, p.*,
            ARRAY(
                SELECT t.id
                FROM teams t
                JOIN team_members m ON m.team_id = t.id
                WHERE t.owner_id = $1 AND m.pokemon_id = c.pokemon_id
                ORDER BY t.id
            ) AS team_ids
        FROM pokemon_changes c
        JOIN pokemon p ON p.id = c.pokemon_id
        WHERE c.detected_at >= $2
          AND c.detected_at <= $3
          AND EXISTS (
              SELECT 1
              FROM team_members m
              JOIN teams t ON t.id = m.team_id
              WHERE m.pokemon_id = c.pokemon_id AND t.owner_id = $1
          )
        ORDER BY c.detected_at DESC,c.id DESC
        LIMIT $4
        OFFSET $5
        "#,
    )
    .bind(owner)
    .bind(since)
    .bind(now)
    .bind(i64::from(q.limit))
    .bind(i64::from(q.offset))
    .fetch_all(&mut *tx)
    .await?;
    let mut notifications = Vec::new();
    for row in rows {
        let pokemon = model::from_row(&row)?.public();
        let differences: Value = row.try_get("differences")?;
        let descriptions = describe(&pokemon.name, &differences);
        notifications.push(Notification {
            id: row.try_get("event_id")?,
            pokemon,
            affected_team_ids: row.try_get("team_ids")?,
            detected_at: row.try_get("detected_at")?,
            differences,
            descriptions,
        });
    }
    Ok(Json(NotificationPage {
        notifications,
        total,
        limit: q.limit,
        offset: q.offset,
    }))
}

fn describe(name: &str, differences: &Value) -> Vec<String> {
    let mut descriptions = Vec::new();
    for (key, change) in differences.as_object().into_iter().flat_map(|m| m.iter()) {
        match key.as_str() {
            "stats" => {
                for (stat, after) in change["after"]
                    .as_object()
                    .into_iter()
                    .flat_map(|m| m.iter())
                {
                    let before = &change["before"][stat];
                    if before != after {
                        descriptions.push(format!(
                            "{name}: {} changed from {before} to {after}",
                            stat.replace('_', " ")
                        ));
                    }
                }
            }
            "sprite_variants" => {
                descriptions.push(format!("{name}: available sprite versions changed"))
            }
            "sprite_hash" => descriptions.push(format!("{name}: sprite image content changed")),
            "sprite_source" => descriptions.push(format!("{name}: upstream sprite URL changed")),
            "active" => descriptions.push(format!(
                "{name}: {}",
                if change["after"] == true {
                    "available again"
                } else {
                    "no longer available for new selections"
                }
            )),
            "availability" => descriptions.push(format!("{name}: added to the catalog")),
            "type_effectiveness" => {
                for relation in change.as_array().into_iter().flatten() {
                    descriptions.push(format!(
                        "{name}: {} attacks against {} changed from {}× to {}×",
                        relation["attacking"].as_str().unwrap_or("unknown"),
                        relation["defending"].as_str().unwrap_or("unknown"),
                        relation["before"],
                        relation["after"]
                    ));
                }
            }
            _ => descriptions.push(format!(
                "{name}: {} changed from {} to {}",
                key.replace('_', " "),
                change["before"],
                change["after"]
            )),
        }
    }
    descriptions
}
