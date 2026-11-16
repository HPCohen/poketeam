import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
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
  pageOffset,
} from "../shared";
import { useSession } from "../session";
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
  const search = params.get("search") || "";
  const [input, setInput] = useState(search);
  const offset = pageOffset(params.get("offset"));
  const type = params.get("type") || "";
  useEffect(() => {
    setInput(search);
  }, [search]);
  useEffect(() => {
    if (input === search) return;
    const timer = setTimeout(
      () =>
        setParams(
          (previous) => {
            const next = new URLSearchParams(previous);
            next.set("search", input);
            next.delete("offset");
            return next;
          },
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
    ...(type ? { type } : {}),
  });
  const catalog = useQuery({
    queryKey: ["pokemon", query.toString()],
    queryFn: ({ signal }) => api.catalog(query, signal),
  });
  const types = useQuery({
    queryKey: ["types"],
    queryFn: ({ signal }) => api.types(signal),
  });
  function change(key: string, value: string) {
    setParams((previous) => {
      const next = new URLSearchParams(previous);
      next.set(key, value);
      if (key !== "offset") next.delete("offset");
      return next;
    });
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
      <div className="filters">
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
          Type
          <select
            value={type}
            onChange={(event) => change("type", event.target.value)}
          >
            <option value="">All types</option>
            {types.data?.types.map((t) => (
              <option key={t}>{t}</option>
            ))}
          </select>
        </label>
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
}: {
  onSelect?: (p: Pokemon) => void;
  back?: string;
}) {
  const { pokemonId } = useParams();
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
  const p = pokemon.data;
  return (
    <section>
      <Link to={back || "/pokemon"}>
        ← {back ? "Back to draft" : "Back to Pokédex"}
      </Link>
      <div className="detail card">
        <div>
          <span className="number">#{p.id}</span>
          <Sprite pokemon={p} />
          <h1>{p.name}</h1>
          <Types types={p.types} />
          <p>
            {p.active
              ? "Available for selection"
              : "Unavailable · existing teams may retain this Pokémon"}
          </p>
          {onSelect ? (
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
