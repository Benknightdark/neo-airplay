import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdtempSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { delimiter, join } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import test from "node:test";
import { getSetupCommand, runTauri, withDevelopmentPath } from "./run-tauri.mjs";

const installerPath = fileURLToPath(new URL("./install.sh", import.meta.url));
const powershellInstallerPath = fileURLToPath(new URL("./install.ps1", import.meta.url));
const runtimeEntryPath = fileURLToPath(new URL("./prepare-runtime.mjs", import.meta.url));
const shellChecksUnsupported = process.platform === "win32";

function findHostCommand(commandName) {
  for (const directory of (process.env.PATH ?? "").split(delimiter)) {
    const candidate = join(directory, commandName);
    if (existsSync(candidate)) return candidate;
  }
  throw new Error(`Host utility not found: ${commandName}`);
}

function snapshotDirectory(directory) {
  return readdirSync(directory, { withFileTypes: true })
    .sort((left, right) => left.name.localeCompare(right.name))
    .map((entry) => {
      const path = join(directory, entry.name);
      return [entry.name, entry.isDirectory() ? snapshotDirectory(path) : readFileSync(path, "utf8")];
    });
}

function createScenario(options = {}) {
  const root = mkdtempSync(join(tmpdir(), "neo-airplay-setup-check-"));
  const home = join(root, "home");
  const fakeBin = join(root, "fake-bin");
  const utilityBin = join(root, "utility-bin");
  const engineDirectory = join(root, "engine");
  const engineBin = join(engineDirectory, "bin");
  const engineGstreamerPlugins = join(engineDirectory, "lib", "gstreamer-1.0");
  const temporaryFiles = join(root, "tmp");
  const gstreamerPlugins = join(root, "gstreamer-plugins");
  const gstreamerScannerDirectory = join(root, "gstreamer-scanner");
  const packageManagerMarker = join(root, "package-manager-was-called");
  const brewMarker = join(root, "brew-was-called");
  const cltInstallMarker = join(root, "clt-install-was-requested");
  const scrcpyDownloadMarker = join(root, "scrcpy-download-was-called");
  const gstInspectMarker = join(root, "gst-inspect-calls");
  mkdirSync(home);
  mkdirSync(fakeBin);
  mkdirSync(utilityBin);
  mkdirSync(temporaryFiles);
  mkdirSync(engineBin, { recursive: true });
  mkdirSync(engineGstreamerPlugins, { recursive: true });
  mkdirSync(gstreamerPlugins);
  mkdirSync(gstreamerScannerDirectory);
  writeFileSync(join(gstreamerScannerDirectory, "gst-plugin-scanner"), "#!/bin/sh\nexit 0\n");
  chmodSync(join(gstreamerScannerDirectory, "gst-plugin-scanner"), 0o755);

  for (const utility of ["awk", "cat", "chmod", "cp", "dirname", "find", "grep", "head", "ln", "mkdir", "mktemp", "mv", "rm", "sed"]) {
    symlinkSync(findHostCommand(utility), join(utilityBin, utility));
  }
  for (const pluginFile of ["libgstplayback.dylib", "libgstvideoparsersbad.dylib", "libgstvideoconvertscale.dylib", "libgstautodetect.dylib", "libgstlibav.dylib", "libgstosxvideo.dylib"]) {
    writeFileSync(join(engineGstreamerPlugins, pluginFile), "fixture GStreamer plugin\n");
  }

  const fakeCommands = {
    uname: '#!/bin/sh\ncase "$1" in -m) printf "%s\\n" "${FAKE_ARCH:-x86_64}" ;; *) printf "%s\\n" "${FAKE_UNAME:-Linux}" ;; esac\n',
    uxplay: '#!/bin/sh\nprintf "*=== GStreamer >= 1.22 on macOS ===*\\nUxPlay version %s; for help, use option \\"-h\\"\\n" "${FAKE_UXPLAY_VERSION:-1.73.7}"\n',
    "gst-inspect-1.0": `#!/bin/sh
printf '%s\\n' "$*" >> "$GST_INSPECT_MARKER"
if [ "\${1:-}" = "--print-plugin-auto-install-info" ]; then
  case "\${FAKE_GST_SCAN_MODE:-}" in
    error) printf "fake GStreamer scan error\\n" >&2; exit 42 ;;
    timeout) printf "fake GStreamer scan timeout\\n" >&2; exit 124 ;;
  esac
  for element in decodebin h264parse videoconvert autoaudiosink avdec_aac osxvideosink avdec_h264; do
    if [ "$element" != "\${MISSING_GST_ELEMENT:-}" ]; then printf 'element-%s\\n' "$element"; fi
  done
  exit 0
fi
if [ "\${1:-}" = "\${MISSING_GST_ELEMENT:-}" ]; then exit 1; fi
exit 0
`,
    timeout: `#!/bin/sh
while [ "\${1#-}" != "$1" ]; do shift; done
shift
if [ "\${FAKE_GST_SCAN_MODE:-}" = "timeout" ] &&
  [ "\${1##*/}" = "gst-inspect-1.0" ] &&
  [ "\${2:-}" = "--print-plugin-auto-install-info" ]; then exit 124; fi
exec "$@"
`,
    gtimeout: `#!/bin/sh
while [ "\${1#-}" != "$1" ]; do shift; done
shift
if [ "\${FAKE_GST_SCAN_MODE:-}" = "timeout" ] &&
  [ "\${1##*/}" = "gst-inspect-1.0" ] &&
  [ "\${2:-}" = "--print-plugin-auto-install-info" ]; then exit 124; fi
exec "$@"
`,
    adb: '#!/bin/sh\ncase "${ADB_PROBE:-ok}" in fail) exit 1 ;; empty) exit 0 ;; *) printf "Android Debug Bridge version 1.0.41\\n" ;; esac\n',
    scrcpy: '#!/bin/sh\ncase "${SCRCPY_PROBE:-ok}" in fail) exit 1 ;; empty) exit 0 ;; *) printf "scrcpy 5.0.1\\n" ;; esac\n',
    "avahi-daemon": "#!/bin/sh\nexit 0\n",
    pgrep: '#!/bin/sh\n[ "${AVAHI_ACTIVE:-1}" = "1" ]\n',
    "pkg-config": '#!/bin/sh\ncase "${1:-}" in --exists) [ "${2:-}" != "${MISSING_PKG_MODULE:-}" ] ;; --variable=pluginsdir) printf "%s\\n" "$GSTREAMER_PLUGINS" ;; --variable=pluginscannerdir) printf "%s\\n" "$GSTREAMER_SCANNER_DIRECTORY" ;; *) exit 0 ;; esac\n',
    rustup: '#!/bin/sh\nif [ "${1:-}" = "component" ]; then printf "rustfmt-x86_64-unknown-linux-gnu\\nclippy-x86_64-unknown-linux-gnu\\n"; fi\nexit 0\n',
    rustc: '#!/bin/sh\nprintf "rustc 1.88.0 (isolated setup test)\\n"\n',
    cargo: '#!/bin/sh\nprintf "cargo 1.88.0 (isolated setup test)\\n"\n',
    cmake: "#!/bin/sh\nexit 0\n",
    cc: "#!/bin/sh\nexit 0\n",
    "c++": "#!/bin/sh\nexit 0\n",
    patchelf: "#!/bin/sh\nexit 0\n",
    xz: "#!/bin/sh\nexit 0\n",
    bun: '#!/bin/sh\nprintf "%s\\n" "${FAKE_BUN_VERSION:-1.4.2}"\n',
    node: '#!/bin/sh\nprintf "%s\\n" "${FAKE_NODE_VERSION:-v22.23.3}"\n',
    sudo: '#!/bin/sh\nexec "$@"\n',
    "apt-get": `#!/bin/sh\nprintf '%s\\n' "$*" >> "$PACKAGE_MANAGER_MARKER"\nexit "\${FAKE_PACKAGE_MANAGER_STATUS:-97}"\n`,
    curl: '#!/bin/sh\nwhile [ "$#" -gt 0 ]; do\n  if [ "$1" = "--output" ]; then shift; : > "$1"; fi\n  shift\ndone\nprintf download >> "$SCRCPY_DOWNLOAD_MARKER"\n',
    sha256sum: '#!/bin/sh\nprintf "%s  %s\\n" "$FAKE_SCRCPY_SHA256" "$1"\n',
    tar: '#!/bin/sh\n[ "${1:-}" = "-xzf" ] || exit 1\nshift\nshift\n[ "${1:-}" = "-C" ] || exit 1\ndestination="$2"\nbundle="$destination/scrcpy-linux-x86_64-v5.0.1"\nmkdir -p "$bundle"\nprintf "#!/bin/sh\\nprintf \\\"scrcpy 5.0.1\\\\n\\\"\\n" > "$bundle/scrcpy"\nprintf "fake scrcpy server\\n" > "$bundle/scrcpy-server"\nprintf "#!/bin/sh\\nprintf \\\"Android Debug Bridge version 1.0.41\\\\n\\\"\\n" > "$bundle/adb"\nchmod 0755 "$bundle/scrcpy" "$bundle/adb"\n',
    brew: `#!/bin/sh\nprintf '%s\\n' "$*" >> "$BREW_MARKER"\nexit 97\n`,
    "xcode-select": '#!/bin/sh\nif [ "${1:-}" = "--install" ]; then printf requested > "$CLT_INSTALL_MARKER"; exit 0; fi\n[ "${CLT_AVAILABLE:-1}" = "1" ]\n',
  };

  const missingCommands = new Set(options.missingCommands ?? []);
  const engineCommands = new Set(["uxplay", "gst-inspect-1.0", "timeout", "gtimeout", "adb", "scrcpy"]);
  for (const [commandName, contents] of Object.entries(fakeCommands)) {
    if (missingCommands.has(commandName)) {
      if (engineCommands.has(commandName)) {
        const systemOnlyPath = join(fakeBin, commandName);
        writeFileSync(systemOnlyPath, contents);
        chmodSync(systemOnlyPath, 0o755);
      }
      continue;
    }
    const commandPath = join(engineCommands.has(commandName) ? engineBin : fakeBin, commandName);
    writeFileSync(commandPath, contents);
    chmodSync(commandPath, 0o755);
  }
  if (!options.missingScrcpyServer) {
    writeFileSync(join(engineBin, "scrcpy-server"), options.emptyScrcpyServer ? "" : "fake scrcpy server\n");
  }
  const engineSnapshot = snapshotDirectory(engineDirectory);

  const environment = {
    ...process.env,
    HOME: home,
    PATH: [fakeBin, utilityBin].join(delimiter),
    NEO_AIRPLAY_ENGINE_DIR: engineDirectory,
    TMPDIR: temporaryFiles,
    GSTREAMER_PLUGINS: gstreamerPlugins,
    GSTREAMER_SCANNER_DIRECTORY: gstreamerScannerDirectory,
    FAKE_UNAME: options.uname ?? "Linux",
    FAKE_ARCH: "x86_64",
    FAKE_UXPLAY_VERSION: options.uxplayVersion ?? "1.73.7",
    FAKE_BUN_VERSION: options.bunVersion ?? "1.4.2",
    FAKE_NODE_VERSION: options.nodeVersion ?? "v22.23.3",
    SCRCPY_PROBE: options.scrcpyProbe ?? "ok",
    ADB_PROBE: options.adbProbe ?? "ok",
    MISSING_GST_ELEMENT: options.missingGstElement ?? "",
    FAKE_GST_SCAN_MODE: options.gstScanMode ?? "ok",
    MISSING_PKG_MODULE: options.missingPkgModule ?? "",
    AVAHI_ACTIVE: options.avahiActive === false ? "0" : "1",
    CLT_AVAILABLE: options.cltAvailable === false ? "0" : "1",
    FAKE_PACKAGE_MANAGER_STATUS: String(options.packageManagerStatus ?? 97),
    PACKAGE_MANAGER_MARKER: packageManagerMarker,
    BREW_MARKER: brewMarker,
    CLT_INSTALL_MARKER: cltInstallMarker,
    SCRCPY_DOWNLOAD_MARKER: scrcpyDownloadMarker,
    GST_INSPECT_MARKER: gstInspectMarker,
    FAKE_SCRCPY_SHA256: "9f969d30cc574816077edecda65719c1c70b0aee1df1ba5fa00779ac07b8fd6e",
  };

  return {
    root,
    home,
    packageManagerMarker,
    brewMarker,
    cltInstallMarker,
    scrcpyDownloadMarker,
    gstInspectMarker,
    engineDirectory,
    engineSnapshot,
    temporaryFiles,
    runInstaller(args, scriptPath = installerPath) {
      return spawnSync("/bin/bash", [scriptPath, "--engine-dir", engineDirectory, ...args], {
        cwd: root,
        env: environment,
        encoding: "utf8",
        timeout: 10_000,
        maxBuffer: 2 * 1024 * 1024,
      });
    },
    cleanup() {
      rmSync(root, { recursive: true, force: true });
    },
  };
}

