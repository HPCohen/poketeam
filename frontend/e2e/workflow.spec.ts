import { expect, test, type Page } from "@playwright/test";
import { bulbasaur, charmander, counter, types } from "../tests/fixtures";
import type { Pokemon, Team } from "../src/api/client";
async function backend(page: Page, advanced = false) {
  let session = false;
  let nextId = 1;
  const teams: Team[] = [];
  let change = true;
  const dataset: Pokemon[] = [
    bulbasaur,
    charmander,
    ...(advanced
      ? [
          {
            ...charmander,
            id: 6,
            species_id: 6,
            name: "charizard",
            types: ["fire", "flying"],
          },
          {
            ...bulbasaur,
            id: 153,
            species_id: 153,
            name: "bayleef",
            types: ["grass"],
            generation: 2,
          },
          {
            ...bulbasaur,
            id: 150,
            species_id: 150,
            name: "mewtwo",
            types: ["psychic"],
            is_starter: false,
            is_legendary: true,
          },
          {
            ...bulbasaur,
            id: 151,
            species_id: 151,
            name: "mew",
            types: ["psychic"],
            is_starter: false,
            is_mythical: true,
          },
          {
            ...bulbasaur,
            id: 245,
            species_id: 245,
            name: "suicune",
            types: ["water"],
            generation: 2,
            is_starter: false,
            is_legendary: true,
          },
        ]
      : []),
  ];
  const member = (
    id: number,
    selections?: Record<string, string>,
    previous: Pokemon[] = [],
  ) => {
    const pokemon = dataset.find((pokemon) => pokemon.id === id)!;
    const selection = selections?.[String(id)];
    return {
      ...pokemon,
      selected_sprite:
        selection === "front_default"
          ? null
          : selection ||
            previous.find((p) => p.id === id)?.selected_sprite ||
            null,
    };
  };
  await page.route("https://example.com/**", (route) =>
    route.fulfill({
      status: 200,
      contentType: "image/svg+xml",
      body: '<svg xmlns="http://www.w3.org/2000/svg" width="32" height="32"><rect width="32" height="32" fill="red"/></svg>',
    }),
  );
  await page.route("**/api/v1/**", async (route) => {
    const request = route.request();
    const url = new URL(request.url());
    const path = url.pathname.replace("/api/v1", "");
    const body = request.postDataJSON();
    const json = (data: unknown, status = 200) =>
      route.fulfill({ status, json: data });
    if (path === "/session") {
      session = true;
      return json({ owner_id: "owner" });
    }
    if ((path.startsWith("/teams") || path === "/notifications") && !session)
      return json(
        { code: "identity_required", message: "Session required" },
        401,
      );
    if (path === "/teams" && request.method() === "GET") return json(teams);
    if (path === "/teams" && request.method() === "POST") {
      const team = {
        id: `team-${nextId++}`,
        name: body.name,
        revision: 1,
        pokemon: body.pokemon_ids.map((id: number) =>
          member(id, body.sprite_selections),
        ),
        created_at: new Date().toISOString(),
        updated_at: new Date().toISOString(),
      };
      teams.push(team);
      return json(team, 201);
    }
    if (path.endsWith("/counter-team")) return json(counter);
    if (path.startsWith("/teams/")) {
      const index = teams.findIndex((team) => team.id === path.split("/")[2]);
      const team = teams[index];
      if (!team)
        return json({ code: "not_found", message: "Team missing" }, 404);
      if (request.method() === "DELETE") {
        teams.splice(index, 1);
        return route.fulfill({ status: 204 });
      }
      if (request.method() === "PUT") {
        Object.assign(team, {
          name: body.name,
          revision: team.revision + 1,
          pokemon: body.pokemon_ids.map((id: number) =>
            member(id, body.sprite_selections, team.pokemon),
          ),
        });
      }
      return json(team);
    }
    if (path === "/types") return json(types);
    if (path === "/pokemon") {
      const search = url.searchParams.get("search") || "";
      const selectedTypes = (
        url.searchParams.get("types") ||
        url.searchParams.get("type") ||
        ""
      )
        .split(",")
        .filter(Boolean);
      const generation = url.searchParams.get("generation");
      const category = url.searchParams.get("category");
      const pokemon = dataset.filter(
        (p) =>
          p.name.includes(search.toLowerCase()) &&
          (!generation || p.generation === Number(generation)) &&
          (!category ||
            (category === "starter"
              ? p.is_starter
              : category === "legendary"
                ? p.is_legendary
                : p.is_mythical)) &&
          (!selectedTypes.length ||
            (url.searchParams.get("type_match") === "any"
              ? selectedTypes.some((type) => p.types.includes(type))
              : selectedTypes.every((type) => p.types.includes(type)))),
      );
      return json({
        pokemon,
        total: pokemon.length,
        offset: 0,
        limit: 20,
        catalog_revision: 1,
        generations: [1, 2],
      });
    }
    if (path.startsWith("/pokemon/"))
      return json(
        dataset.find(
          (pokemon) => pokemon.id === Number(path.split("/").at(-1)),
        ) || bulbasaur,
      );
    if (path === "/notifications") {
      const notifications =
        change && teams.length
          ? [
              {
                id: 10,
                pokemon: bulbasaur,
                affected_team_ids: [teams[0].id],
                detected_at: new Date().toISOString(),
                descriptions: ["Base attack changed"],
                differences: {},
              },
            ]
          : [];
      change = false;
      return json({
        notifications: notifications.length
          ? [...notifications, ...notifications]
          : [],
        total: notifications.length,
        limit: 20,
        offset: 0,
      });
    }
    return json({}, 404);
  });
}
async function chooseType(page: Page, name: string) {
  const checkbox = page.getByRole("checkbox", { name: `${name} type` });
  if (!(await checkbox.isChecked())) await checkbox.click();
  // URL-backed controls update after router navigation, so await the rendered state.
  await expect(checkbox).toBeChecked();
}

