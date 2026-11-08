import { spawnSync } from "node:child_process";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

if (process.env.NEO_AIRPLAY_SKIP_SETUP !== "1") {
  const windows = process.platform === "win32";
  const script = fileURLToPath(new URL(`./install.${windows ? "ps1" : "sh"}`, import.meta.url));
  const engineDirectory = resolve(fileURLToPath(new URL("../engine/", import.meta.url)));
  const result = spawnSync(windows ? "powershell.exe" : "/bin/bash", windows
    ? ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script, "-EngineDirectory", engineDirectory]
    : [script, "--engine-dir", engineDirectory], {
    stdio: "inherit",
    env: { ...process.env, NEO_AIRPLAY_ENGINE_DIR: engineDirectory },
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
