import { useEffect, useRef, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Link,
  Route,
  Routes,
  useBlocker,
  useLocation,
  useNavigate,
  useParams,
} from "react-router-dom";
import { api, ApiError, type Pokemon, type Team } from "../api/client";
import { ErrorMessage, Loading, Sprite, validateTeam } from "../shared";
import { useSession, useTeamRefresh } from "../session";
import { Catalog, PokemonDetail } from "./catalog";
export function Home() {
  const { ready } = useSession();
  const teams = useQuery({
    queryKey: ["teams"],
    queryFn: ({ signal }) => api.teams(signal),
    enabled: ready,
  });
  return (
    <>
      <section className="hero">
        <p className="eyebrow">YOUR TEAM. YOUR ADVENTURE.</p>
        <h1>
          Great teams
          <br />
          start with a spark.
        </h1>
        <p>
          Find your favorites, discover new possibilities,
          <br />
          and meet your next rival.
        </p>
        <Link className="button primary" to={ready ? "/teams/new" : "/pokemon"}>
          {ready ? "Build a team" : "Explore the Pokédex"}{" "}
          <span aria-hidden="true">↗</span>
        </Link>
        <div className="pokeball" aria-hidden="true" />
      </section>
      {ready && (
        <section>
          <div className="section-heading">
            <h2>Your teams</h2>
            <Link to="/teams/new">+ New team</Link>
          </div>
          {teams.isPending ? (
            <Loading />
          ) : teams.error ? (
            <ErrorMessage error={teams.error} />
          ) : teams.data.length ? (
            <div className="grid">
              {teams.data.map((team) => (
                <article className="card" key={team.id}>
                  <h3>
                    <Link to={`/teams/${team.id}`}>{team.name}</Link>
                  </h3>
                  <div className="mini-roster">
                    {team.pokemon.map((p) => (
                      <Sprite key={p.id} pokemon={p} />
                    ))}
                  </div>
                  <p>{team.pokemon.length}/6 teammates</p>
                  <Link to={`/teams/${team.id}`}>Edit team →</Link>
                  {team.pokemon.length > 0 && (
                    <Link
                      className="secondary-link"
                      to={`/teams/${team.id}/counter`}
                    >
                      Find a counter →
                    </Link>
                  )}
                </article>
              ))}
            </div>
          ) : (
            <p>
              Your first team is waiting to happen. Build a team to get started.
            </p>
          )}
        </section>
      )}
    </>
  );
}
export function TeamEditor() {
  const { id } = useParams();
  const { ready } = useSession();
  const isNew = !id;
  const team = useQuery({
    queryKey: ["team", id],
    queryFn: ({ signal }) => api.team(id!, signal),
    enabled: ready && !isNew,
  });
  if (!isNew && !ready) return <p>Use Get started to access saved teams.</p>;
  if (!isNew && team.isPending) return <Loading />;
  if (!isNew && team.error)
    return (
      <ErrorMessage error={team.error} retry={() => void team.refetch()} />
    );
  return <Editor key={id || "new"} initial={isNew ? undefined : team.data} />;
}
function Editor({ initial }: { initial?: Team }) {
  const { ready } = useSession();
  const [name, setName] = useState(initial?.name || "");
  const [members, setMembers] = useState<Pokemon[]>(initial?.pokemon || []);
  const [revision, setRevision] = useState(initial?.revision);
  const [baseline, setBaseline] = useState(
    JSON.stringify([
      initial?.name || "",
      initial?.pokemon.map((p) => p.id) || [],
    ]),
  );
  const [message, setMessage] = useState("");
  const navigate = useNavigate();
  const location = useLocation();
  const refresh = useTeamRefresh();
  const client = useQueryClient();
  const leaving = useRef(false);
  const base = initial ? `/teams/${initial.id}` : "/teams/new";
  const dirty = baseline !== JSON.stringify([name, members.map((p) => p.id)]);
  const blocker = useBlocker(
    ({ nextLocation }) =>
      dirty &&
      !leaving.current &&
      nextLocation.pathname !== base &&
      !nextLocation.pathname.startsWith(`${base}/pokemon/`),
  );
  useEffect(() => {
    if (!dirty) return;
    const warn = (event: BeforeUnloadEvent) => {
      event.preventDefault();
      event.returnValue = "";
    };
    window.addEventListener("beforeunload", warn);
    return () => window.removeEventListener("beforeunload", warn);
  }, [dirty]);
  const staged = useRef<string | null>(null);
  useEffect(() => {
    const add = location.state?.add as Pokemon | undefined;
    if (!add || staged.current === location.key) return;
    staged.current = location.key;
    setMembers((previous) =>
      !add.active ||
      previous.length >= 6 ||
      previous.some((p) => p.id === add.id)
        ? previous
        : [...previous, add],
    );
    // Consume the navigation handoff so browser history cannot restore a draft.
    navigate(`${location.pathname}${location.search}`, {
      replace: true,
      state: null,
    });
  }, [
    location.key,
    location.state,
    location.pathname,
    location.search,
    navigate,
  ]);
  const save = useMutation({
    mutationFn: () =>
      initial
        ? api.update(initial.id, {
            name: name.trim(),
            pokemon_ids: members.map((p) => p.id),
            revision: revision!,
          })
        : api.create({
            name: name.trim(),
            pokemon_ids: members.map((p) => p.id),
          }),
    onSuccess: (saved) => {
      setName(saved.name);
      setMembers(saved.pokemon);
      setRevision(saved.revision);
      setBaseline(JSON.stringify([saved.name, saved.pokemon.map((p) => p.id)]));
      client.setQueryData(["team", saved.id], saved);
      refresh();
      setMessage("Team saved.");
      if (!initial) {
        leaving.current = true;
        navigate(`/teams/${saved.id}`, { replace: true });
      }
    },
  });
  const remove = useMutation({
    mutationFn: () => api.remove(initial!.id),
    onSuccess: () => {
      leaving.current = true;
      refresh();
      navigate("/");
    },
  });
  const reload = useMutation({
    mutationFn: () => api.team(initial!.id),
    onSuccess: (saved) => {
      setName(saved.name);
      setMembers(saved.pokemon);
      setRevision(saved.revision);
      setBaseline(JSON.stringify([saved.name, saved.pokemon.map((p) => p.id)]));
      save.reset();
      setMessage("Loaded latest saved team; draft discarded.");
    },
  });
  function add(p: Pokemon) {
    if (
      pending ||
      !p.active ||
      members.length >= 6 ||
      members.some((m) => m.id === p.id)
    ) {
      setMessage(
        "Choose an available Pokémon not already on this team, with room in the roster.",
      );
      return;
    }
    setMembers([...members, p]);
    setMessage(`${p.name} added to draft.`);
  }
  function move(index: number, direction: number) {
    const next = [...members];
    [next[index], next[index + direction]] = [
      next[index + direction],
      next[index],
    ];
    setMembers(next);
  }
  const invalid = validateTeam(name, members);
  const pending = save.isPending || remove.isPending || reload.isPending;
  return (
    <>
      <section>
        <p className="eyebrow">TEAM BUILDER · {members.length}/6</p>
        <h1>{initial ? "Make it your own." : "A new adventure."}</h1>
        <label className="team-name">
          Team name
          <input
            disabled={pending}
            maxLength={200}
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </label>
        <div className="roster">
          {members.map((p, index) => (
            <article className="card roster-member" key={p.id}>
              <Link to={`${base}/pokemon/${p.id}`}>
                <Sprite pokemon={p} />
                <strong>{p.name}</strong>
              </Link>
              {!p.active && <p>Unavailable · retained member</p>}
              <div className="actions">
                <button
                  aria-label={`Move ${p.name} left`}
                  disabled={pending || index === 0}
                  onClick={() => move(index, -1)}
                >
                  ←
                </button>
                <button
                  aria-label={`Move ${p.name} right`}
                  disabled={pending || index === members.length - 1}
                  onClick={() => move(index, 1)}
                >
                  →
                </button>
                <button
                  disabled={pending}
                  aria-label={`Remove ${p.name}`}
                  onClick={() =>
                    setMembers(members.filter((m) => m.id !== p.id))
                  }
                >
                  Remove
                </button>
              </div>
            </article>
          ))}
        </div>
        {!members.length && (
          <p className="empty">Room for six. Pick your first teammate below.</p>
        )}
        <div className="actions">
          <button
            className="primary"
            disabled={!ready || !!invalid || pending}
            onClick={() => save.mutate()}
          >
            {save.isPending ? "Saving…" : "Save team"}
          </button>
          {initial && (
            <>
              <button
                disabled={pending}
                onClick={() => {
                  if (
                    window.confirm(
                      `Delete ${initial.name}? This cannot be undone.`,
                    )
                  )
                    remove.mutate();
                }}
              >
                Delete team
              </button>
              <Link
                className="button"
                aria-disabled={dirty || !members.length}
                to={`${base}/counter`}
                onClick={(event) => {
                  if (dirty || !members.length) {
                    event.preventDefault();
                    setMessage(
                      "Save or discard your edits and use a nonempty team before generating a counter.",
                    );
                  }
                }}
              >
                Find counter team
              </Link>
            </>
          )}
        </div>
        {invalid && <p>{invalid}</p>}
        {!ready && <p>Use Get started below before saving.</p>}
        <p role="status">
          {message || (dirty ? "Unsaved draft" : "All changes saved")}
        </p>
        {save.error && <ErrorMessage error={save.error} />}
        {save.error instanceof ApiError && save.error.status === 409 && (
          <div className="notice">
            <p>
              A newer revision exists. Your draft is preserved. Reloading
              explicitly discards it.
            </p>
            <button disabled={pending} onClick={() => reload.mutate()}>
              Discard draft and reload latest
            </button>
          </div>
        )}
        {remove.error && <ErrorMessage error={remove.error} />}
        {reload.error && <ErrorMessage error={reload.error} />}
        {blocker.state === "blocked" && (
          <div
            role="alertdialog"
            aria-modal="true"
            aria-label="Unsaved changes"
            className="dialog"
          >
            <p>Discard your unsaved changes and leave?</p>
            <button autoFocus onClick={() => blocker.reset()}>
              Keep editing
            </button>
            <button onClick={() => blocker.proceed()}>Discard and leave</button>
          </div>
        )}
      </section>
      <Routes>
        <Route
          index
          element={
            <Catalog onSelect={add} members={members} detailBase={base} />
          }
        />
        <Route
          path="pokemon/:pokemonId"
          element={<PokemonDetail onSelect={add} back={base} />}
        />
      </Routes>
    </>
  );
}
