pub mod pokeapi;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
pub struct Stats {
    pub hp: i32,
    pub attack: i32,
    pub defense: i32,
    pub special_attack: i32,
    pub special_defense: i32,
    pub speed: i32,
}
impl Stats {
    pub fn total(&self) -> i32 {
        self.hp
            + self.attack
            + self.defense
            + self.special_attack
            + self.special_defense
            + self.speed
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, ToSchema)]
pub struct Pokemon {
    pub id: i32,
    pub species_id: i32,
    pub name: String,
    pub types: Vec<String>,
    pub stats: Stats,
    pub base_stat_total: i32,
    pub sprite_url: Option<String>,
    pub active: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CatalogPokemon {
    pub id: i32,
    pub species_id: i32,
    pub name: String,
    pub types: Vec<String>,
    pub stats: Stats,
    pub sprite_source: Option<String>,
    pub sprite_hash: Option<String>,
    pub active: bool,
}
impl CatalogPokemon {
    pub fn public(&self) -> Pokemon {
        Pokemon {
            id: self.id,
            species_id: self.species_id,
            name: self.name.clone(),
            types: self.types.clone(),
            stats: self.stats.clone(),
            base_stat_total: self.stats.total(),
            sprite_url: self
                .sprite_hash
                .as_ref()
                .map(|h| format!("/api/v1/sprites/{h}")),
            active: self.active,
        }
    }
}
pub type TypeChart = BTreeMap<String, BTreeMap<String, f64>>;

pub fn from_row(row: &sqlx::postgres::PgRow) -> Result<CatalogPokemon, sqlx::Error> {
    use sqlx::Row;
    let stats: sqlx::types::Json<Stats> = row.try_get("stats")?;
    Ok(CatalogPokemon {
        id: row.try_get("id")?,
        species_id: row.try_get("species_id")?,
        name: row.try_get("name")?,
        types: row.try_get("types")?,
        stats: stats.0,
        sprite_source: row.try_get("sprite_source")?,
        sprite_hash: row.try_get("sprite_hash")?,
        active: row.try_get("active")?,
    })
}
