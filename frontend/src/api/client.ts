import type { components } from "./generated";
export type Pokemon = components["schemas"]["Pokemon"];
export type Team = components["schemas"]["Team"];
export type Counter = components["schemas"]["CounterResult"];
export type Mode = components["schemas"]["Mode"];
export class ApiError extends Error {
  constructor(
    public status: number,
    public code: string,
    message: string,
    public details?: unknown,
  ) {
    super(message);
  }
}
let unauthorized = () => {};
export function onUnauthorized(handler: () => void) {
  unauthorized = handler;
}
export async function request<T>(
  path: string,
  method = "GET",
  body?: unknown,
  signal?: AbortSignal,
): Promise<T> {
  const response = await fetch(`/api/v1${path}`, {
    method,
    credentials: "same-origin",
    signal,
    headers:
      body === undefined ? undefined : { "Content-Type": "application/json" },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  if (!response.ok) {
    const error = await response
      .json()
      .catch(() => ({
        code: "http_error",
        message: `Request failed (${response.status})`,
      }));
    if (response.status === 401) unauthorized();
    throw new ApiError(
      response.status,
      error.code,
      error.message,
      error.details,
    );
  }
  return response.status === 204 ? (undefined as T) : response.json();
}
export const api = {
  session: () =>
    request<components["schemas"]["Session"]>("/session", "POST", {}),
  teams: (signal?: AbortSignal) =>
    request<Team[]>("/teams", "GET", undefined, signal),
  team: (id: string, signal?: AbortSignal) =>
    request<Team>(`/teams/${id}`, "GET", undefined, signal),
  create: (body: components["schemas"]["CreateTeam"]) =>
    request<Team>("/teams", "POST", body),
  update: (id: string, body: components["schemas"]["UpdateTeam"]) =>
    request<Team>(`/teams/${id}`, "PUT", body),
  remove: (id: string) => request<void>(`/teams/${id}`, "DELETE", {}),
  pokemon: (id: string, signal?: AbortSignal) =>
    request<Pokemon>(`/pokemon/${id}`, "GET", undefined, signal),
  catalog: (params: URLSearchParams, signal?: AbortSignal) =>
    request<components["schemas"]["CatalogPage"]>(
      `/pokemon?${params}`,
      "GET",
      undefined,
      signal,
    ),
  types: (signal?: AbortSignal) =>
    request<components["schemas"]["TypeResponse"]>(
      "/types",
      "GET",
      undefined,
      signal,
    ),
  counter: (id: string, mode: Mode) =>
    request<Counter>(`/teams/${id}/counter-team`, "POST", { mode }),
  notifications: (offset: number, signal?: AbortSignal) =>
    request<components["schemas"]["NotificationPage"]>(
      `/notifications?limit=20&offset=${offset}`,
      "GET",
      undefined,
      signal,
    ),
};
