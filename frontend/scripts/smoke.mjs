import assert from "node:assert/strict";
const origin = process.env.FRONTEND_ORIGIN || "http://127.0.0.1:5173";
const base = process.env.API_BASE || origin;
let cookie;
async function call(path, method = "GET", body, selectedOrigin = origin) {
  return fetch(`${base}/api/v1${path}`, {
    method,
    headers: {
      ...(cookie ? { Cookie: cookie } : {}),
      ...(body === undefined
        ? {}
        : { "Content-Type": "application/json", Origin: selectedOrigin }),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
}
assert.equal((await fetch(origin)).status, 200, "Frontend must be serving");
assert.equal(
  (await fetch(`${base}/health/live`)).status,
  200,
  "API must be live",
);
assert.equal((await call("/teams")).status, 401);
assert.equal(
  (await call("/session", "POST", {}, "http://wrong-origin.invalid")).status,
  400,
);
const session = await call("/session", "POST", {});
assert.equal(session.status, 200);
cookie = session.headers.get("set-cookie")?.split(";")[0];
assert.ok(cookie);
assert.match(session.headers.get("set-cookie"), /HttpOnly/i);
assert.ok((await session.json()).owner_id);
let id;
try {
  const catalog = await call("/pokemon?limit=1");
  let ids = [];
  if (catalog.status === 503) {
    assert.equal((await catalog.json()).code, "catalog_unavailable");
    console.log(
      "LIMITATION: catalog is uninitialized; live Pokémon/counter validation skipped.",
    );
  } else {
    assert.equal(catalog.status, 200);
    const page = await catalog.json();
    assert.ok(Array.isArray(page.pokemon));
    ids = page.pokemon.map((p) => p.id);
  }
  const created = await call("/teams", "POST", {
    name: `Frontend smoke ${Date.now()}`,
    pokemon_ids: ids,
  });
  assert.equal(created.status, 201);
  const team = await created.json();
  id = team.id;
  assert.equal(team.pokemon.length, ids.length);
  const updated = await call(`/teams/${id}`, "PUT", {
    name: team.name,
    pokemon_ids: ids,
    revision: team.revision,
  });
  assert.equal(updated.status, 200);
  assert.equal(
    (
      await call(`/teams/${id}`, "PUT", {
        name: team.name,
        pokemon_ids: ids,
        revision: team.revision,
      })
    ).status,
    409,
  );
  assert.equal((await call(`/teams/${id}`)).status, 200);
  if (ids.length) {
    const counter = await call(`/teams/${id}/counter-team`, "POST", {
      mode: "power",
    });
    assert.equal(counter.status, 200);
    assert.ok(Array.isArray((await counter.json()).pokemon));
  }
  console.log(
    "PASS: frontend HTTP, API liveness, cookies, exact Origin validation, session, team CRUD and revision conflict.",
  );
} finally {
  if (id) {
    const deleted = await call(`/teams/${id}`, "DELETE", {});
    assert.equal(deleted.status, 204);
    assert.equal(await deleted.text(), "");
  }
}
