//! Opt-in operator edits used to demonstrate real catalog change notifications.
use crate::{
    AppState,
    http::{ApiError, ApiJson, ApiPath, ApiResult, ErrorBody},
    model::{self, Pokemon, Stats},
    notifications::{differences, record_change},
};
use axum::{
    Json,
    extract::State,
    http::{HeaderMap, StatusCode, header},
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use utoipa::ToSchema;

#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct StatUpdates {
    pub hp: Option<i32>,
    pub attack: Option<i32>,
    pub defense: Option<i32>,
    pub special_attack: Option<i32>,
    pub special_defense: Option<i32>,
    pub speed: Option<i32>,
}
impl StatUpdates {
    fn validate(&self) -> ApiResult<()> {
        let values = [
            self.hp,
            self.attack,
            self.defense,
            self.special_attack,
            self.special_defense,
            self.speed,
        ];
        if values.iter().all(Option::is_none) {
            return Err(ApiError::invalid("Supply at least one base stat"));
        }
        if values
            .into_iter()
            .flatten()
            .any(|value| !(1..=1000).contains(&value))
        {
            return Err(ApiError::invalid(
                "Base stats must be integers from 1 to 1000",
            ));
        }
        Ok(())
    }
    fn apply(self, stats: &mut Stats) {
        if let Some(value) = self.hp {
            stats.hp = value;
        }
        if let Some(value) = self.attack {
            stats.attack = value;
        }
        if let Some(value) = self.defense {
            stats.defense = value;
        }
        if let Some(value) = self.special_attack {
            stats.special_attack = value;
        }
        if let Some(value) = self.special_defense {
            stats.special_defense = value;
        }
        if let Some(value) = self.speed {
            stats.speed = value;
        }
    }
}
#[derive(Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdatePokemon {
    pub stats: StatUpdates,
}
#[derive(Serialize, ToSchema)]
pub struct UpdateResult {
    pub pokemon: Pokemon,
    pub catalog_revision: i64,
    /// Null when the submitted values already match the catalog.
    pub change_id: Option<i64>,
}

#[utoipa::path(patch,path="/api/v1/demo/pokemon/{id}",params(("id"=i32,Path)),request_body=UpdatePokemon,responses((status=200,body=UpdateResult),(status=400,body=ErrorBody),(status=401,body=ErrorBody),(status=404,body=ErrorBody),(status=422,body=ErrorBody),(status=503,body=ErrorBody)),security(("demo_operator"=[])),tag="demo")]
pub(crate) async fn update_pokemon(
    State(state): State<AppState>,
    headers: HeaderMap,
    ApiPath(id): ApiPath<i32>,
    ApiJson(update): ApiJson<UpdatePokemon>,
) -> ApiResult<Json<UpdateResult>> {
    let expected = state
        .config
        .demo_update_token
        .as_ref()
        .ok_or_else(ApiError::not_found)?;
    let provided = headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    let provided_hash = Sha256::digest(provided.as_bytes());
    let expected_hash = Sha256::digest(expected.as_bytes());
    if !bool::from(provided_hash[..].ct_eq(&expected_hash[..])) {
        return Err(ApiError::new(
            StatusCode::UNAUTHORIZED,
            "demo_auth_required",
            "A valid demo operator token is required",
        ));
    }
    update.stats.validate()?;
    let mut transaction = state.pool.begin().await?;
    // Match sync's lock order so stats, revision and event describe one atomic edit.
    let (revision, initialized): (i64, bool) = sqlx::query_as(
        "SELECT revision,last_success_at IS NOT NULL FROM catalog_state WHERE singleton FOR UPDATE",
    )
    .fetch_one(&mut *transaction)
    .await?;
    if !initialized {
        return Err(ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "catalog_unavailable",
            "Complete the initial catalog sync before demo updates",
        ));
    }
    let row = sqlx::query("SELECT * FROM pokemon WHERE id=$1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *transaction)
        .await?
        .ok_or_else(ApiError::not_found)?;
    let before = model::from_row(&row)?;
    let mut after = before.clone();
    update.stats.apply(&mut after.stats);
    let changes = differences(Some(&before), &after)?;
    let (catalog_revision, change_id) = if changes.is_empty() {
        (revision, None)
    } else {
        sqlx::query("UPDATE pokemon SET stats=$2 WHERE id=$1")
            .bind(id)
            .bind(sqlx::types::Json(&after.stats))
            .execute(&mut *transaction)
            .await?;
        let change_id =
            record_change(&mut transaction, id, None, state.clock.now(), changes).await?;
        let revision = sqlx::query_scalar(
            "UPDATE catalog_state SET revision=revision+1 WHERE singleton RETURNING revision",
        )
        .fetch_one(&mut *transaction)
        .await?;
        (revision, change_id)
    };
    transaction.commit().await?;
    Ok(Json(UpdateResult {
        pokemon: after.public(),
        catalog_revision,
        change_id,
    }))
}