test("complete team workflow, notifications, and direct links", async ({
  page,
}) => {
  await backend(page);
  await page.goto("/");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("link", { name: "Build a team" }).click();
  await page.getByLabel("Team name").fill("First team");
  await page.getByRole("searchbox").fill("bulba");
  await expect(page.locator(".pokemon")).toHaveCount(1);
  await page
    .locator(".pokemon")
    .getByRole("link", { name: /bulbasaur/ })
    .click();
  await expect(page.getByRole("heading", { name: "Base stats" })).toBeVisible();
  await page.getByRole("button", { name: "Add Pokémon to draft" }).click();
  await page.getByRole("link", { name: "Back to draft" }).click();
  await page.getByRole("searchbox").fill("charm");
  await page.getByRole("button", { name: "Add Pokémon", exact: true }).click();
  await page.getByRole("button", { name: "Move charmander left" }).click();
  await page.getByRole("button", { name: "Save team" }).click();
  await expect(page).toHaveURL(/\/teams\/team-1$/);
  await page.reload();
  await expect(page.getByLabel("Team name")).toHaveValue("First team");
  await expect(page.locator(".roster-member").first()).toContainText(
    "charmander",
  );
  await page.getByRole("link", { name: "Find counter team" }).click();
  await page.getByRole("button", { name: "Generate counter team" }).click();
  await page.getByLabel("New team name").fill("Rival team");
  await page.getByRole("button", { name: "Save as new team" }).click();
  await expect(page).toHaveURL(/\/teams\/team-2$/);
  await page.getByLabel("Team name").fill("Edited rival");
  await page.getByRole("button", { name: "Save team" }).click();
  await expect(page.getByRole("status")).toHaveText("Team saved.");
  await page.getByRole("link", { name: "Changes", exact: true }).click();
  await expect(page.getByText("Base attack changed")).toHaveCount(1);
  await page.getByRole("button", { name: "Refresh changes" }).click();
  await expect(
    page.getByText("No changes for your teams in the last seven days."),
  ).toBeVisible();
  await page.goto("/teams/team-2");
  page.on("dialog", (dialog) => dialog.accept());
  await page.getByRole("button", { name: "Delete team" }).click();
  await expect(page).toHaveURL("/");
  await expect(page.getByText("Edited rival")).toHaveCount(0);
  await page.goto("/pokemon/1");
  await expect(page.getByRole("heading", { name: "Base stats" })).toBeVisible();
});
test("keyboard navigation, public exploration and staged add", async ({
  page,
}) => {
  await backend(page);
  await page.goto("/pokemon");
  await page.keyboard.press("Tab");
  await expect(
    page.getByRole("link", { name: "Skip to content" }),
  ).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page.locator("main")).toBeFocused();
  await chooseType(page, "Fire");
  await expect(page.locator(".pokemon")).toHaveCount(1);
  await page.getByRole("button", { name: "Add to team" }).click();
  await page.getByRole("button", { name: "Start new draft" }).click();
  await expect(page.locator(".roster")).toContainText("charmander");
  await expect(page.getByRole("button", { name: "Save team" })).toBeDisabled();
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByLabel("Team name").fill("Keyboard team");
  const save = page.getByRole("button", { name: "Save team" });
  await expect(save).toBeEnabled();
  await save.focus();
  await expect(save).toBeFocused();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/teams\/team-1$/);
});

