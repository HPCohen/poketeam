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
    pub generation: Resource,
    pub is_legendary: bool,
    pub is_mythical: bool,
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
    #[serde(flatten)]
    pub other: BTreeMap<String, serde_json::Value>,
}
impl Sprites {
    /// Preserve stable upstream paths, including future game/artwork groups.
    pub fn variants(&self) -> BTreeMap<String, String> {
        fn collect(path: &str, value: &serde_json::Value, variants: &mut BTreeMap<String, String>) {
            match value {
                serde_json::Value::String(url)
                    if url.starts_with("https://") || url.starts_with("http://") =>
                {
                    variants.insert(path.to_owned(), url.clone());
                }
                serde_json::Value::Object(group) => {
                    for (name, value) in group {
                        collect(&format!("{path}/{name}"), value, variants);
                    }
                }
                _ => {}
            }
        }
        let mut variants = BTreeMap::new();
        if let Some(url) = &self.front_default {
            variants.insert("front_default".into(), url.clone());
        }
        for (name, value) in &self.other {
            collect(name, value, &mut variants);
        }
        variants
    }
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

/// First-partner evolution families through generation IX, plus Pikachu and Eevee.
/// Species IDs keep alternate default varieties in the same family. New families require a list update.
pub fn is_starter_family(species_id: i32) -> bool {
    matches!(species_id,
        1..=9 | 152..=160 | 252..=260 | 387..=395 | 495..=503 |
        650..=658 | 722..=730 | 810..=818 | 906..=914 |
        25 | 26 | 172 | 133..=136 | 196 | 197 | 470 | 471 | 700
    )
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
        let sprite_variants = raw.sprites.variants();
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
            generation: None,
            is_starter: false,
            is_legendary: false,
            is_mythical: false,
            sprite_source: raw.sprites.front_default,
            sprite_hash: None,
            sprite_variants,
            active: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{Sprites, is_starter_family};
    use serde_json::json;

    #[test]
    fn starter_families_include_evolutions_and_special_first_partners() {
        for species in [
            1, 3, 6, 9, 152, 160, 260, 395, 503, 658, 730, 818, 914, 25, 26, 172, 133, 136, 196,
            197, 470, 471, 700,
        ] {
            assert!(is_starter_family(species), "missing {species}");
        }
        for species in [10, 24, 27, 132, 137, 150, 151, 161, 251, 915] {
            assert!(!is_starter_family(species), "unexpected {species}");
        }
    }

    #[test]
    fn sprite_paths_include_all_non_null_formats_and_nested_versions() {
        let sprites: Sprites = serde_json::from_value(json!({
            "front_default": "https://example.com/front.png",
            "front_shiny": "https://example.com/shiny.png",
            "back_female": "https://example.com/back-female.png",
            "front_female": null,
            "other": {
                "dream_world": {"front_default": "https://example.com/art.svg"},
                "showdown": {"front_default": "https://example.com/animated.gif"}
            },
            "versions": {"generation-i": {"red-blue": {"back_default": "https://example.com/red-back.png"}}}
        })).unwrap();
        let variants = sprites.variants();
        assert_eq!(variants.len(), 6);
        assert_eq!(variants["front_default"], "https://example.com/front.png");
        assert_eq!(
            variants["other/dream_world/front_default"],
            "https://example.com/art.svg"
        );
        assert_eq!(
            variants["other/showdown/front_default"],
            "https://example.com/animated.gif"
        );
        assert_eq!(
            variants["versions/generation-i/red-blue/back_default"],
            "https://example.com/red-back.png"
        );
        assert!(!variants.contains_key("front_female"));
    }
}
