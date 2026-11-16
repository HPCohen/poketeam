import { useQuery } from "@tanstack/react-query";
import { Link, useSearchParams } from "react-router-dom";
import { api } from "../api/client";
import { ErrorMessage, Loading, Pagination, pageOffset } from "../shared";
import { useSession } from "../session";
export function Notifications() {
  const { ready } = useSession();
  const [params, setParams] = useSearchParams();
  const offset = pageOffset(params.get("offset"));
  const query = useQuery({
    queryKey: ["notifications", offset],
    queryFn: ({ signal }) => api.notifications(offset, signal),
    enabled: ready,
    refetchInterval: 60_000,
    refetchIntervalInBackground: false,
  });
  const teams = useQuery({
    queryKey: ["teams"],
    queryFn: ({ signal }) => api.teams(signal),
    enabled: ready,
  });
  if (!ready)
    return <p>Use Get started to see changes affecting your teams.</p>;
  return (
    <section>
      <p className="eyebrow">STAY IN THE KNOW</p>
      <h1>
        A little change.
        <br />A new possibility.
      </h1>
      <p>
        Changes affecting your current teams from the preceding seven days.
        Refreshed every 60 seconds while visible.
      </p>
      <button disabled={query.isFetching} onClick={() => void query.refetch()}>
        Refresh changes
      </button>
      {query.isPending ? (
        <Loading />
      ) : query.error ? (
        <ErrorMessage error={query.error} retry={() => void query.refetch()} />
      ) : (
        <>
          <p role="status">
            {query.isFetching ? "Refreshing…" : "Changes up to date."}
          </p>
          {[
            ...new Map(
              query.data.notifications.map((item) => [item.id, item]),
            ).values(),
          ].map((item) => (
            <article className="card change" key={item.id}>
              <h2>
                <Link to={`/pokemon/${item.pokemon.id}`}>
                  {item.pokemon.name}
                </Link>
              </h2>
              <time dateTime={item.detected_at}>
                {new Date(item.detected_at).toLocaleString()}
              </time>
              <ul>
                {item.descriptions.map((description, index) => (
                  <li key={index}>{description}</li>
                ))}
              </ul>
              <p>
                Affected teams:{" "}
                {item.affected_team_ids.map((id) => (
                  <Link key={id} className="secondary-link" to={`/teams/${id}`}>
                    {teams.data?.find((team) => team.id === id)?.name || id}
                  </Link>
                ))}
              </p>
            </article>
          ))}
          {!query.data.notifications.length && (
            <p>No changes for your teams in the last seven days.</p>
          )}
          <Pagination
            offset={offset}
            total={query.data.total}
            onChange={(value) => setParams({ offset: String(value) })}
          />
        </>
      )}
    </section>
  );
}
