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
    #[serde(default)]
    pub generation: Option<i32>,
    #[serde(default)]
    pub is_starter: bool,
    #[serde(default)]
    pub is_legendary: bool,
    #[serde(default)]
    pub is_mythical: bool,
    pub base_stat_total: i32,
    pub sprite_url: Option<String>,
    #[serde(default)]
    pub sprite_variants: BTreeMap<String, String>,
    pub selected_sprite: Option<String>,
    pub active: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CatalogPokemon {
    pub id: i32,
    pub species_id: i32,
    pub name: String,
    pub types: Vec<String>,
    pub stats: Stats,
    #[serde(default)]
    pub generation: Option<i32>,
    #[serde(default)]
    pub is_starter: bool,
    #[serde(default)]
    pub is_legendary: bool,
    #[serde(default)]
    pub is_mythical: bool,
    pub sprite_source: Option<String>,
    pub sprite_hash: Option<String>,
    #[serde(default)]
    pub sprite_variants: BTreeMap<String, String>,
    pub active: bool,
}
impl CatalogPokemon {
    pub fn public(&self) -> Pokemon {
        let sprite_url = self
            .sprite_hash
            .as_ref()
            .map(|hash| format!("/api/v1/sprites/{hash}"));
        let mut sprite_variants = self.sprite_variants.clone();
        if let Some(url) = &sprite_url {
            sprite_variants.insert("front_default".into(), url.clone());
        }
        Pokemon {
            id: self.id,
            species_id: self.species_id,
            name: self.name.clone(),
            types: self.types.clone(),
            stats: self.stats.clone(),
            generation: self.generation,
            is_starter: self.is_starter,
            is_legendary: self.is_legendary,
            is_mythical: self.is_mythical,
            base_stat_total: self.stats.total(),
            sprite_url,
            sprite_variants,
            selected_sprite: None,
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
        generation: row.try_get("generation")?,
        is_starter: row.try_get("is_starter")?,
        is_legendary: row.try_get("is_legendary")?,
        is_mythical: row.try_get("is_mythical")?,
        sprite_source: row.try_get("sprite_source")?,
        sprite_hash: row.try_get("sprite_hash")?,
        sprite_variants: row
            .try_get::<sqlx::types::Json<BTreeMap<String, String>>, _>("sprite_variants")?
            .0,
        active: row.try_get("active")?,
    })
}
