import { readFileSync, mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { execFileSync } from "node:child_process";
const dir = mkdtempSync(join(tmpdir(), "poketeam-api-"));
try {
  const output = join(dir, "generated.d.ts");
  execFileSync(
    process.execPath,
    [
      "node_modules/openapi-typescript/bin/cli.js",
      "../backend/openapi.json",
      "-o",
      output,
    ],
    { stdio: "inherit" },
  );
  if (
    readFileSync(output, "utf8") !==
    readFileSync("src/api/generated.d.ts", "utf8")
  )
    throw new Error("API types drifted. Run npm run generate:api.");
} finally {
  rmSync(dir, { recursive: true, force: true });
}
