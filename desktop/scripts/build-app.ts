import { spawnSync } from "node:child_process";

const result = spawnSync("bun", ["run", "tauri", "build", "--ci"], {
  cwd: process.cwd(),
  stdio: "inherit",
});

if (result.error) {
  throw result.error;
}

if (result.status !== 0) {
  process.exitCode = result.status ?? 1;
}
