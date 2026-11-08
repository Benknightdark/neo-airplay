import { chmodSync, copyFileSync, mkdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const targetDir = resolve(process.env.CARGO_TARGET_DIR ?? "src-tauri/target");
const appBinaryPath = join(targetDir, "debug", `neo-airplay${process.platform === "win32" ? ".exe" : ""}`);
const profileOwner = process.env.NEO_AIRPLAY_WDIO_PROFILE === undefined;
const profilePath =
  process.env.NEO_AIRPLAY_WDIO_PROFILE ?? join(tmpdir(), `neo-airplay-wdio-${process.pid}`);
process.env.NEO_AIRPLAY_WDIO_PROFILE = profilePath;
process.env.NEO_AIRPLAY_E2E_APP_BINARY = appBinaryPath;
process.env.NEO_AIRPLAY_E2E_PROFILE = profilePath;
if (profileOwner) {
  rmSync(profilePath, { force: true, recursive: true });
  mkdirSync(profilePath, { recursive: true });

  const fixtureName = `fake-engine${process.platform === "win32" ? ".exe" : ""}`;
  const fixturePath = join(targetDir, "neo-airplay-e2e-engine", fixtureName);
  const engineDirectory = join(profilePath, "engine");
  mkdirSync(engineDirectory, { recursive: true });
  for (const engineName of ["adb", "scrcpy", "uxplay"]) {
    const enginePath = join(engineDirectory, `${engineName}${process.platform === "win32" ? ".exe" : ""}`);
    copyFileSync(fixturePath, enginePath);
    if (process.platform !== "win32") chmodSync(enginePath, 0o755);
  }
}

export const config = {
  runner: "local",
  logLevel: "error",
  specs: ["./e2e/**/*.spec.ts"],
  maxInstances: 1,
  capabilities: [{
    browserName: "tauri",
    "tauri:options": {
      application: appBinaryPath,
      args: ["--e2e-profile", profilePath],
      env: { NEO_AIRPLAY_E2E_PROFILE: profilePath },
    },
  }],
  services: [["@wdio/tauri-service", {
    appBinaryPath,
    appArgs: ["--e2e-profile", profilePath],
    env: { NEO_AIRPLAY_E2E_PROFILE: profilePath },
    driverProvider: "embedded",
    startTimeout: 90_000,
    captureBackendLogs: true,
    captureFrontendLogs: true,
  }]],
  framework: "mocha",
  reporters: ["spec"],
  mochaOpts: { timeout: 60_000 },
  onComplete: () => {
    if (process.env.NEO_AIRPLAY_KEEP_E2E_PROFILE !== "1") {
      rmSync(profilePath, { force: true, recursive: true });
    }
  },
};
