import { spawnSync } from "node:child_process";
import { mkdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const testDirectory = resolve(scriptDirectory, "..");
const desktopDirectory = resolve(testDirectory, "../desktop");
const targetDirectory = resolve(desktopDirectory, process.env.CARGO_TARGET_DIR ?? "src-tauri/target");
const e2eConfig = join(testDirectory, "e2e", "tauri.e2e.conf.json");
const fakeEngineSource = join(testDirectory, "e2e", "fake-engine.rs");

const result = spawnSync(
  "bun",
  ["run", "tauri", "build", "--debug", "--no-bundle", "--features", "wdio", "--config", e2eConfig],
  {
    cwd: desktopDirectory,
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
  const fixtureDirectory = join(targetDirectory, "neo-airplay-e2e-engine");
  const fixtureName = `fake-engine${process.platform === "win32" ? ".exe" : ""}`;
  mkdirSync(fixtureDirectory, { recursive: true });

  const fixtureResult = spawnSync(
    "rustc",
    [fakeEngineSource, "-O", "-o", join(fixtureDirectory, fixtureName)],
    { cwd: desktopDirectory, encoding: "utf8", maxBuffer: 8 * 1024 * 1024 },
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
