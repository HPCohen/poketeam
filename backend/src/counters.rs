//! Deterministic, bounded local search. No database or HTTP dependencies.
use crate::model::{Pokemon, TypeChart};
use serde::{Deserialize, Serialize};
use std::{cmp::Ordering, collections::BTreeSet};
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Power,
    Rival,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct OpponentExplanation {
    pub opponent_id: i32,
    pub best_attacker_id: i32,
    pub attacking_type: String,
    pub effectiveness: f64,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct SharedWeakness {
    pub attacking_type: String,
    pub pokemon_ids: Vec<i32>,
}
#[derive(Debug, Serialize, ToSchema)]
pub struct CounterResult {
    pub mode: Mode,
    pub algorithm_version: String,
    pub pokemon: Vec<Pokemon>,
    pub source_total: i32,
    pub source_average: f64,
    pub counter_total: i32,
    pub counter_average: f64,
    pub percentage_stat_difference: f64,
    pub coverage: usize,
    pub matchup_score: f64,
    pub shared_weaknesses: Vec<SharedWeakness>,
    pub explanations: Vec<OpponentExplanation>,
    pub catalog_revision: i64,
}
pub fn effectiveness(attack: &str, defender: &Pokemon, chart: &TypeChart) -> f64 {
    defender
        .types
        .iter()
        .map(|d| {
            chart
                .get(attack)
                .and_then(|r| r.get(d))
                .copied()
                .unwrap_or(1.0)
        })
        .product()
}
fn best_attack<'a>(attacker: &'a Pokemon, defender: &Pokemon, chart: &TypeChart) -> (&'a str, f64) {
    attacker
        .types
        .iter()
        .map(|t| (t.as_str(), effectiveness(t, defender, chart)))
        .max_by(|a, b| a.1.total_cmp(&b.1).then_with(|| b.0.cmp(a.0)))
        .unwrap_or(("unknown", 1.0))
}
fn evaluate(team: &[Pokemon], source: &[Pokemon], chart: &TypeChart) -> (usize, f64) {
    let hits: Vec<f64> = source
        .iter()
        .map(|d| {
            team.iter()
                .map(|a| best_attack(a, d, chart).1)
                .fold(0.0, f64::max)
        })
        .collect();
    let penalty: usize = chart
        .keys()
        .map(|t| {
            let n = team
                .iter()
                .filter(|p| effectiveness(t, p, chart) > 1.0)
                .count();
            n * n.saturating_sub(1) / 2
        })
        .sum();
    (
        hits.iter().filter(|&&x| x > 1.0).count(),
        hits.iter().sum::<f64>() - 0.25 * penalty as f64,
    )
}
fn total(team: &[Pokemon]) -> i32 {
    team.iter().map(|p| p.base_stat_total).sum()
}
fn ids(team: &[Pokemon]) -> Vec<i32> {
    let mut ids: Vec<_> = team.iter().map(|p| p.id).collect();
    ids.sort_unstable();
    ids
}
fn better(a: &[Pokemon], b: &[Pokemon], source: &[Pokemon], chart: &TypeChart, mode: Mode) -> bool {
    let (ac, am) = evaluate(a, source, chart);
    let (bc, bm) = evaluate(b, source, chart);
    let order = ac
        .cmp(&bc)
        .then_with(|| match mode {
            Mode::Power => total(a).cmp(&total(b)).then_with(|| am.total_cmp(&bm)),
            Mode::Rival => am.total_cmp(&bm).then_with(|| {
                (total(b) - total(source))
                    .abs()
                    .cmp(&(total(a) - total(source)).abs())
            }),
        })
        .then_with(|| ids(b).cmp(&ids(a)));
    order == Ordering::Greater
}
pub fn balanced(candidate: i32, source: i32) -> bool {
    source > 0 && i64::from((candidate - source).abs()) * 100 <= i64::from(source) * 5
}

