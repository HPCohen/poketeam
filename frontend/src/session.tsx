import {
  createContext,
  useContext,
  useState,
  useEffect,
  useRef,
  type ReactNode,
} from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { api, ApiError, onUnauthorized } from "./api/client";
import { ErrorMessage, Loading } from "./shared";
import { useNavigate } from "react-router-dom";
const Context = createContext({ ready: false, epoch: 0 });
export function SessionProvider({ children }: { children: ReactNode }) {
  const client = useQueryClient();
  const navigate = useNavigate();
  const readyBeforeExpiry = useRef(false);
  const [redirectEpoch, setRedirectEpoch] = useState(0);
  const [epoch, setEpoch] = useState(0);
  const [expired, setExpired] = useState(false);
  onUnauthorized(() => {
    if (readyBeforeExpiry.current) setRedirectEpoch((value) => value + 1);
    setExpired(true);
    setEpoch((value) => value + 1);
    client.removeQueries({
      predicate: (query) =>
        ["teams", "team", "notifications"].includes(String(query.queryKey[0])),
    });
  });
  const teams = useQuery({
    queryKey: ["teams"],
    queryFn: ({ signal }) => api.teams(signal),
    enabled: !expired,
    retry: false,
  });
  const session = useMutation({
    mutationFn: api.session,
    onSuccess: () => {
      setExpired(false);
      void client.invalidateQueries({ queryKey: ["teams"] });
    },
  });
  useEffect(() => {
    if (redirectEpoch > 0) navigate("/", { replace: true, state: null });
  }, [redirectEpoch, navigate]);
  const ready = !expired && teams.isSuccess;
  readyBeforeExpiry.current = ready;
  return (
    <Context.Provider value={{ ready, epoch }}>
      <div className="session-status" aria-live="polite">
        {expired && "No active browser session. Use Get started to save teams."}
      </div>
      {children}
      <div hidden={ready} className="session-panel">
        {teams.isPending && !expired ? (
          <Loading />
        ) : teams.error &&
          !(teams.error instanceof ApiError && teams.error.status === 401) &&
          !expired ? (
          <ErrorMessage
            error={teams.error}
            retry={() => void teams.refetch()}
          />
        ) : (
          <>
            <p>
              Teams are saved for this browser. Clearing cookies loses access.
            </p>
            <button
              className="primary"
              disabled={session.isPending}
              onClick={() => session.mutate()}
            >
              {session.isPending ? "Starting…" : "Get started"}
            </button>
          </>
        )}
        {session.error && <ErrorMessage error={session.error} />}
      </div>
    </Context.Provider>
  );
}
export const useSession = () => useContext(Context);
export function useTeamRefresh() {
  const client = useQueryClient();
  return () => {
    for (const key of ["teams", "team", "notifications"])
      void client.invalidateQueries({ queryKey: [key] });
  };
}
