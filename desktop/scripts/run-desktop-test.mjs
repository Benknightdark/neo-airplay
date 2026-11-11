import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const [script, ...args] = process.argv.slice(2);
if (!script) {
  throw new Error("請指定 desktop-test 中的執行項目。");
}

const testProjectDirectory = resolve(dirname(fileURLToPath(import.meta.url)), "../../desktop-test");
const result = spawnSync("bun", ["run", script, ...args], {
  cwd: testProjectDirectory,
  stdio: "inherit",
});

if (result.error) {
  throw result.error;
}

process.exitCode = result.status ?? 1;