function assertReadOnly(scenario) {
  assert.equal(existsSync(join(scenario.home, ".local")), false, "check mode must not create user install directories");
  assert.equal(existsSync(scenario.packageManagerMarker), false, "check mode must not call package managers");
  assert.equal(existsSync(scenario.brewMarker), false, "check mode must not call Homebrew");
  assert.equal(existsSync(scenario.cltInstallMarker), false, "check mode must not request a CLT installation");
  assert.deepEqual(snapshotDirectory(scenario.engineDirectory), scenario.engineSnapshot, "check mode must not modify the engine directory");
  assert.deepEqual(readdirSync(scenario.temporaryFiles), [], "temporary GStreamer registry files must be cleaned up");
}

test("install.sh --check accepts ready runtime tools without writing to HOME", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux" });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /Runtime dependencies are available\./);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check ignores a system PATH tool when the engine copy is missing", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux", missingCommands: ["scrcpy"] });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /- scrcpy/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check reports a missing scrcpy server binary", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux", missingScrcpyServer: true });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /- scrcpy server \(scrcpy-server\)/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check rejects an empty scrcpy server file", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux", emptyScrcpyServer: true });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /- scrcpy server \(scrcpy-server\)/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh repairs a missing scrcpy server in the engine directory and then skips redownloading", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux", missingScrcpyServer: true, packageManagerStatus: 0 });
  try {
    const firstRun = scenario.runInstaller([]);
    assert.equal(firstRun.status, 0, firstRun.stderr || firstRun.stdout);
    assert.equal(readFileSync(join(scenario.engineDirectory, "bin", "scrcpy-server"), "utf8"), "fake scrcpy server\n");
    assert.equal(readFileSync(scenario.scrcpyDownloadMarker, "utf8"), "download");

    const secondRun = scenario.runInstaller([]);
    assert.equal(secondRun.status, 0, secondRun.stderr || secondRun.stdout);
    assert.equal(readFileSync(scenario.scrcpyDownloadMarker, "utf8"), "download", "complete engine must not download scrcpy again");
  } finally {
    scenario.cleanup();
  }
});

