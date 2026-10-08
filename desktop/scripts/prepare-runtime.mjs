import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

if (process.env.NEO_AIRPLAY_SKIP_SETUP !== "1") {
  const windows = process.platform === "win32";
  const script = fileURLToPath(new URL(`../src-tauri/resources/setup/install.${windows ? "ps1" : "sh"}`, import.meta.url));
  const result = spawnSync(windows ? "powershell.exe" : "/bin/bash", windows
    ? ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", script]
    : [script], { stdio: "inherit" });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}
