//! Serde models for the upstream PokéAPI responses.
use super::{CatalogPokemon, Stats};
use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use url::Url;

#[derive(Deserialize)]
pub struct Resource {
    pub name: String,
    pub url: String,
}
#[derive(Deserialize)]
pub struct Page {
    pub count: usize,
    pub next: Option<String>,
    pub results: Vec<Resource>,
}
#[derive(Deserialize)]
pub struct Species {
    pub id: i32,
    pub varieties: Vec<Variety>,
}
#[derive(Deserialize)]
pub struct Variety {
    pub is_default: bool,
    pub pokemon: Resource,
}
#[derive(Deserialize)]
pub struct Pokemon {
    pub id: i32,
    pub name: String,
    pub is_default: bool,
    pub species: Resource,
    pub types: Vec<TypeSlot>,
    pub stats: Vec<Stat>,
    pub sprites: Sprites,
}
#[derive(Deserialize)]
pub struct TypeSlot {
    pub slot: usize,
    #[serde(rename = "type")]
    pub kind: Resource,
}
#[derive(Deserialize)]
pub struct Stat {
    pub base_stat: i32,
    pub stat: Resource,
}
#[derive(Deserialize)]
pub struct Sprites {
    pub front_default: Option<String>,
}
#[derive(Deserialize)]
pub struct Type {
    pub name: String,
    pub damage_relations: DamageRelations,
}
#[derive(Deserialize)]
pub struct DamageRelations {
    pub no_damage_to: Vec<Resource>,
    pub half_damage_to: Vec<Resource>,
    pub double_damage_to: Vec<Resource>,
}

impl Resource {
    pub fn id(&self) -> Result<i32> {
        Url::parse(&self.url)?
            .path()
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .context("missing resource id")?
            .parse()
            .context("invalid resource id")
    }
}

impl TryFrom<Pokemon> for CatalogPokemon {
    type Error = anyhow::Error;

    fn try_from(raw: Pokemon) -> Result<Self> {
        ensure!(raw.id > 0 && raw.is_default, "invalid default Pokémon");
        let mut types = raw.types;
        types.sort_by_key(|t| t.slot);
        ensure!(
            (1..=2).contains(&types.len())
                && types.iter().enumerate().all(|(i, t)| t.slot == i + 1)
                && types
                    .iter()
                    .map(|t| &t.kind.name)
                    .collect::<BTreeSet<_>>()
                    .len()
                    == types.len(),
            "invalid type slots"
        );
        let stats: BTreeMap<_, _> = raw
            .stats
            .iter()
            .map(|s| (s.stat.name.as_str(), s.base_stat))
            .collect();
        ensure!(
            stats.len() == 6
                && raw.stats.len() == 6
                && stats.values().all(|n| *n > 0 && *n <= 1000),
            "invalid base stats"
        );
        let stat = |name| stats.get(name).copied().context("missing stat");
        Ok(Self {
            id: raw.id,
            species_id: raw.species.id()?,
            name: raw.name,
            types: types.into_iter().map(|t| t.kind.name).collect(),
            stats: Stats {
                hp: stat("hp")?,
                attack: stat("attack")?,
                defense: stat("defense")?,
                special_attack: stat("special-attack")?,
                special_defense: stat("special-defense")?,
                speed: stat("speed")?,
            },
            sprite_source: raw.sprites.front_default,
            sprite_hash: None,
            active: true,
        })
    }
}
