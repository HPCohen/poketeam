import { useState } from "react";
import { Link } from "react-router-dom";
import { ApiError, type Pokemon } from "./api/client";
export function ErrorMessage({
  error,
  retry,
}: {
  error: unknown;
  retry?: () => void;
}) {
  const message =
    error instanceof ApiError && error.code === "catalog_unavailable"
      ? "The Pokédex is initializing. Please try again shortly."
      : error instanceof ApiError && error.status === 404
        ? "This resource is missing or no longer available."
        : error instanceof Error
          ? error.message
          : "A network error occurred. Please try again.";
  return (
    <div role="alert" className="notice">
      {message} {retry && <button onClick={retry}>Try again</button>}
    </div>
  );
}
export function Loading() {
  return <p role="status">Loading…</p>;
}
export function Sprite({ pokemon }: { pokemon: Pokemon }) {
  const selected = pokemon.selected_sprite;
  const url =
    (selected && pokemon.sprite_variants?.[selected]) || pokemon.sprite_url;
  const artwork =
    !!selected?.startsWith("other/") && !selected.includes("showdown");
  return (
    <SpriteImage
      key={`${pokemon.id}:${url}`}
      name={pokemon.name}
      url={url}
      fallback={pokemon.sprite_url}
      artwork={artwork}
    />
  );
}
function SpriteImage({
  name,
  url,
  fallback,
  artwork,
}: {
  name: string;
  url?: string | null;
  fallback?: string | null;
  artwork: boolean;
}) {
  const [failed, setFailed] = useState<string[]>([]);
  const source = [url, fallback].find(
    (value) => value && !failed.includes(value),
  );
  return source ? (
    <img
      className={`sprite${artwork ? " artwork" : ""}`}
      src={source}
      alt={name}
      onError={() => setFailed((previous) => [...previous, source])}
    />
  ) : (
    <span
      className="sprite fallback"
      role="img"
      aria-label={`${name} image unavailable`}
    >
      ?
    </span>
  );
}
export function Types({ types }: { types: string[] }) {
  return (
    <span className="badges">
      {types.map((type) => (
        <span key={type} className={`badge type-${type}`}>
          {type}
        </span>
      ))}
    </span>
  );
}
export function generationLabel(generation: number) {
  return (
    ["", "I", "II", "III", "IV", "V", "VI", "VII", "VIII", "IX", "X"][
      generation
    ] || String(generation)
  );
}
export function PokemonMetadata({ pokemon }: { pokemon: Pokemon }) {
  const labels = [
    pokemon.generation
      ? `Generation ${generationLabel(pokemon.generation)}`
      : null,
    pokemon.is_starter ? "Starter family" : null,
    pokemon.is_legendary ? "Legendary" : null,
    pokemon.is_mythical ? "Mythical" : null,
  ].filter(Boolean);
  return labels.length ? (
    <p className="pokemon-metadata">{labels.join(" · ")}</p>
  ) : null;
}
export function PokemonCard({
  pokemon,
  detailPath,
  action,
}: {
  pokemon: Pokemon;
  detailPath?: string;
  action?: React.ReactNode;
}) {
  return (
    <article className="card pokemon">
      <span className="number">#{String(pokemon.id).padStart(3, "0")}</span>
      <Link to={detailPath || `/pokemon/${pokemon.id}`}>
        <Sprite pokemon={pokemon} />
        <h3>{pokemon.name}</h3>
      </Link>
      <Types types={pokemon.types} />
      <PokemonMetadata pokemon={pokemon} />
      <p>
        {pokemon.base_stat_total} total stats
        {!pokemon.active && " · Unavailable"}
      </p>
      {action}
    </article>
  );
}
export function Pagination({
  offset,
  total,
  size = 20,
  onChange,
}: {
  offset: number;
  total: number;
  size?: number;
  onChange: (offset: number) => void;
}) {
  return (
    <nav className="pagination" aria-label="Pagination">
      <button
        disabled={offset === 0}
        onClick={() => onChange(Math.max(0, offset - size))}
      >
        Previous
      </button>
      <span>
        {total
          ? `${offset + 1}–${Math.min(offset + size, total)} of ${total}`
          : "0 results"}
      </span>
      <button
        disabled={offset + size >= total}
        onClick={() => onChange(offset + size)}
      >
        Next
      </button>
    </nav>
  );
}
export function pageOffset(value: string | null) {
  const number = Number(value);
  return Number.isSafeInteger(number) && number >= 0 ? number : 0;
}
export function validateTeam(name: string, pokemon: Pokemon[]) {
  if (!name.trim() || [...name.trim()].length > 100)
    return "Use a name with 1–100 characters.";
  if (pokemon.length > 6) return "A team can have at most six Pokémon.";
  if (new Set(pokemon.map((p) => p.id)).size !== pokemon.length)
    return "Choose distinct Pokémon.";
  return "";
}
