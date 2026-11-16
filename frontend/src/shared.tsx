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
  const [failed, setFailed] = useState(false);
  return pokemon.sprite_url && !failed ? (
    <img
      className="sprite"
      src={pokemon.sprite_url}
      alt={pokemon.name}
      onError={() => setFailed(true)}
    />
  ) : (
    <span
      className="sprite fallback"
      role="img"
      aria-label={`${pokemon.name} image unavailable`}
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
