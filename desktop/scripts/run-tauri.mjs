import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { dirname, join, posix, resolve, win32 } from "node:path";
import { homedir } from "node:os";
import { fileURLToPath } from "node:url";

const scriptDirectory = dirname(fileURLToPath(import.meta.url));
const defaultDesktopDirectory = resolve(scriptDirectory, "..");
const nodeVersion = readFileSync(join(defaultDesktopDirectory, ".node-version"), "utf8").trim();

export function getSetupCommand(platform, scriptsDirectory) {
  const joinPath = platform === "win32" ? win32.join : posix.join;
  const setupScript = joinPath(scriptsDirectory, platform === "win32" ? "setup.ps1" : "setup.sh");
  return platform === "win32"
    ? ["powershell.exe", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", setupScript]]
    : ["/bin/bash", [setupScript]];
}

export function withDevelopmentPath(environment, platform, desktopDirectory, homeDirectory) {
  const joinPath = platform === "win32" ? win32.join : posix.join;
  const pathKey = Object.keys(environment).find((key) => key.toLowerCase() === "path") ?? "PATH";
  const pathDelimiter = platform === "win32" ? win32.delimiter : posix.delimiter;
  const home = homeDirectory ?? environment.HOME ?? environment.USERPROFILE ?? homedir();
  const desktop = desktopDirectory;
  const additions = platform === "win32"
    ? [
      joinPath(environment.LOCALAPPDATA ?? joinPath(home, "AppData", "Local"), "NeoAirPlay", `node-v${nodeVersion}`),
      joinPath(home, ".cargo", "bin"),
      joinPath(home, ".bun", "bin"),
      joinPath(desktop, "node_modules", ".bin"),
    ]
    : [
      joinPath(home, ".local", "bin"),
      joinPath(home, ".bun", "bin"),
      joinPath(home, ".cargo", "bin"),
      ...(platform === "darwin"
        ? [
          "/opt/homebrew/opt/node@22/bin",
          "/usr/local/opt/node@22/bin",
          "/opt/homebrew/bin",
          "/opt/homebrew/sbin",
          "/usr/local/bin",
          "/usr/local/sbin",
        ]
        : []),
      joinPath(desktop, "node_modules", ".bin"),
    ];
  const existing = (environment[pathKey] ?? "").split(pathDelimiter).filter(Boolean);
  const seen = new Set();
  const pathEntries = [...additions, ...existing].filter((entry) => {
    const normalized = platform === "win32" ? entry.toLowerCase() : entry;
    if (seen.has(normalized)) return false;
    seen.add(normalized);
    return true;
  });

  return { ...environment, [pathKey]: pathEntries.join(pathDelimiter) };
}

export function runTauri(args, {
  platform = process.platform,
  environment = process.env,
  spawn = spawnSync,
  scriptsDirectory = scriptDirectory,
  desktopDirectory = defaultDesktopDirectory,
  homeDirectory,
} = {}) {
  let childEnvironment = { ...environment };
  const setupAlreadyCompleted = childEnvironment.NEO_AIRPLAY_SETUP_COMPLETE === "1";
  const setupDisabled = childEnvironment.NEO_AIRPLAY_SKIP_SETUP === "1";

  if (!setupAlreadyCompleted && !setupDisabled) {
    const [setupExecutable, setupArguments] = getSetupCommand(platform, scriptsDirectory);
    const setupResult = spawn(setupExecutable, setupArguments, {
      cwd: resolve(scriptsDirectory, ".."),
      env: childEnvironment,
      stdio: "inherit",
    });
    if (setupResult.error) throw setupResult.error;
    if (setupResult.status !== 0) return setupResult.status ?? 1;
  }

  childEnvironment = withDevelopmentPath(childEnvironment, platform, desktopDirectory, homeDirectory);
  childEnvironment.NEO_AIRPLAY_SETUP_COMPLETE = "1";
  const joinPath = platform === "win32" ? win32.join : posix.join;
  const cliEntry = joinPath(desktopDirectory, "node_modules", "@tauri-apps", "cli", "tauri.js");
  const result = spawn("node", [cliEntry, ...args], {
    cwd: desktopDirectory,
    env: childEnvironment,
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  return result.status ?? 1;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  process.exitCode = runTauri(process.argv.slice(2));
}
