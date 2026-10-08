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

const installerPath = fileURLToPath(new URL("../src-tauri/resources/setup/install.sh", import.meta.url));
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
  const temporaryFiles = join(root, "tmp");
  const packageManagerMarker = join(root, "package-manager-was-called");
  const brewMarker = join(root, "brew-was-called");
  const cltInstallMarker = join(root, "clt-install-was-requested");
  mkdirSync(home);
  mkdirSync(fakeBin);
  mkdirSync(utilityBin);
  mkdirSync(temporaryFiles);

  for (const utility of ["awk", "grep", "head", "mkdir", "mv", "rm"]) {
    symlinkSync(findHostCommand(utility), join(utilityBin, utility));
  }

  const fakeCommands = {
    uname: '#!/bin/sh\ncase "$1" in -m) printf "%s\\n" "${FAKE_ARCH:-x86_64}" ;; *) printf "%s\\n" "${FAKE_UNAME:-Linux}" ;; esac\n',
    uxplay: '#!/bin/sh\nprintf "UxPlay %s\\n" "${FAKE_UXPLAY_VERSION:-1.73.7}"\n',
    "gst-inspect-1.0": '#!/bin/sh\nif [ "${1:-}" = "${MISSING_GST_ELEMENT:-}" ]; then exit 1; fi\nexit 0\n',
    timeout: '#!/bin/sh\nshift\nshift\nexec "$@"\n',
    gtimeout: '#!/bin/sh\nshift\nshift\nexec "$@"\n',
    adb: '#!/bin/sh\ncase "${ADB_PROBE:-ok}" in fail) exit 1 ;; empty) exit 0 ;; *) printf "Android Debug Bridge version 1.0.41\\n" ;; esac\n',
    scrcpy: '#!/bin/sh\ncase "${SCRCPY_PROBE:-ok}" in fail) exit 1 ;; empty) exit 0 ;; *) printf "scrcpy 5.0.1\\n" ;; esac\n',
    "avahi-daemon": "#!/bin/sh\nexit 0\n",
    pgrep: '#!/bin/sh\n[ "${AVAHI_ACTIVE:-1}" = "1" ]\n',
    "pkg-config": '#!/bin/sh\nif [ "${1:-}" = "--exists" ] && [ "${2:-}" = "${MISSING_PKG_MODULE:-}" ]; then exit 1; fi\nexit 0\n',
    rustup: "#!/bin/sh\nexit 0\n",
    rustc: '#!/bin/sh\nprintf "rustc 1.88.0 (isolated setup test)\\n"\n',
    cargo: '#!/bin/sh\nprintf "cargo 1.88.0 (isolated setup test)\\n"\n',
    bun: '#!/bin/sh\nprintf "%s\\n" "${FAKE_BUN_VERSION:-1.4.2}"\n',
    node: '#!/bin/sh\nprintf "%s\\n" "${FAKE_NODE_VERSION:-v22.23.3}"\n',
    sudo: '#!/bin/sh\nexec "$@"\n',
    "apt-get": `#!/bin/sh\nprintf '%s\\n' "$*" >> "$PACKAGE_MANAGER_MARKER"\nexit 97\n`,
    brew: `#!/bin/sh\nprintf '%s\\n' "$*" >> "$BREW_MARKER"\nexit 97\n`,
    "xcode-select": '#!/bin/sh\nif [ "${1:-}" = "--install" ]; then printf requested > "$CLT_INSTALL_MARKER"; exit 0; fi\n[ "${CLT_AVAILABLE:-1}" = "1" ]\n',
  };

  const missingCommands = new Set(options.missingCommands ?? []);
  for (const [commandName, contents] of Object.entries(fakeCommands)) {
    if (missingCommands.has(commandName)) continue;
    const commandPath = join(fakeBin, commandName);
    writeFileSync(commandPath, contents);
    chmodSync(commandPath, 0o755);
  }

  const environment = {
    ...process.env,
    HOME: home,
    PATH: [fakeBin, utilityBin].join(delimiter),
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
    runInstaller(args) {
      return spawnSync("/bin/bash", [installerPath, ...args], {
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

test("install.sh --check reports a missing command using the isolated PATH", { skip: shellChecksUnsupported }, () => {
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
  const setupDirectory = join(runtimeRoot, "src-tauri", "resources", "setup");
  const prepareRuntimeCopy = join(runtimeScripts, "prepare-runtime.mjs");
  const marker = join(root, "installer-was-called");
  mkdirSync(runtimeScripts, { recursive: true });
  mkdirSync(setupDirectory, { recursive: true });
  copyFileSync(runtimeEntryPath, prepareRuntimeCopy);

  if (process.platform === "win32") {
    writeFileSync(join(setupDirectory, "install.ps1"), "Set-Content -LiteralPath $env:SETUP_TEST_MARKER -Value invoked\n");
  } else {
    const fakeInstaller = join(setupDirectory, "install.sh");
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
