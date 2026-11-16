import { useState } from "react";
import { useMutation, useQuery } from "@tanstack/react-query";
import { Link, useNavigate, useParams } from "react-router-dom";
import { api, ApiError, type Mode } from "../api/client";
import { ErrorMessage, Loading, PokemonCard, validateTeam } from "../shared";
import { useSession, useTeamRefresh } from "../session";
export function Counters() {
  const { id } = useParams();
  const { ready } = useSession();
  const [mode, setMode] = useState<Mode>("power");
  const [name, setName] = useState("");
  const navigate = useNavigate();
  const refresh = useTeamRefresh();
  const team = useQuery({
    queryKey: ["team", id],
    queryFn: ({ signal }) => api.team(id!, signal),
    enabled: ready,
  });
  const generate = useMutation({
    mutationFn: (selected: Mode) => api.counter(id!, selected),
  });
  const save = useMutation({
    mutationFn: () =>
      api.create({
        name: name.trim(),
        pokemon_ids: generate.data!.pokemon.map((p) => p.id),
      }),
    onSuccess: (saved) => {
      refresh();
      navigate(`/teams/${saved.id}`);
    },
  });
  if (!ready) return <p>Use Get started to access your teams.</p>;
  if (team.isPending) return <Loading />;
  if (team.error)
    return (
      <ErrorMessage error={team.error} retry={() => void team.refetch()} />
    );
  const result = generate.data;
  const sourceName = (id: number) =>
    team.data.pokemon.find((p) => p.id === id)?.name || `#${id}`;
  const counterName = (id: number) =>
    result?.pokemon.find((p) => p.id === id)?.name || `#${id}`;
  return (
    <section>
      <Link to={`/teams/${id}`}>← Back to team</Link>
      <p className="eyebrow">A WORTHY OPPONENT</p>
      <h1>Meet your match.</h1>
      <p>
        Generate a counter to the saved team “{team.data.name}”. Recommendations
        stay unsaved until you choose a name and save.
      </p>
      <label>
        Counter mode
        <select
          value={mode}
          disabled={generate.isPending || save.isPending}
          onChange={(event) => {
            setMode(event.target.value as Mode);
            generate.reset();
            save.reset();
          }}
        >
          <option value="power">Power</option>
          <option value="rival">Rival</option>
        </select>
      </label>
      <p>
        {mode === "power"
          ? "Power prioritizes type coverage, then total base stats and matchup score."
          : "Rival keeps total base stats within ±5% of the source team, prioritizing coverage and matchups."}{" "}
        These are heuristic recommendations, not battle predictions.
      </p>
      <button
        className="primary"
        disabled={
          !team.data.pokemon.length || generate.isPending || save.isPending
        }
        onClick={() => {
          save.reset();
          generate.mutate(mode);
        }}
      >
        {generate.isPending
          ? "Finding your match…"
          : generate.isError
            ? "Retry generation"
            : "Generate counter team"}
      </button>
      {!team.data.pokemon.length && (
        <p>Add and save at least one Pokémon first.</p>
      )}
      {generate.error && <ErrorMessage error={generate.error} />}{" "}
      {generate.error instanceof ApiError &&
        generate.error.code === "rival_balance_not_found" && (
          <p>
            The search found no qualifying recommendation within ±5%. Try again
            or switch to Power; this does not prove that no qualifying team
            exists.
          </p>
        )}
      {result && (
        <>
          <h2>Your {result.mode} recommendation</h2>
          <div className="comparison card">
            <table>
              <caption>Base stat comparison</caption>
              <thead>
                <tr>
                  <th>Team</th>
                  <th>Total</th>
                  <th>Average</th>
                </tr>
              </thead>
              <tbody>
                <tr>
                  <th>Source</th>
                  <td>{result.source_total}</td>
                  <td>{result.source_average.toFixed(1)}</td>
                </tr>
                <tr>
                  <th>Counter</th>
                  <td>{result.counter_total}</td>
                  <td>{result.counter_average.toFixed(1)}</td>
                </tr>
              </tbody>
            </table>
            <p>
              Stat difference: {result.percentage_stat_difference.toFixed(1)}% ·
              Coverage: {result.coverage}/{team.data.pokemon.length} · Matchup
              score: {result.matchup_score.toFixed(2)}
            </p>
          </div>
          <div className="grid">
            {result.pokemon.map((p) => (
              <PokemonCard key={p.id} pokemon={p} />
            ))}
          </div>
          <h2>Opponent matchups</h2>
          <ul>
            {result.explanations.map((item) => (
              <li key={item.opponent_id}>
                {counterName(item.best_attacker_id)} uses {item.attacking_type}{" "}
                against {sourceName(item.opponent_id)}: ×{item.effectiveness}
              </li>
            ))}
          </ul>
          <h2>Shared weaknesses</h2>
          {result.shared_weaknesses.length ? (
            <ul>
              {result.shared_weaknesses.map((item) => (
                <li key={item.attacking_type}>
                  {item.attacking_type}:{" "}
                  {item.pokemon_ids.map(counterName).join(", ")}
                </li>
              ))}
            </ul>
          ) : (
            <p>No shared weaknesses found.</p>
          )}
          <form
            className="card"
            onSubmit={(event) => {
              event.preventDefault();
              if (!validateTeam(name, result.pokemon)) save.mutate();
            }}
          >
            <h2>Keep this lineup</h2>
            <label>
              New team name
              <input
                value={name}
                onChange={(event) => setName(event.target.value)}
                required
              />
            </label>
            <button
              className="primary"
              disabled={
                !!validateTeam(name, result.pokemon) ||
                save.isPending ||
                generate.isPending
              }
            >
              Save as new team
            </button>
            {save.error && <ErrorMessage error={save.error} />}
          </form>
        </>
      )}
    </section>
  );
}