test("staged runtime install.sh does not require the development .node-version file", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux" });
  const stagedSetup = join(scenario.root, "App Support", "setup");
  const stagedInstaller = join(stagedSetup, "install.sh");
  mkdirSync(stagedSetup, { recursive: true });
  copyFileSync(installerPath, stagedInstaller);
  try {
    const result = scenario.runInstaller(["--check"], stagedInstaller);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /Runtime dependencies are available\./);
    assert.doesNotMatch(result.stderr, /\.node-version/);
  } finally {
    scenario.cleanup();
  }
});

test("staged Windows runtime installer checks scrcpy-server without requiring .node-version", { skip: process.platform !== "win32" }, () => {
  const root = mkdtempSync(join(tmpdir(), "neo-airplay-powershell-runtime-check-"));
  const stagedSetup = join(root, "App Support", "setup");
  const stagedInstaller = join(stagedSetup, "install.ps1");
  const engineDirectory = join(root, "engine");
  mkdirSync(stagedSetup, { recursive: true });
  mkdirSync(engineDirectory);
  copyFileSync(powershellInstallerPath, stagedInstaller);
  try {
    const result = spawnSync("powershell.exe", ["-NoProfile", "-ExecutionPolicy", "Bypass", "-File", stagedInstaller, "-Check", "-EngineDirectory", engineDirectory], {
      cwd: root,
      encoding: "utf8",
      timeout: 30_000,
      maxBuffer: 2 * 1024 * 1024,
    });
    const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
    assert.equal(result.status, 1, output);
    assert.match(output, /scrcpy server \(scrcpy-server\)/);
    assert.doesNotMatch(output, /\.node-version/);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("install.sh --check accepts UxPlay 1.73.7 after a GStreamer version banner", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux", uxplayVersion: "1.73.7" });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /Runtime dependencies are available\./);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check rejects UxPlay below the required version despite its banner", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux", uxplayVersion: "1.73.6" });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /uxplay 1\.73\.7 or newer/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check rejects a failing scrcpy version probe", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux", scrcpyProbe: "fail" });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /scrcpy \(unusable or version probe timed out\)/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check rejects an empty adb version probe", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux", adbProbe: "empty" });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /adb \(unusable or version probe timed out\)/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check scans macOS GStreamer elements once and cleans its temporary registry", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Darwin" });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /Runtime dependencies are available\./);
    assert.deepEqual(readFileSync(scenario.gstInspectMarker, "utf8").trim().split("\n"), ["--print-plugin-auto-install-info"]);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check reports a missing GStreamer element", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux", missingGstElement: "avdec_aac" });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /- GStreamer element avdec_aac/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check requires the macOS software H.264 decoder", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Darwin", missingGstElement: "avdec_h264" });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /- GStreamer element avdec_h264/);
    assert.deepEqual(
      result.stdout.split(/\r?\n/).filter((line) => line.startsWith("  - ")),
      ["  - GStreamer element avdec_h264"],
      "only the unavailable element should be reported",
    );
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check reports a macOS GStreamer scan error with its diagnostic", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Darwin", gstScanMode: "error" });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /GStreamer plugin scan failed \(exit 42\)/);
    assert.match(result.stdout, /GStreamer scan diagnostic: fake GStreamer scan error/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check reports a timed out macOS GStreamer scan", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Darwin", gstScanMode: "timeout" });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /GStreamer plugin scan timed out after 30 seconds/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check requires the native macOS video sink", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Darwin", missingGstElement: "osxvideosink" });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /- GStreamer element osxvideosink/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check still requires autovideosink on Linux", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux", missingGstElement: "autovideosink" });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /- GStreamer element autovideosink/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check requires an active Avahi service on Linux", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux", avahiActive: false });
  try {
    const result = scenario.runInstaller(["--check"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /running Avahi service \(avahi-daemon\)/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check --development validates Rust stable, Bun 1.4.2, Node 22.23.3, and macOS CLT", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Darwin" });
  try {
    const result = scenario.runInstaller(["--check", "--development"]);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /Development tools are available/);
    assert.match(result.stdout, /Bun 1\.4\.2/);
    assert.match(result.stdout, /Node v22\.23\.3/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh does not reinstall runtime packages when only Linux development libraries are missing", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({
    uname: "Linux",
    missingPkgModule: "webkit2gtk-4.1",
    packageManagerStatus: 0,
  });
  try {
    const result = scenario.runInstaller(["--development"]);
    assert.equal(result.status, 1, result.stderr || result.stdout || "the mock package manager does not actually provide the missing module");
    const packageManagerCalls = readFileSync(scenario.packageManagerMarker, "utf8").trim().split("\n");
    assert.equal(packageManagerCalls.length, 1);
    assert.match(packageManagerCalls[0], /^install /, "only development libraries should be requested");
    assert.doesNotMatch(packageManagerCalls[0], /avahi-daemon|gstreamer1/, "runtime engine packages should not be reinstalled");
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check --development validates the Linux Tauri pkg-config modules", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux" });
  try {
    const result = scenario.runInstaller(["--check", "--development"]);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.match(result.stdout, /Development tools are available/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check --development reports a missing Linux Tauri pkg-config module", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux", missingPkgModule: "webkit2gtk-4.1" });
  try {
    const result = scenario.runInstaller(["--check", "--development"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /Tauri Linux pkg-config modules:.*webkit2gtk-4\.1/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check --development reports missing macOS Command Line Tools without requesting installation", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Darwin", cltAvailable: false });
  try {
    const result = scenario.runInstaller(["--check", "--development"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /Command Line Tools/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("install.sh --check --development rejects unsupported tool versions", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({
    uname: "Linux",
    bunVersion: "1.4.1",
    nodeVersion: "v22.7.0",
    missingCommands: ["rustup"],
  });
  try {
    const result = scenario.runInstaller(["--check", "--development"]);
    assert.equal(result.status, 1, result.stderr || result.stdout);
    assert.match(result.stdout, /Rust stable \(rustup, rustc, cargo\)/);
    assert.match(result.stdout, /Bun 1\.4\.2/);
    assert.match(result.stdout, /Node\.js 22/);
    assertReadOnly(scenario);
  } finally {
    scenario.cleanup();
  }
});

test("unknown installer options fail before creating user directories", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux" });
  try {
    const result = scenario.runInstaller(["--not-supported"]);
    assert.equal(result.status, 2);
    assert.match(result.stderr, /Unknown option: --not-supported/);
    assert.equal(existsSync(join(scenario.home, ".local")), false);
    assert.equal(existsSync(scenario.packageManagerMarker), false);
  } finally {
    scenario.cleanup();
  }
});

test("an already-ready normal Linux run avoids all package managers", { skip: shellChecksUnsupported }, () => {
  const scenario = createScenario({ uname: "Linux" });
  try {
    const result = scenario.runInstaller([]);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.equal(existsSync(scenario.packageManagerMarker), false, "apt-get must not run for a ready installation");
    assert.equal(existsSync(scenario.brewMarker), false, "brew must not run for a ready installation");
    assert.equal(existsSync(join(scenario.home, ".local")), false, "ready fast path should exit before user-bin creation");
  } finally {
    scenario.cleanup();
  }
});

test("an already-ready normal macOS run does not call Homebrew when no host Homebrew is available", {
  skip: shellChecksUnsupported || ["/opt/homebrew/bin/brew", "/usr/local/bin/brew"].some(existsSync),
}, () => {
  const scenario = createScenario({ uname: "Darwin" });
  try {
    const result = scenario.runInstaller([]);
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.equal(existsSync(scenario.brewMarker), false, "brew must not run for a ready installation");
    assert.equal(existsSync(join(scenario.home, ".local")), false, "ready fast path should exit before user-bin creation");
  } finally {
    scenario.cleanup();
  }
});

test("prepare-runtime skips its installer subprocess when setup is disabled", () => {
  const root = mkdtempSync(join(tmpdir(), "neo-airplay-prepare-runtime-test-"));
  const runtimeRoot = join(root, "desktop");
  const runtimeScripts = join(runtimeRoot, "scripts");
  const prepareRuntimeCopy = join(runtimeScripts, "prepare-runtime.mjs");
  const marker = join(root, "installer-was-called");
  mkdirSync(runtimeScripts, { recursive: true });
  copyFileSync(runtimeEntryPath, prepareRuntimeCopy);

  if (process.platform === "win32") {
    writeFileSync(join(runtimeScripts, "install.ps1"), "Set-Content -LiteralPath $env:SETUP_TEST_MARKER -Value invoked\n");
  } else {
    const fakeInstaller = join(runtimeScripts, "install.sh");
    writeFileSync(fakeInstaller, '#!/bin/sh\nprintf invoked > "$SETUP_TEST_MARKER"\n');
    chmodSync(fakeInstaller, 0o755);
  }

  try {
    const result = spawnSync(process.execPath, [prepareRuntimeCopy], {
      cwd: root,
      env: { ...process.env, NEO_AIRPLAY_SKIP_SETUP: "1", SETUP_TEST_MARKER: marker },
      encoding: "utf8",
      timeout: 10_000,
      maxBuffer: 1024 * 1024,
    });
    assert.equal(result.status, 0, result.stderr || result.stdout);
    assert.equal(existsSync(marker), false, "disabled setup must not spawn its installer entry");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("Tauri runner prepares development tools before invoking the CLI", () => {
  const calls = [];
  const result = runTauri(["dev", "--verbose"], {
    platform: "darwin",
    environment: { HOME: "/test/home", PATH: "/usr/bin" },
    scriptsDirectory: "/project/desktop/scripts",
    desktopDirectory: "/project/desktop",
    spawn: (command, args, options) => {
      calls.push({ command, args, options });
      return { status: 0 };
    },
  });

  assert.equal(result, 0);
  assert.deepEqual(calls.map(({ command }) => command), ["/bin/bash", "node"]);
  assert.deepEqual(calls[0].args, ["/project/desktop/scripts/setup.sh"]);
  assert.deepEqual(calls[1].args, ["/project/desktop/node_modules/@tauri-apps/cli/tauri.js", "dev", "--verbose"]);
  assert.equal(calls[1].options.env.NEO_AIRPLAY_SETUP_COMPLETE, "1");
  assert.match(calls[1].options.env.PATH, /\/test\/home\/\.cargo\/bin/);
});

test("Tauri runner uses the PowerShell setup entry on Windows", () => {
  const [command, args] = getSetupCommand("win32", "C:\\project\\desktop\\scripts");
  assert.equal(command, "powershell.exe");
  assert.deepEqual(args, [
    "-NoProfile",
    "-ExecutionPolicy",
    "Bypass",
    "-File",
    "C:\\project\\desktop\\scripts\\setup.ps1",
  ]);
  const environment = withDevelopmentPath({
    LOCALAPPDATA: "C:\\Users\\Tester\\AppData\\Local",
    PATH: "C:\\Windows\\System32",
    USERPROFILE: "C:\\Users\\Tester",
  }, "win32", "C:\\project\\desktop");
  assert.match(environment.PATH, /NeoAirPlay\\node-v22\.23\.3/);
});

test("Tauri runner stops when setup fails", () => {
  const calls = [];
  const result = runTauri(["build"], {
    platform: "linux",
    environment: { HOME: "/test/home", PATH: "/usr/bin" },
    scriptsDirectory: "/project/desktop/scripts",
    desktopDirectory: "/project/desktop",
    spawn: (command, args) => {
      calls.push({ command, args });
      return { status: 12 };
    },
  });

  assert.equal(result, 12);
  assert.equal(calls.length, 1, "Tauri must not run after setup fails");
});

test("Tauri runner respects CI setup skip and an already-completed bootstrap", () => {
  for (const environment of [
    { HOME: "/test/home", PATH: "/usr/bin", NEO_AIRPLAY_SKIP_SETUP: "1" },
    { HOME: "/test/home", PATH: "/usr/bin", NEO_AIRPLAY_SETUP_COMPLETE: "1" },
  ]) {
    const calls = [];
    const result = runTauri(["build"], {
      platform: "linux",
      environment,
      scriptsDirectory: "/project/desktop/scripts",
      desktopDirectory: "/project/desktop",
      spawn: (command, args) => {
        calls.push({ command, args });
        return { status: 0 };
      },
    });

    assert.equal(result, 0);
    assert.equal(calls.length, 1);
    assert.equal(calls[0].command, "node");
  }
});