test("sprite previews, saved selections, reordering and independent teams", async ({
  page,
}) => {
  await backend(page);
  await page.goto("/pokemon/1");
  await page.getByRole("button", { name: "Get started" }).click();
  await page
    .getByRole("combobox", { name: "Sprite for bulbasaur" })
    .selectOption("front_shiny");
  await expect(page.locator(".detail img")).toHaveAttribute(
    "src",
    "https://example.com/shiny.png",
  );
  await page.getByRole("button", { name: "Add to team" }).click();
  await page.getByRole("button", { name: "Start new draft" }).click();
  await page.getByLabel("Team name").fill("Originals");
  await expect(
    page.getByRole("combobox", { name: "Sprite for bulbasaur" }),
  ).toHaveValue("front_shiny");
  await page.getByRole("button", { name: "Save team" }).click();
  await expect(page).toHaveURL(/\/teams\/team-1$/);
  await page.reload();
  await expect(
    page.getByRole("combobox", { name: "Sprite for bulbasaur" }),
  ).toHaveValue("front_shiny");
  await page.getByRole("button", { name: "Add Pokémon", exact: true }).click();
  await page
    .getByRole("combobox", { name: "Sprite for charmander" })
    .selectOption("versions/generation-i/red-blue/back_default");
  await page
    .getByRole("combobox", { name: "Sprite for bulbasaur" })
    .selectOption("other/official-artwork/front_default");
  await page.getByRole("button", { name: "Move charmander left" }).click();
  await page.getByRole("button", { name: "Save team" }).click();
  await expect(page.getByRole("status")).toHaveText("Team saved.");
  await page.reload();
  await expect(page.locator(".roster-member").first()).toContainText(
    "charmander",
  );
  await expect(
    page.getByRole("combobox", { name: "Sprite for charmander" }),
  ).toHaveValue("versions/generation-i/red-blue/back_default");
  await expect(
    page.getByRole("combobox", { name: "Sprite for bulbasaur" }),
  ).toHaveValue("other/official-artwork/front_default");
  await page.goto("/pokemon/1");
  await page
    .getByRole("combobox", { name: "Sprite for bulbasaur" })
    .selectOption("other/showdown/front_default");
  await expect(page.locator(".detail img")).toHaveAttribute(
    "src",
    "https://example.com/animated.gif",
  );
  await page.getByRole("button", { name: "Add to team" }).click();
  await page.getByRole("button", { name: "Start new draft" }).click();
  await page.getByLabel("Team name").fill("Animated");
  await page.getByRole("button", { name: "Save team" }).click();
  await expect(page).toHaveURL(/\/teams\/team-2$/);
  await page.reload();
  await expect(
    page.getByRole("combobox", { name: "Sprite for bulbasaur" }),
  ).toHaveValue("other/showdown/front_default");
  await page.goto("/teams/team-1");
  await expect(
    page.getByRole("combobox", { name: "Sprite for bulbasaur" }),
  ).toHaveValue("other/official-artwork/front_default");
});

test("combined type, generation and group filters in explorer and team builder", async ({
  page,
}) => {
  await backend(page, true);
  await page.goto("/pokemon");
  await chooseType(page, "Fire");
  await chooseType(page, "Flying");
  await expect(page.locator(".pokemon")).toHaveCount(1);
  await expect(page.locator(".pokemon")).toContainText("charizard");
  await expect(page).toHaveURL(/types=fire%2Cflying/);
  await page
    .getByRole("combobox", { name: "Type matching" })
    .selectOption("any");
  await expect(page.locator(".pokemon")).toHaveCount(2);
  await page.goBack();
  await expect(
    page.getByRole("combobox", { name: "Type matching" }),
  ).toHaveValue("all");
  await expect(page.locator(".pokemon")).toHaveCount(1);
  await page.getByRole("button", { name: "Clear filters" }).click();
  await chooseType(page, "Grass");
  await page.getByRole("combobox", { name: "Introduced in" }).selectOption("2");
  await page
    .getByRole("combobox", { name: "Pokémon group" })
    .selectOption("starter");
  await expect(page.locator(".pokemon")).toHaveCount(1);
  await expect(page.locator(".pokemon")).toContainText("bayleef");
  await page.reload();
  await expect(
    page.getByRole("checkbox", { name: "Grass type" }),
  ).toBeChecked();
  await expect(
    page.getByRole("combobox", { name: "Introduced in" }),
  ).toHaveValue("2");
  await expect(
    page.getByRole("combobox", { name: "Pokémon group" }),
  ).toHaveValue("starter");
  await page.getByRole("button", { name: "Clear filters" }).click();
  await page.getByRole("combobox", { name: "Introduced in" }).selectOption("2");
  await page
    .getByRole("combobox", { name: "Pokémon group" })
    .selectOption("legendary");
  await expect(page.locator(".pokemon")).toHaveCount(1);
  await expect(page.locator(".pokemon")).toContainText("suicune");
  await page
    .getByRole("combobox", { name: "Pokémon group" })
    .selectOption("mythical");
  await expect(page.getByText("No Pokémon match these filters.")).toBeVisible();
  await page.getByRole("combobox", { name: "Introduced in" }).selectOption("");
  await expect(page.locator(".pokemon")).toHaveCount(1);
  await expect(page.locator(".pokemon")).toContainText("mew");
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByRole("link", { name: "Your teams" }).click();
  await page.getByRole("link", { name: "Build a team" }).click();
  await page.getByLabel("Team name").fill("Filtered starters");
  await chooseType(page, "Fire");
  await chooseType(page, "Flying");
  await page
    .getByRole("combobox", { name: "Pokémon group" })
    .selectOption("starter");
  await expect(page.locator(".pokemon")).toHaveCount(1);
  await page.getByRole("button", { name: "Add Pokémon", exact: true }).click();
  await expect(page.locator(".roster")).toContainText("charizard");
  await expect(page.getByLabel("Team name")).toHaveValue("Filtered starters");
  await page.getByRole("button", { name: "Save team" }).click();
  await expect(page).toHaveURL(/\/teams\/team-1$/);
});