pub fn generate(
    source: &[Pokemon],
    candidates: &[Pokemon],
    chart: &TypeChart,
    mode: Mode,
    revision: i64,
) -> Result<CounterResult, &'static str> {
    if source.is_empty() {
        return Err("empty_source_team");
    }
    let mut candidates: Vec<Pokemon> = candidates.iter().filter(|p| p.active).cloned().collect();
    candidates.sort_by_key(|p| p.id);
    candidates.dedup_by_key(|p| p.id);
    if candidates.len() < source.len() {
        return Err("insufficient_active_pokemon");
    }
    let mut team = Vec::new();
    if mode == Mode::Rival {
        // Reserve all still-active source members before replacing unavailable ones.
        let mut used: BTreeSet<i32> = source
            .iter()
            .filter(|s| candidates.iter().any(|p| p.id == s.id))
            .map(|p| p.id)
            .collect();
        for s in source {
            if let Some(p) = candidates.iter().find(|p| p.id == s.id) {
                team.push(p.clone());
            } else {
                let p = candidates
                    .iter()
                    .filter(|p| !used.contains(&p.id))
                    .min_by_key(|p| ((p.base_stat_total - s.base_stat_total).abs(), p.id))
                    .ok_or("rival_balance_not_found")?;
                used.insert(p.id);
                team.push(p.clone());
            }
        }
        if !balanced(total(&team), total(source)) {
            return Err("rival_balance_not_found");
        }
    } else {
        while team.len() < source.len() {
            let mut best: Option<Vec<Pokemon>> = None;
            for p in &candidates {
                if team.iter().any(|t: &Pokemon| t.id == p.id) {
                    continue;
                }
                let mut next = team.clone();
                next.push(p.clone());
                if best
                    .as_ref()
                    .is_none_or(|b| better(&next, b, source, chart, mode))
                {
                    best = Some(next);
                }
            }
            team = best.ok_or("insufficient_active_pokemon")?;
        }
    }
    for _ in 0..10 {
        let mut best = team.clone();
        for i in 0..team.len() {
            for p in &candidates {
                if team.iter().any(|t| t.id == p.id) {
                    continue;
                }
                let mut next = team.clone();
                next[i] = p.clone();
                if mode == Mode::Rival && !balanced(total(&next), total(source)) {
                    continue;
                }
                if better(&next, &best, source, chart, mode) {
                    best = next;
                }
            }
        }
        if ids(&best) == ids(&team) {
            break;
        }
        team = best;
    }
    team.sort_by_key(|p| p.id);
    let (coverage, matchup_score) = evaluate(&team, source, chart);
    let explanations = source
        .iter()
        .map(|d| {
            let (p, (t, m)) = team
                .iter()
                .map(|p| (p, best_attack(p, d, chart)))
                .max_by(|a, b| a.1.1.total_cmp(&b.1.1).then_with(|| b.0.id.cmp(&a.0.id)))
                .expect("nonempty team");
            OpponentExplanation {
                opponent_id: d.id,
                best_attacker_id: p.id,
                attacking_type: t.into(),
                effectiveness: m,
            }
        })
        .collect();
    let shared_weaknesses = chart
        .keys()
        .filter_map(|t| {
            let pokemon_ids: Vec<_> = team
                .iter()
                .filter(|p| effectiveness(t, p, chart) > 1.0)
                .map(|p| p.id)
                .collect();
            (pokemon_ids.len() > 1).then(|| SharedWeakness {
                attacking_type: t.clone(),
                pokemon_ids,
            })
        })
        .collect();
    let source_total = total(source);
    let counter_total = total(&team);
    let n = team.len() as f64;
    Ok(CounterResult {
        mode,
        algorithm_version: "type-greedy-v1".into(),
        pokemon: team,
        source_total,
        source_average: source_total as f64 / n,
        counter_total,
        counter_average: counter_total as f64 / n,
        percentage_stat_difference: 100.0 * (counter_total - source_total) as f64
            / source_total as f64,
        coverage,
        matchup_score,
        shared_weaknesses,
        explanations,
        catalog_revision: revision,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::Stats;
    fn p(id: i32, t: &[&str], stat: i32) -> Pokemon {
        Pokemon {
            id,
            species_id: id,
            name: format!("p{id}"),
            types: t.iter().map(|s| s.to_string()).collect(),
            stats: Stats {
                hp: stat,
                attack: stat,
                defense: stat,
                special_attack: stat,
                special_defense: stat,
                speed: stat,
            },
            base_stat_total: stat * 6,
            generation: None,
            is_starter: false,
            is_legendary: false,
            is_mythical: false,
            sprite_url: None,
            sprite_variants: Default::default(),
            selected_sprite: None,
            active: true,
        }
    }
    fn chart() -> TypeChart {
        serde_json::from_value(serde_json::json!({"fire":{"grass":2.0,"water":0.5},"water":{"fire":2.0,"grass":0.5},"electric":{"water":2.0,"ground":0.0},"grass":{"water":2.0,"fire":0.5}})).unwrap()
    }
    #[test]
    fn multiplication_and_immunity() {
        assert_eq!(
            effectiveness("electric", &p(1, &["water", "ground"], 50), &chart()),
            0.0
        );
        assert_eq!(
            effectiveness("fire", &p(1, &["grass", "water"], 50), &chart()),
            1.0
        );
    }
    #[test]
    fn power_prefers_coverage_then_stats() {
        let s = vec![p(1, &["grass"], 50)];
        let c = vec![
            p(2, &["water"], 100),
            p(3, &["fire"], 60),
            p(4, &["fire"], 90),
        ];
        let r = generate(&s, &c, &chart(), Mode::Power, 1).unwrap();
        assert_eq!(r.pokemon[0].id, 4);
        assert_eq!(r.coverage, 1);
    }
    #[test]
    fn rival_boundaries() {
        assert!(balanced(105, 100));
        assert!(balanced(95, 100));
        assert!(!balanced(106, 100));
        assert!(!balanced(94, 100));
    }
    #[test]
    fn deterministic_unique_equal_size() {
        let s = vec![p(1, &["grass"], 50), p(2, &["water"], 50)];
        let mut c = s.clone();
        c.extend([p(3, &["fire"], 50), p(4, &["electric"], 50)]);
        for mode in [Mode::Power, Mode::Rival] {
            let a = generate(&s, &c, &chart(), mode, 1).unwrap();
            c.reverse();
            let b = generate(&s, &c, &chart(), mode, 1).unwrap();
            assert_eq!(ids(&a.pokemon), ids(&b.pokemon));
            assert_eq!(a.pokemon.len(), 2);
            assert_ne!(a.pokemon[0].id, a.pokemon[1].id);
            assert_eq!(a.counter_average, a.counter_total as f64 / 2.0);
        }
    }
    #[test]
    fn unavailable_seed_and_failure() {
        let mut s = vec![p(1, &["grass"], 50), p(2, &["water"], 50)];
        s[0].active = false;
        let c = vec![s[1].clone(), p(3, &["fire"], 51)];
        assert!(generate(&s, &c, &chart(), Mode::Rival, 1).is_ok());
        let c = vec![p(4, &["fire"], 100), p(5, &["water"], 100)];
        assert_eq!(
            generate(&s, &c, &chart(), Mode::Rival, 1).unwrap_err(),
            "rival_balance_not_found"
        );
    }
    #[test]
    fn empty_rejected_and_power_default() {
        assert_eq!(Mode::default(), Mode::Power);
        assert_eq!(
            generate(&[], &[], &chart(), Mode::Power, 0).unwrap_err(),
            "empty_source_team"
        );
    }
}

#[cfg(test)]
mod scoring_tests {
    use super::*;
    use crate::model::Stats;
    fn pokemon(id: i32, types: &[&str], stat: i32) -> Pokemon {
        Pokemon {
            id,
            species_id: id,
            name: format!("p{id}"),
            types: types.iter().map(|t| t.to_string()).collect(),
            stats: Stats {
                hp: stat,
                attack: stat,
                defense: stat,
                special_attack: stat,
                special_defense: stat,
                speed: stat,
            },
            base_stat_total: 6 * stat,
            generation: None,
            is_starter: false,
            is_legendary: false,
            is_mythical: false,
            sprite_url: None,
            sprite_variants: Default::default(),
            selected_sprite: None,
            active: true,
        }
    }
    #[test]
    fn power_stats_outrank_matchup_after_coverage_and_ids_break_ties() {
        let chart: TypeChart = serde_json::from_value(
            serde_json::json!({"fire":{"grass":2.0},"electric":{"grass":2.0,"water":2.0}}),
        )
        .unwrap();
        let source = vec![pokemon(1, &["grass", "water"], 50)];
        let candidates = vec![
            pokemon(3, &["fire"], 100),
            pokemon(2, &["fire"], 100),
            pokemon(4, &["electric"], 50),
        ];
        let result = generate(&source, &candidates, &chart, Mode::Power, 1).unwrap();
        assert_eq!(result.pokemon[0].id, 2);
        assert_eq!(result.coverage, 1);
        assert_eq!(result.explanations[0].effectiveness, 2.0);
    }
    #[test]
    fn own_types_choose_strongest_and_shared_weakness_penalty_counts_pairs() {
        let chart: TypeChart = serde_json::from_value(
            serde_json::json!({"fire":{"grass":2.0},"water":{"fire":2.0,"grass":0.5}}),
        )
        .unwrap();
        let source = vec![pokemon(1, &["grass"], 50)];
        let dual = pokemon(2, &["water", "fire"], 50);
        assert_eq!(best_attack(&dual, &source[0], &chart), ("fire", 2.0));
        let team = vec![
            pokemon(2, &["fire"], 50),
            pokemon(3, &["fire"], 50),
            pokemon(4, &["fire"], 50),
        ];
        assert_eq!(evaluate(&team, &source, &chart), (1, 1.25)); // Three pairs at 0.25 each.
    }
    #[test]
    fn rival_improves_coverage_while_preserving_balance() {
        let chart: TypeChart =
            serde_json::from_value(serde_json::json!({"fire":{"grass":2.0},"grass":{"fire":0.5}}))
                .unwrap();
        let source = vec![pokemon(1, &["grass"], 50)];
        let candidates = vec![
            source[0].clone(),
            pokemon(2, &["fire"], 51),
            pokemon(3, &["fire"], 100),
        ];
        let result = generate(&source, &candidates, &chart, Mode::Rival, 1).unwrap();
        assert_eq!(result.pokemon[0].id, 2);
        assert_eq!(result.coverage, 1);
        assert_eq!(result.percentage_stat_difference, 2.0);
    }
}
