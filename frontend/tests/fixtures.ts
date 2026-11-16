import type { Counter, Pokemon, Team } from "../src/api/client";
export const bulbasaur: Pokemon = {
  id: 1,
  species_id: 1,
  name: "bulbasaur",
  types: ["grass", "poison"],
  stats: {
    hp: 45,
    attack: 49,
    defense: 49,
    special_attack: 65,
    special_defense: 65,
    speed: 45,
  },
  generation: 1,
  is_starter: true,
  is_legendary: false,
  is_mythical: false,
  base_stat_total: 318,
  active: true,
  sprite_url: null,
  sprite_variants: {
    front_default: "/api/v1/sprites/front",
    front_shiny: "https://example.com/shiny.png",
    "other/official-artwork/front_default": "https://example.com/art.png",
    "other/showdown/front_default": "https://example.com/animated.gif",
    "versions/generation-i/red-blue/back_default":
      "https://example.com/red-back.png",
  },
  selected_sprite: null,
};
export const charmander: Pokemon = {
  ...bulbasaur,
  id: 4,
  species_id: 4,
  name: "charmander",
  types: ["fire"],
};
export const team: Team = {
  id: "team-1",
  name: "Favorites",
  revision: 7,
  created_at: "2026-11-15T00:00:00Z",
  updated_at: "2026-11-15T00:00:00Z",
  pokemon: [bulbasaur, charmander],
};
export const counter: Counter = {
  mode: "power",
  algorithm_version: "v1",
  catalog_revision: 1,
  pokemon: [charmander],
  source_total: 318,
  source_average: 318,
  counter_total: 318,
  counter_average: 318,
  percentage_stat_difference: 0,
  coverage: 1,
  matchup_score: 2,
  shared_weaknesses: [{ attacking_type: "water", pokemon_ids: [4] }],
  explanations: [
    {
      opponent_id: 1,
      best_attacker_id: 4,
      attacking_type: "fire",
      effectiveness: 2,
    },
  ],
};
export const catalog = {
  generations: [1, 2],
  pokemon: [bulbasaur, charmander],
  total: 2,
  limit: 20,
  offset: 0,
  catalog_revision: 1,
};
export const types = {
  types: ["grass", "poison", "fire", "water", "flying", "psychic"],
  effectiveness: { fire: { grass: 2, poison: 1 }, water: { fire: 2 } },
  catalog_revision: 1,
};
