import { useEffect, useRef, useState } from "react";
import { keepPreviousData, useQuery } from "@tanstack/react-query";
import {
  Link,
  useNavigate,
  useParams,
  useSearchParams,
} from "react-router-dom";
import { api, type Pokemon } from "../api/client";
import {
  ErrorMessage,
  Loading,
  Pagination,
  PokemonCard,
  Sprite,
  Types,
  PokemonMetadata,
  pageOffset,
  generationLabel,
} from "../shared";
import { useSession } from "../session";
import { SpritePicker } from "./sprites";
export function AddToTeam({ pokemon }: { pokemon: Pokemon }) {
  const { ready } = useSession();
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();
  const teams = useQuery({
    queryKey: ["teams"],
    queryFn: ({ signal }) => api.teams(signal),
    enabled: ready,
  });
  return (
    <>
      <button disabled={!pokemon.active} onClick={() => setOpen(!open)}>
        Add to team
      </button>
      {open && (
        <div className="chooser">
          <p>Choose a team. The selection stays a draft until Save.</p>
          <button
            onClick={() => navigate("/teams/new", { state: { add: pokemon } })}
          >
            Start new draft
          </button>
          {ready ? (
            <>
              {teams.error && <ErrorMessage error={teams.error} />}{" "}
              {teams.data?.map((team) => (
                <button
                  key={team.id}
                  disabled={
                    team.pokemon.length >= 6 ||
                    team.pokemon.some((p) => p.id === pokemon.id)
                  }
                  onClick={() =>
                    navigate(`/teams/${team.id}`, { state: { add: pokemon } })
                  }
                >
                  {team.name}
                </button>
              ))}
            </>
          ) : (
            <p>Use Get started below to access saved teams.</p>
          )}
        </div>
      )}
    </>
  );
}
export function Catalog({
  onSelect,
  members = [],
  detailBase,
}: {
  onSelect?: (p: Pokemon) => void;
  members?: Pokemon[];
  detailBase?: string;
}) {
  const [params, setParams] = useSearchParams();
  // Keep rapid control changes together while URL navigation is still pending.
  const pendingParams = useRef(params);
  useEffect(() => {
    pendingParams.current = params;
  }, [params]);
  const search = params.get("search") || "";
  const [input, setInput] = useState(search);
  const offset = pageOffset(params.get("offset"));
  const selectedTypes = [
    ...new Set(
      (params.get("types") || params.get("type") || "")
        .split(",")
        .map((type) => type.trim().toLowerCase())
        .filter(Boolean),
    ),
  ];
  const typeMatch = params.get("type_match") || "all";
  const generation = params.get("generation") || "";
  const category = params.get("category") || "";
  useEffect(() => {
    setInput(search);
  }, [search]);
  useEffect(() => {
    if (input === search) return;
    const timer = setTimeout(
      () =>
        setParams(
          (() => {
            const next = new URLSearchParams(pendingParams.current);
            if (input) next.set("search", input);
            else next.delete("search");
            next.delete("offset");
            pendingParams.current = next;
            return next;
          })(),
          { replace: true },
        ),
      300,
    );
    return () => clearTimeout(timer);
  }, [input, search, setParams]);
  const query = new URLSearchParams({
    limit: "20",
    offset: String(offset),
    search,
    ...(selectedTypes.length
      ? { types: selectedTypes.join(","), type_match: typeMatch }
      : {}),
    ...(generation ? { generation } : {}),
    ...(category ? { category } : {}),
  });
  const catalog = useQuery({
    queryKey: ["pokemon", query.toString()],
    queryFn: ({ signal }) => api.catalog(query, signal),
    placeholderData: keepPreviousData,
  });
  const types = useQuery({
    queryKey: ["types"],
    queryFn: ({ signal }) => api.types(signal),
  });
  function change(key: string, value: string) {
    const next = new URLSearchParams(pendingParams.current);
    if (value) next.set(key, value);
    else next.delete(key);
    if (key === "types") next.delete("type");
    if (key !== "offset") next.delete("offset");
    pendingParams.current = next;
    setParams(next);
  }
  function clear() {
    pendingParams.current = new URLSearchParams();
    setInput("");
    setParams(pendingParams.current);
  }
  return (
    <section>
      <div className="section-heading">
        <div>
          <p className="eyebrow">THE POKÉDEX</p>
          <h2>
            {onSelect ? "Find your next teammate" : "A world of possibilities"}
          </h2>
        </div>
        <p>Explore. Compare. Build something great.</p>
      </div>
      <div className="catalog-filters card">
        <div className="filter-fields">
          <label>
            Search Pokémon
            <input
              type="search"
              placeholder="Search by name…"
              value={input}
              onChange={(event) => setInput(event.target.value)}
            />
          </label>
          <label>
            Introduced in
            <select
              value={generation}
              onChange={(event) => change("generation", event.target.value)}
            >
              <option value="">All generations</option>
              {catalog.data?.generations?.map((value) => (
                <option key={value} value={String(value)}>
                  Generation {generationLabel(value)}
                </option>
              ))}
              {generation &&
                !catalog.data?.generations?.includes(Number(generation)) && (
                  <option value={generation}>
                    Generation {generationLabel(Number(generation))}
                  </option>
                )}
            </select>
          </label>
          <label>
            Pokémon group
            <select
              value={category}
              onChange={(event) => change("category", event.target.value)}
            >
              <option value="">All Pokémon</option>
              <option value="starter">Starter families</option>
              <option value="legendary">Legendary</option>
              <option value="mythical">Mythical</option>
            </select>
          </label>
          <label>
            Type matching
            <select
              value={typeMatch}
              onChange={(event) =>
                change(
                  "type_match",
                  event.target.value === "all" ? "" : event.target.value,
                )
              }
            >
              <option value="all" disabled={selectedTypes.length > 2}>
                All selected types
              </option>
              <option value="any">Any selected type</option>
            </select>
          </label>
        </div>
        <fieldset className="type-filter">
          <legend>Types</legend>
          <div className="type-choices">
            {types.data?.types.map((type) => (
              <label
                key={type}
                className={`type-choice badge type-${type}${selectedTypes.includes(type) ? " checked" : ""}`}
              >
                <input
                  type="checkbox"
                  aria-label={`${type.charAt(0).toUpperCase() + type.slice(1)} type`}
                  checked={selectedTypes.includes(type)}
                  disabled={
                    typeMatch === "all" &&
                    selectedTypes.length >= 2 &&
                    !selectedTypes.includes(type)
                  }
                  onChange={() =>
                    change(
                      "types",
                      (selectedTypes.includes(type)
                        ? selectedTypes.filter((value) => value !== type)
                        : [...selectedTypes, type]
                      )
                        .sort()
                        .join(","),
                    )
                  }
                />
                {type}
              </label>
            ))}
          </div>
        </fieldset>
        <div className="filter-summary">
          <p>
            {typeMatch === "all"
              ? "Match both types of a dual-type Pokémon, or choose one type."
              : "Match Pokémon with at least one selected type."}
            {category === "starter" &&
              " Starter families include evolutions, Pikachu and Eevee."}
          </p>
          <button onClick={clear} disabled={!params.toString()}>
            Clear filters
          </button>
        </div>
      </div>
      {types.error && (
        <ErrorMessage error={types.error} retry={() => void types.refetch()} />
      )}{" "}
      {catalog.isPending ? (
        <Loading />
      ) : catalog.error ? (
        <ErrorMessage
          error={catalog.error}
          retry={() => void catalog.refetch()}
        />
      ) : (
        <>
          <p className="catalog-count" aria-live="polite">
            {catalog.data.total} matching Pokémon
            {catalog.isFetching ? " · Updating…" : ""}
          </p>
          <div className="grid">
            {catalog.data.pokemon.map((p) => (
              <PokemonCard
                key={p.id}
                pokemon={p}
                detailPath={
                  detailBase ? `${detailBase}/pokemon/${p.id}` : undefined
                }
                action={
                  onSelect ? (
                    <button
                      disabled={
                        !p.active ||
                        members.length >= 6 ||
                        members.some((m) => m.id === p.id)
                      }
                      onClick={() => onSelect(p)}
                    >
                      {members.some((m) => m.id === p.id)
                        ? "On your team"
                        : "Add Pokémon"}
                    </button>
                  ) : (
                    <AddToTeam pokemon={p} />
                  )
                }
              />
            ))}
          </div>
          {!catalog.data.total && <p>No Pokémon match these filters.</p>}
          <Pagination
            offset={offset}
            total={catalog.data.total}
            onChange={(value) => change("offset", String(value))}
          />
        </>
      )}
    </section>
  );
}
export function PokemonDetail({
  onSelect,
  back,
  members,
  onSpriteChange,
}: {
  onSelect?: (p: Pokemon) => void;
  back?: string;
  members?: Pokemon[];
  onSpriteChange?: (id: number, key: string | null) => void;
}) {
  const { pokemonId } = useParams();
  const [selected, setSelected] = useState<string | null>(null);
  useEffect(() => setSelected(null), [pokemonId]);
  const member = members?.find((p) => String(p.id) === pokemonId);
  const pokemon = useQuery({
    queryKey: ["pokemon-detail", pokemonId],
    queryFn: ({ signal }) => api.pokemon(pokemonId!, signal),
  });
  const chart = useQuery({
    queryKey: ["types"],
    queryFn: ({ signal }) => api.types(signal),
  });
  if (pokemon.isPending) return <Loading />;
  if (pokemon.error)
    return (
      <ErrorMessage
        error={pokemon.error}
        retry={() => void pokemon.refetch()}
      />
    );
  const p = {
    ...pokemon.data,
    selected_sprite: member ? member.selected_sprite : selected,
  };
  return (
    <section>
      <Link to={back || "/pokemon"}>
        ← {back ? "Back to draft" : "Back to Pokédex"}
      </Link>
      <div className="detail card">
        <div>
          <span className="number">#{p.id}</span>
          <Sprite pokemon={p} />
          <SpritePicker
            name={p.name}
            variants={p.sprite_variants || {}}
            value={p.selected_sprite}
            onChange={(key) =>
              member && onSpriteChange
                ? onSpriteChange(p.id, key)
                : setSelected(key)
            }
          />
          <h1>{p.name}</h1>
          <Types types={p.types} />
          <PokemonMetadata pokemon={p} />
          <p>
            {p.active
              ? "Available for selection"
              : "Unavailable · existing teams may retain this Pokémon"}
          </p>
          {member ? (
            <p>On your team · sprite changes stay in your draft until Save.</p>
          ) : onSelect ? (
            <button disabled={!p.active} onClick={() => onSelect(p)}>
              Add Pokémon to draft
            </button>
          ) : (
            <AddToTeam pokemon={p} />
          )}
        </div>
        <div>
          <h2>Base stats</h2>
          {Object.entries(p.stats).map(([name, value]) => (
            <div className="stat" key={name}>
              <span>{name.replaceAll("_", " ")}</span>
              <meter min="0" max="255" value={value} />
              <strong>{value}</strong>
            </div>
          ))}
          <p>
            <strong>{p.base_stat_total}</strong> total stats
          </p>
        </div>
      </div>
      <h2>Defensive effectiveness</h2>
      <p>
        Damage received by attacking type; dual-type multipliers are combined.
      </p>
      {chart.error ? (
        <ErrorMessage error={chart.error} retry={() => void chart.refetch()} />
      ) : chart.isPending ? (
        <Loading />
      ) : (
        <div className="effectiveness">
          {chart.data.types.map((type) => (
            <span className="card" key={type}>
              <Types types={[type]} />
              <strong>
                {" "}
                ×
                {p.types.reduce(
                  (value, defender) =>
                    value * (chart.data.effectiveness[type]?.[defender] ?? 1),
                  1,
                )}
              </strong>
            </span>
          ))}
        </div>
      )}
    </section>
  );
}
