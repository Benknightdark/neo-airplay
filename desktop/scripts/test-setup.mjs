import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  copyFileSync,
  existsSync,
  mkdtempSync,
  mkdirSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { delimiter, join } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import test from "node:test";

const installerPath = fileURLToPath(new URL("./install.sh", import.meta.url));
const runtimeEntryPath = fileURLToPath(new URL("./prepare-runtime.mjs", import.meta.url));
const shellChecksUnsupported = process.platform === "win32";

function findHostCommand(commandName) {
  for (const directory of (process.env.PATH ?? "").split(delimiter)) {
    const candidate = join(directory, commandName);
    if (existsSync(candidate)) return candidate;
  }
  throw new Error(`Host utility not found: ${commandName}`);
}

function createScenario(options = {}) {
  const root = mkdtempSync(join(tmpdir(), "neo-airplay-setup-check-"));
  const home = join(root, "home");
  const fakeBin = join(root, "fake-bin");
  const utilityBin = join(root, "utility-bin");
  const engineDirectory = join(root, "engine");
  const engineBin = join(engineDirectory, "bin");
  const temporaryFiles = join(root, "tmp");
  const packageManagerMarker = join(root, "package-manager-was-called");
  const brewMarker = join(root, "brew-was-called");
  const cltInstallMarker = join(root, "clt-install-was-requested");
  mkdirSync(home);
  mkdirSync(fakeBin);
  mkdirSync(utilityBin);
  mkdirSync(temporaryFiles);
  mkdirSync(engineBin, { recursive: true });

  for (const utility of ["awk", "grep", "head", "mkdir", "mv", "rm", "sed"]) {
    symlinkSync(findHostCommand(utility), join(utilityBin, utility));
  }

  const fakeCommands = {
    uname: '#!/bin/sh\ncase "$1" in -m) printf "%s\\n" "${FAKE_ARCH:-x86_64}" ;; *) printf "%s\\n" "${FAKE_UNAME:-Linux}" ;; esac\n',
    uxplay: '#!/bin/sh\nprintf "*=== GStreamer >= 1.22 on macOS ===*\\nUxPlay version %s; for help, use option \\"-h\\"\\n" "${FAKE_UXPLAY_VERSION:-1.73.7}"\n',
    "gst-inspect-1.0": '#!/bin/sh\nif [ "${1:-}" = "${MISSING_GST_ELEMENT:-}" ]; then exit 1; fi\nexit 0\n',
    timeout: '#!/bin/sh\nshift\nshift\nexec "$@"\n',
    gtimeout: '#!/bin/sh\nshift\nshift\nexec "$@"\n',
    adb: '#!/bin/sh\ncase "${ADB_PROBE:-ok}" in fail) exit 1 ;; empty) exit 0 ;; *) printf "Android Debug Bridge version 1.0.41\\n" ;; esac\n',
    scrcpy: '#!/bin/sh\ncase "${SCRCPY_PROBE:-ok}" in fail) exit 1 ;; empty) exit 0 ;; *) printf "scrcpy 5.0.1\\n" ;; esac\n',
    "avahi-daemon": "#!/bin/sh\nexit 0\n",
    pgrep: '#!/bin/sh\n[ "${AVAHI_ACTIVE:-1}" = "1" ]\n',
    "pkg-config": '#!/bin/sh\nif [ "${1:-}" = "--exists" ] && [ "${2:-}" = "${MISSING_PKG_MODULE:-}" ]; then exit 1; fi\nexit 0\n',
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
    "apt-get": `#!/bin/sh\nprintf '%s\\n' "$*" >> "$PACKAGE_MANAGER_MARKER"\nexit 97\n`,
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

  const environment = {
    ...process.env,
    HOME: home,
    PATH: [fakeBin, utilityBin].join(delimiter),
    NEO_AIRPLAY_ENGINE_DIR: engineDirectory,
    TMPDIR: temporaryFiles,
    FAKE_UNAME: options.uname ?? "Linux",
    FAKE_ARCH: "x86_64",
    FAKE_UXPLAY_VERSION: options.uxplayVersion ?? "1.73.7",
    FAKE_BUN_VERSION: options.bunVersion ?? "1.4.2",
    FAKE_NODE_VERSION: options.nodeVersion ?? "v22.23.3",
    SCRCPY_PROBE: options.scrcpyProbe ?? "ok",
    ADB_PROBE: options.adbProbe ?? "ok",
    MISSING_GST_ELEMENT: options.missingGstElement ?? "",
    MISSING_PKG_MODULE: options.missingPkgModule ?? "",
    AVAHI_ACTIVE: options.avahiActive === false ? "0" : "1",
    CLT_AVAILABLE: options.cltAvailable === false ? "0" : "1",
    PACKAGE_MANAGER_MARKER: packageManagerMarker,
    BREW_MARKER: brewMarker,
    CLT_INSTALL_MARKER: cltInstallMarker,
  };

  return {
    root,
    home,
    packageManagerMarker,
    brewMarker,
    cltInstallMarker,
    engineDirectory,
    runInstaller(args) {
      return spawnSync("/bin/bash", [installerPath, "--engine-dir", engineDirectory, ...args], {
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
