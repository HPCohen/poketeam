import { QueryClientProvider } from "@tanstack/react-query";
import {
  act,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { createMemoryRouter, RouterProvider } from "react-router-dom";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { createClient, routes } from "../src/App";
import { api, ApiError, onUnauthorized, request } from "../src/api/client";
import { Sprite, validateTeam } from "../src/shared";
import {
  bulbasaur,
  catalog,
  charmander,
  counter,
  team,
  types,
} from "./fixtures";
let identity = true;
let conflict = false;
let expired = false;
let unavailable = false;
let storedTeam: typeof team;
let calls: { path: string; method: string; body: any }[];
beforeEach(() => {
  identity = true;
  conflict = false;
  expired = false;
  unavailable = false;
  calls = [];
  storedTeam = { ...team, pokemon: team.pokemon.map((p) => ({ ...p })) };
  vi.stubGlobal(
    "fetch",
    vi.fn(async (input: string, init?: RequestInit) => {
      const path = input.replace("/api/v1", "");
      const method = init?.method || "GET";
      const body = init?.body ? JSON.parse(String(init.body)) : undefined;
      calls.push({ path, method, body });
      const response = (data: unknown, status = 200) =>
        new Response(status === 204 ? null : JSON.stringify(data), {
          status,
          headers: { "Content-Type": "application/json" },
        });
      if (path === "/session") {
        identity = true;
        expired = false;
        return response({ owner_id: "owner" });
      }
      if (
        (path.startsWith("/teams") || path.startsWith("/notifications")) &&
        (!identity || expired)
      )
        return response(
          { code: "identity_required", message: "Session expired" },
          401,
        );
      if (path === "/teams" && method === "GET") return response([storedTeam]);
      if (path === "/teams" && method === "POST")
        return response(
          { ...team, id: "new-team", name: body.name, pokemon: [charmander] },
          201,
        );
      if (path.endsWith("/counter-team")) return response(counter);
      if (path.startsWith("/teams/") && method === "PUT") {
        if (conflict)
          return response(
            { code: "revision_conflict", message: "Newer revision exists" },
            409,
          );
        storedTeam = {
          ...storedTeam,
          name: body.name,
          revision: 8,
          pokemon: body.pokemon_ids.map((id: number) => {
            const p = team.pokemon.find((p) => p.id === id)!;
            const key = body.sprite_selections?.[String(id)];
            return {
              ...p,
              selected_sprite: !key || key === "front_default" ? null : key,
            };
          }),
        };
        return response(storedTeam);
      }
      if (path.startsWith("/teams/") && method === "DELETE")
        return response(null, 204);
      if (path.startsWith("/teams/"))
        return response({
          ...storedTeam,
          pokemon: unavailable
            ? [{ ...bulbasaur, active: false }, charmander]
            : storedTeam.pokemon,
        });
      if (path === "/types") return response(types);
      if (path.startsWith("/pokemon?")) return response(catalog);
      if (path.startsWith("/pokemon/")) return response(bulbasaur);
      if (path.startsWith("/notifications"))
        return response({ notifications: [], total: 0, limit: 20, offset: 0 });
      return response({ code: "not_found", message: "Missing" }, 404);
    }),
  );
});
function mount(path = "/") {
  const client = createClient();
  client.setDefaultOptions({
    queries: { retry: false },
    mutations: { retry: false },
  });
  const router = createMemoryRouter(routes, { initialEntries: [path] });
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return { client, router, user: userEvent.setup() };
}
describe("sessions and API", () => {
  it("waits for explicit Get started and never stores identity", async () => {
    const storageWrite = vi.spyOn(Storage.prototype, "setItem");
    identity = false;
    const { user } = mount();
    await user.click(
      await screen.findByRole("button", { name: "Get started" }),
    );
    expect(
      await screen.findByRole("heading", { name: "Your teams" }),
    ).toBeInTheDocument();
    expect(calls.filter((c) => c.path === "/session")).toHaveLength(1);
    expect(storageWrite).not.toHaveBeenCalled();
  });
  it("clears private data and drafts on mutation 401 without retrying", async () => {
    const { user, client } = mount("/teams/team-1");
    const input = await screen.findByLabelText("Team name");
    await user.clear(input);
    await user.type(input, "Secret draft");
    expired = true;
    await user.click(screen.getByRole("button", { name: "Save team" }));
    await screen.findByRole("button", { name: "Get started" });
    expect(screen.queryByDisplayValue("Secret draft")).not.toBeInTheDocument();
    expect(client.getQueryData(["team", "team-1"])).toBeUndefined();
    expect(calls.filter((c) => c.method === "PUT")).toHaveLength(1);
    expect(calls.filter((c) => c.path === "/session")).toHaveLength(0);
  });
  it("sends cookie credentials, DELETE JSON, cancellation, and accepts 204", async () => {
    onUnauthorized(() => {});
    const controller = new AbortController();
    await request("/teams/team-1", "DELETE", {}, controller.signal);
    expect(fetch).toHaveBeenLastCalledWith(
      "/api/v1/teams/team-1",
      expect.objectContaining({
        credentials: "same-origin",
        body: "{}",
        signal: controller.signal,
        headers: { "Content-Type": "application/json" },
      }),
    );
  });
  it("preserves structured API errors", async () => {
    conflict = true;
    await expect(
      api.update("team-1", { name: "X", pokemon_ids: [], revision: 7 }),
    ).rejects.toMatchObject({ status: 409, code: "revision_conflict" });
    expect(new ApiError(409, "revision_conflict", "Conflict")).toBeInstanceOf(
      Error,
    );
  });
});
describe("team drafts", () => {
  it("validates names, uniqueness and roster capacity", () => {
    expect(validateTeam(" ", [])).toContain("name");
    expect(validateTeam("x".repeat(101), [])).toContain("name");
    expect(validateTeam("Valid", [bulbasaur, bulbasaur])).toContain("distinct");
    expect(validateTeam("Valid", Array(7).fill(bulbasaur))).toContain("six");
    expect(validateTeam("Valid", [])).toBe("");
  });
  it("preserves a draft through details and blocks leaving", async () => {
    const { user } = mount("/teams/team-1");
    const input = await screen.findByLabelText("Team name");
    await user.clear(input);
    await user.type(input, "Draft name");
    await user.click(screen.getAllByRole("link", { name: /bulbasaur/ })[0]);
    await screen.findByRole("heading", { name: "Base stats" });
    expect(screen.getByDisplayValue("Draft name")).toBeInTheDocument();
    await user.click(screen.getByRole("link", { name: "Your teams" }));
    await user.click(
      await screen.findByRole("button", { name: "Keep editing" }),
    );
    expect(screen.getByDisplayValue("Draft name")).toBeInTheDocument();
  });
  it("retains unavailable members and saves reordered IDs with loaded revision", async () => {
    unavailable = true;
    const { user } = mount("/teams/team-1");
    await screen.findByText("Unavailable · retained member");
    await user.click(
      screen.getByRole("button", { name: "Move bulbasaur right" }),
    );
    await user.click(screen.getByRole("button", { name: "Save team" }));
    await screen.findByText("Team saved.");
    expect(calls.find((c) => c.method === "PUT")?.body).toEqual({
      name: "Favorites",
      pokemon_ids: [4, 1],
      sprite_selections: { "1": "front_default", "4": "front_default" },
      revision: 7,
    });
  });
  it("keeps a conflicting draft until explicit discard and reload", async () => {
    conflict = true;
    const { user } = mount("/teams/team-1");
    const input = await screen.findByLabelText("Team name");
    await user.clear(input);
    await user.type(input, "Keep me");
    await user.click(screen.getByRole("button", { name: "Save team" }));
    await screen.findByText("Newer revision exists");
    expect(screen.getByDisplayValue("Keep me")).toBeInTheDocument();
    await user.click(
      screen.getByRole("button", { name: "Discard draft and reload latest" }),
    );
    expect(await screen.findByDisplayValue("Favorites")).toBeInTheDocument();
  });
  it("disables duplicate and unavailable additions", async () => {
    const { user } = mount("/teams/team-1");
    await screen.findByLabelText("Team name");
    await waitFor(() =>
      expect(
        screen.getAllByRole("button", { name: "On your team" }),
      ).toHaveLength(2),
    );
    for (const button of screen.getAllByRole("button", {
      name: "On your team",
    }))
      expect(button).toBeDisabled();
    await user.click(screen.getByRole("button", { name: "Remove bulbasaur" }));
    await user.click(screen.getByRole("button", { name: "Add Pokémon" }));
    expect(screen.getByRole("status")).toHaveTextContent("added to draft");
  });
});
it("counter generation is a preview, saved only on explicit submission", async () => {
  const { user } = mount("/teams/team-1/counter");
  await user.click(
    await screen.findByRole("button", { name: "Generate counter team" }),
  );
  await screen.findByRole("heading", { name: "Your power recommendation" });
  expect(
    calls.filter((c) => c.path === "/teams" && c.method === "POST"),
  ).toHaveLength(0);
  await user.type(screen.getByLabelText("New team name"), "My rival");
  await user.click(screen.getByRole("button", { name: "Save as new team" }));
  await waitFor(() =>
    expect(
      calls.find((c) => c.path === "/teams" && c.method === "POST")?.body,
    ).toEqual({ name: "My rival", pokemon_ids: [4] }),
  );
});
it("replaces and deduplicates notification refresh results", async () => {
  const { user, client } = mount("/notifications");
  await screen.findByText("No changes for your teams in the last seven days.");
  const event = {
    id: 8,
    pokemon: bulbasaur,
    descriptions: ["Attack changed"],
    detected_at: team.updated_at,
    differences: {},
    affected_team_ids: ["team-1"],
  };
  await act(async () => {
    client.setQueryData(["notifications", 0], {
      notifications: [event, event],
      total: 1,
      limit: 20,
      offset: 0,
    });
  });
  expect(await screen.findAllByText("Attack changed")).toHaveLength(1);
  await user.click(screen.getByRole("button", { name: "Refresh changes" }));
  await screen.findByText("No changes for your teams in the last seven days.");
  expect(screen.queryByText("Attack changed")).not.toBeInTheDocument();
});
it("permits anonymous exploration and a new draft before Get started", async () => {
  identity = false;
  const { user, router } = mount("/pokemon");
  await screen.findByRole("button", { name: "Get started" });
  await user.click(screen.getAllByRole("button", { name: "Add to team" })[0]);
  await user.click(screen.getByRole("button", { name: "Start new draft" }));
  expect(
    await screen.findByRole("button", { name: "Remove bulbasaur" }),
  ).toBeInTheDocument();
  expect(screen.getByRole("button", { name: "Save team" })).toBeDisabled();
  expect(calls.some((call) => call.path === "/session")).toBe(false);
  await waitFor(() => expect(router.state.location.state).toBeNull());
});
it("explains Rival search failure and offers an explicit retry or mode change", async () => {
  const original = vi.mocked(fetch).getMockImplementation()!;
  vi.mocked(fetch).mockImplementation(async (input, init) =>
    String(input).endsWith("/counter-team")
      ? new Response(
          JSON.stringify({
            code: "rival_balance_not_found",
            message: "No balanced seed found",
          }),
          { status: 422 },
        )
      : original(input, init),
  );
  const { user } = mount("/teams/team-1/counter");
  await user.selectOptions(
    await screen.findByLabelText("Counter mode"),
    "rival",
  );
  await user.click(
    screen.getByRole("button", { name: "Generate counter team" }),
  );
  await screen.findByText(/search found no qualifying recommendation/);
  expect(
    screen.getByRole("button", { name: "Retry generation" }),
  ).toBeEnabled();
  await user.selectOptions(screen.getByLabelText("Counter mode"), "power");
  expect(screen.queryByText("No balanced seed found")).not.toBeInTheDocument();
});
it("warns before closing with an unsaved draft", async () => {
  const { user } = mount("/teams/team-1");
  await user.type(await screen.findByLabelText("Team name"), " draft");
  const event = new Event("beforeunload", { cancelable: true });
  window.dispatchEvent(event);
  expect(event.defaultPrevented).toBe(true);
});

it("treats sprite edits as drafts and preserves them through details, reorder and conflicts", async () => {
  const { user } = mount("/teams/team-1");
  const picker = await screen.findByRole("combobox", {
    name: "Sprite for bulbasaur",
  });
  await user.selectOptions(picker, "front_shiny");
  expect(screen.getByRole("status")).toHaveTextContent("Unsaved draft");
  await user.click(screen.getAllByRole("link", { name: /bulbasaur/ })[0]);
  await screen.findByRole("heading", { name: "Base stats" });
  for (const picker of screen.getAllByRole("combobox", {
    name: "Sprite for bulbasaur",
  }))
    expect(picker).toHaveValue("front_shiny");
  await user.click(screen.getByRole("link", { name: /Back to draft/ }));
  await user.click(
    screen.getByRole("button", { name: "Move bulbasaur right" }),
  );
  conflict = true;
  await user.click(screen.getByRole("button", { name: "Save team" }));
  await screen.findByText("Newer revision exists");
  expect(
    screen.getByRole("combobox", { name: "Sprite for bulbasaur" }),
  ).toHaveValue("front_shiny");
  expect(calls.find((call) => call.method === "PUT")?.body).toMatchObject({
    pokemon_ids: [4, 1],
    sprite_selections: { "1": "front_shiny", "4": "front_default" },
    revision: 7,
  });
  await user.click(
    screen.getByRole("button", { name: "Discard draft and reload latest" }),
  );
  await waitFor(() =>
    expect(
      screen.getByRole("combobox", { name: "Sprite for bulbasaur" }),
    ).toHaveValue("front_default"),
  );
});
it("saves sprite-only changes and clears the dirty state", async () => {
  const { user } = mount("/teams/team-1");
  await user.selectOptions(
    await screen.findByRole("combobox", { name: "Sprite for bulbasaur" }),
    "other/official-artwork/front_default",
  );
  await user.click(screen.getByRole("button", { name: "Save team" }));
  await screen.findByText("Team saved.");
  expect(
    screen.getByRole("combobox", { name: "Sprite for bulbasaur" }),
  ).toHaveValue("other/official-artwork/front_default");
  const event = new Event("beforeunload", { cancelable: true });
  window.dispatchEvent(event);
  expect(event.defaultPrevented).toBe(false);
});
it("stages the viewer's chosen animated sprite when starting a new team", async () => {
  const { user } = mount("/pokemon/1");
  await user.selectOptions(
    await screen.findByRole("combobox", { name: "Sprite for bulbasaur" }),
    "other/showdown/front_default",
  );
  expect(screen.getByRole("img", { name: "bulbasaur" })).toHaveAttribute(
    "src",
    "https://example.com/animated.gif",
  );
  await user.click(screen.getByRole("button", { name: "Add to team" }));
  await user.click(screen.getByRole("button", { name: "Start new draft" }));
  expect(
    await screen.findByRole("combobox", { name: "Sprite for bulbasaur" }),
  ).toHaveValue("other/showdown/front_default");
  await user.type(screen.getByLabelText("Team name"), "Animated team");
  await user.click(screen.getByRole("button", { name: "Save team" }));
  await waitFor(() =>
    expect(
      calls.find((call) => call.path === "/teams" && call.method === "POST")
        ?.body.sprite_selections,
    ).toEqual({ "1": "other/showdown/front_default" }),
  );
});
it("falls back on broken or removed sprites and retries a newly selected URL", () => {
  const p = {
    ...bulbasaur,
    sprite_url: "/default.png",
    selected_sprite: "front_shiny",
  };
  const { rerender } = render(<Sprite pokemon={p} />);
  fireEvent.error(screen.getByRole("img"));
  expect(screen.getByRole("img")).toHaveAttribute("src", "/default.png");
  rerender(
    <Sprite
      pokemon={{
        ...p,
        selected_sprite: "other/official-artwork/front_default",
      }}
    />,
  );
  expect(screen.getByRole("img")).toHaveAttribute(
    "src",
    "https://example.com/art.png",
  );
  fireEvent.error(screen.getByRole("img"));
  fireEvent.error(screen.getByRole("img"));
  expect(screen.getByRole("img")).toHaveAttribute(
    "aria-label",
    "bulbasaur image unavailable",
  );
  rerender(<Sprite pokemon={{ ...p, selected_sprite: "removed" }} />);
  expect(screen.getByRole("img")).toHaveAttribute("src", "/default.png");
});

it("keeps advanced filters in the URL, combines them, and preserves them during pagination", async () => {
  const original = vi.mocked(fetch).getMockImplementation()!;
  vi.mocked(fetch).mockImplementation(async (input, init) =>
    String(input).startsWith("/api/v1/pokemon?")
      ? new Response(JSON.stringify({ ...catalog, total: 45 }))
      : original(input, init),
  );
  const { user, router } = mount(
    "/pokemon?types=grass,poison&generation=1&category=starter&offset=20",
  );
  expect(
    await screen.findByRole("checkbox", { name: "Grass type" }),
  ).toBeChecked();
  expect(screen.getByRole("checkbox", { name: "Poison type" })).toBeChecked();
  expect(screen.getByRole("checkbox", { name: "Fire type" })).toBeDisabled();
  expect(screen.getByRole("combobox", { name: "Introduced in" })).toHaveValue(
    "1",
  );
  expect(screen.getByRole("combobox", { name: "Pokémon group" })).toHaveValue(
    "starter",
  );
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Type matching" }),
    "any",
  );
  expect(new URLSearchParams(router.state.location.search).has("offset")).toBe(
    false,
  );
  expect(screen.getByRole("checkbox", { name: "Fire type" })).toBeEnabled();
  await user.click(screen.getByRole("checkbox", { name: "Fire type" }));
  expect(
    screen.getByRole("option", { name: "All selected types" }),
  ).toBeDisabled();
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Introduced in" }),
    "2",
  );
  await user.selectOptions(
    screen.getByRole("combobox", { name: "Pokémon group" }),
    "legendary",
  );
  await user.type(screen.getByRole("searchbox"), "mew");
  await waitFor(() =>
    expect(
      new URLSearchParams(router.state.location.search).get("search"),
    ).toBe("mew"),
  );
  await user.click(await screen.findByRole("button", { name: "Next" }));
  const params = new URLSearchParams(router.state.location.search);
  expect(params.get("types")).toBe("fire,grass,poison");
  expect(params.get("type_match")).toBe("any");
  expect(params.get("generation")).toBe("2");
  expect(params.get("category")).toBe("legendary");
  expect(params.get("offset")).toBe("20");
  await user.click(screen.getByRole("button", { name: "Clear filters" }));
  expect(router.state.location.search).toBe("");
  expect(screen.getByRole("searchbox")).toHaveValue("");
  expect(
    screen.getByRole("checkbox", { name: "Grass type" }),
  ).not.toBeChecked();
});
it("supports legacy type bookmarks and preserves editor drafts while filtering", async () => {
  const { user, router } = mount("/teams/team-1?type=fire");
  await user.type(await screen.findByLabelText("Team name"), " draft");
  expect(
    await screen.findByRole("checkbox", { name: "Fire type" }),
  ).toBeChecked();
  await user.click(screen.getByRole("checkbox", { name: "Flying type" }));
  expect(new URLSearchParams(router.state.location.search).get("types")).toBe(
    "fire,flying",
  );
  expect(new URLSearchParams(router.state.location.search).has("type")).toBe(
    false,
  );
  expect(screen.getByLabelText("Team name")).toHaveValue("Favorites draft");
  expect(screen.queryByRole("alertdialog")).not.toBeInTheDocument();
});
