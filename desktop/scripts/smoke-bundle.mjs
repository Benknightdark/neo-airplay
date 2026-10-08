import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const targetDirectory = resolve(process.env.CARGO_TARGET_DIR ?? "src-tauri/target");
let executablePath;

if (process.platform === "darwin") {
  executablePath = join(
    targetDirectory,
    "release/bundle/macos/Neo AirPlay.app/Contents/MacOS/neo-airplay",
  );
} else if (process.platform === "win32") {
  executablePath = join(targetDirectory, "release/neo-airplay.exe");
} else if (process.platform === "linux") {
  const appImageDirectory = join(targetDirectory, "release/bundle/appimage");
  if (existsSync(appImageDirectory)) {
    const appDirectories = readdirSync(appImageDirectory)
      .filter((fileName) => fileName.endsWith(".AppDir"))
      .sort();
    if (appDirectories.length > 1) {
      throw new Error(`預期最多找到一個 AppDir，實際找到 ${appDirectories.length} 個。`);
    }
    if (appDirectories.length === 1) {
      executablePath = join(appImageDirectory, appDirectories[0], "usr/bin/neo-airplay");
    }
  }
  executablePath ??= "/usr/bin/neo-airplay";
} else {
  throw new Error(`不支援的作業系統：${process.platform}`);
}

if (!existsSync(executablePath)) {
  throw new Error(`找不到打包後的應用程式：${executablePath}`);
}

const temporaryHome = mkdtempSync(join(tmpdir(), "neo-airplay-bundle-smoke-"));
const environment = { ...process.env };

if (process.platform === "win32") {
  environment.APPDATA = join(temporaryHome, "AppData/Roaming");
  environment.LOCALAPPDATA = join(temporaryHome, "AppData/Local");
  environment.USERPROFILE = temporaryHome;
  mkdirSync(environment.APPDATA, { recursive: true });
  mkdirSync(environment.LOCALAPPDATA, { recursive: true });
} else {
  environment.HOME = temporaryHome;
  if (process.platform === "linux") {
    environment.XDG_CONFIG_HOME = join(temporaryHome, "config");
    environment.XDG_DATA_HOME = join(temporaryHome, "data");
    environment.XDG_STATE_HOME = join(temporaryHome, "state");
    environment.XDG_CACHE_HOME = join(temporaryHome, "cache");
    for (const directory of [
      environment.XDG_CONFIG_HOME,
      environment.XDG_DATA_HOME,
      environment.XDG_STATE_HOME,
      environment.XDG_CACHE_HOME,
    ]) {
      mkdirSync(directory, { recursive: true });
    }
  }
}

let child;
let exitResult;
let diagnostics = "";

function waitFor(promise, timeoutMilliseconds) {
  let timer;
  const timeout = new Promise((resolveTimeout) => {
    timer = setTimeout(() => resolveTimeout(null), timeoutMilliseconds);
  });
  return Promise.race([promise, timeout]).finally(() => clearTimeout(timer));
}

function terminateSmokeApplication(signal) {
  if (!child?.pid) return;

  if (process.platform === "win32") {
    const result = spawnSync(
      "taskkill.exe",
      ["/PID", String(child.pid), "/T", "/F"],
      { stdio: "ignore" },
    );
    if (!result.error && result.status === 0) return;
    child.kill(signal);
    return;
  }

  child.kill(signal);
}

try {
  child = spawn(executablePath, [], {
    cwd: temporaryHome,
    env: environment,
    stdio: ["ignore", "pipe", "pipe"],
  });

  exitResult = new Promise((resolveExit) => {
    child.once("error", (error) => resolveExit({ kind: "error", error }));
    child.once("exit", (code, signal) => resolveExit({ kind: "exit", code, signal }));
  });

  for (const stream of [child.stdout, child.stderr]) {
    stream?.on("data", (chunk) => {
      diagnostics = (diagnostics + chunk.toString()).slice(-8192);
    });
  }

  const startupResult = await waitFor(exitResult, 8_000);
  if (startupResult) {
    const detail =
      startupResult.kind === "error"
        ? startupResult.error.message
        : `結束代碼 ${startupResult.code ?? "無"}，訊號 ${startupResult.signal ?? "無"}`;
    throw new Error(`打包後的應用程式未能保持執行：${detail}\n${diagnostics}`);
  }

  terminateSmokeApplication("SIGTERM");
  let shutdownResult = await waitFor(exitResult, 5_000);
  if (!shutdownResult) {
    terminateSmokeApplication("SIGKILL");
    shutdownResult = await waitFor(exitResult, 2_000);
  }
  if (!shutdownResult) {
    throw new Error(`無法停止打包後的應用程式。\n${diagnostics}`);
  }
  if (shutdownResult.kind === "error") {
    throw new Error(`無法停止打包後的應用程式：${shutdownResult.error.message}\n${diagnostics}`);
  }

  console.log(`打包後應用程式啟動檢查通過（${process.platform}）。`);
} finally {
  if (child && child.exitCode === null && child.signalCode === null) {
    terminateSmokeApplication("SIGKILL");
    if (exitResult) await waitFor(exitResult, 2_000);
  }
  rmSync(temporaryHome, { recursive: true, force: true });
}
