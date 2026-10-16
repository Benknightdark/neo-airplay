import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { join, resolve } from "node:path";

const result = spawnSync(
  "bun",
  ["run", "tauri", "build", "--debug", "--no-bundle", "--features", "wdio", "--config", "src-tauri/tauri.e2e.conf.json"],
  {
    cwd: process.cwd(),
    env: { ...process.env, VITE_WDIO: "true", NEO_AIRPLAY_SKIP_SETUP: "1" },
    encoding: "utf8",
    maxBuffer: 32 * 1024 * 1024,
  },
);

if (result.error) {
  throw result.error;
}

if (result.status !== 0) {
  process.stderr.write(result.stdout ?? "");
  process.stderr.write(result.stderr ?? "");
  process.exitCode = result.status ?? 1;
} else {
  const targetDir = resolve(process.env.CARGO_TARGET_DIR ?? "src-tauri/target");
  const fixtureDirectory = join(targetDir, "neo-airplay-e2e-engine");
  const fixtureName = `fake-engine${process.platform === "win32" ? ".exe" : ""}`;
  mkdirSync(fixtureDirectory, { recursive: true });

  const fixtureResult = spawnSync(
    "rustc",
    [resolve("e2e/fake-engine.rs"), "-O", "-o", join(fixtureDirectory, fixtureName)],
    { cwd: process.cwd(), encoding: "utf8", maxBuffer: 8 * 1024 * 1024 },
  );

  if (fixtureResult.error) {
    throw fixtureResult.error;
  }

  if (fixtureResult.status !== 0) {
    process.stderr.write(fixtureResult.stdout ?? "");
    process.stderr.write(fixtureResult.stderr ?? "");
    process.exitCode = fixtureResult.status ?? 1;
  }
}
