import { expect, test, type Page } from "@playwright/test";
import { bulbasaur, charmander, counter, types } from "../tests/fixtures";
import type { Team } from "../src/api/client";
async function backend(page: Page) {
  let session = false;
  let nextId = 1;
  const teams: Team[] = [];
  let change = true;
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
          id === 1 ? bulbasaur : charmander,
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
            id === 1 ? bulbasaur : charmander,
          ),
        });
      }
      return json(team);
    }
    if (path === "/types") return json(types);
    if (path === "/pokemon") {
      const search = url.searchParams.get("search") || "";
      const type = url.searchParams.get("type");
      const pokemon = [bulbasaur, charmander].filter(
        (p) => p.name.includes(search) && (!type || p.types.includes(type)),
      );
      return json({
        pokemon,
        total: pokemon.length,
        offset: 0,
        limit: 20,
        catalog_revision: 1,
      });
    }
    if (path.startsWith("/pokemon/"))
      return json(path.endsWith("/4") ? charmander : bulbasaur);
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
  await page
    .getByRole("combobox", { name: "Type", exact: true })
    .selectOption("fire");
  await expect(page.locator(".pokemon")).toHaveCount(1);
  await page.getByRole("button", { name: "Add to team" }).click();
  await page.getByRole("button", { name: "Start new draft" }).click();
  await expect(page.locator(".roster")).toContainText("charmander");
  await expect(page.getByRole("button", { name: "Save team" })).toBeDisabled();
  await page.getByRole("button", { name: "Get started" }).click();
  await page.getByLabel("Team name").fill("Keyboard team");
  await page.getByRole("button", { name: "Save team" }).focus();
  await page.keyboard.press("Enter");
  await expect(page).toHaveURL(/\/teams\/team-1$/);
});
