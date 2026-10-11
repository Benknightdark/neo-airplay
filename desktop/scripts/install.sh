#!/usr/bin/env bash
set -Eeuo pipefail

readonly UXPLAY_VERSION="1.73.7"
readonly UXPLAY_COMMIT="df67c212a433cf6dda3676dd40c097900d24e645"
readonly UXPLAY_SHA256="168de53ed688f87c4022d79357ce91f9608ad5c53236bff95134d19b88efeb3f"
readonly UXPLAY_URL="https://github.com/FDH2/UxPlay/archive/${UXPLAY_COMMIT}.tar.gz"
readonly BUN_VERSION="1.4.2"
readonly NODE_VERSION="$(< "$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/.node-version")"
readonly SCRCPY_VERSION="5.0.1"
readonly SCRCPY_SHA256="9f969d30cc574816077edecda65719c1c70b0aee1df1ba5fa00779ac07b8fd6e"
readonly SCRCPY_LINUX_URL="https://github.com/Genymobile/scrcpy/releases/download/v${SCRCPY_VERSION}/scrcpy-linux-x86_64-v${SCRCPY_VERSION}.tar.gz"
readonly SCRCPY_MACOS_ARM64_SHA256="33611e51977a8289e2e124b220f727895181ef2eb9060e0987a4dc2db9cf0d6b"
readonly SCRCPY_MACOS_X86_64_SHA256="31a5467a9e907f162b093267cbbab1276b15c6e709c2ea8159566a8d165beb50"

development=0
check_only=0
gui=0
engine_dir="${NEO_AIRPLAY_ENGINE_DIR:-}"
marker_file=""
result_file=""

while (($#)); do
  argument="$1"
  case "$argument" in
    --development) development=1 ;;
    --check) check_only=1 ;;
    --gui) gui=1 ;;
    --engine-dir)
      (($# >= 2)) || { printf 'Missing value for --engine-dir\n' >&2; exit 2; }
      engine_dir="$2"; shift ;;
    --marker-file)
      (($# >= 2)) || { printf 'Missing value for --marker-file\n' >&2; exit 2; }
      marker_file="$2"; shift ;;
    --result-file)
      (($# >= 2)) || { printf 'Missing value for --result-file\n' >&2; exit 2; }
      result_file="$2"; shift ;;
    *) printf 'Unknown option: %s\n' "$argument" >&2; exit 2 ;;
  esac
  shift
done

if [[ -z "$engine_dir" ]]; then
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  engine_dir="${script_dir}/../engine"
fi
engine_dir="$(cd "$engine_dir" 2>/dev/null && pwd || printf '%s' "$engine_dir")"
engine_bin="${engine_dir}/bin"

log() { printf '%s\n' "$*"; }
fail() { printf 'Setup failed: %s\n' "$*" >&2; exit 1; }

user_bin="${HOME}/.local/bin"
export NEO_AIRPLAY_ENGINE_DIR="$engine_dir"
export PATH="${engine_bin}:${engine_dir}/android/platform-tools:${engine_dir}/android:${engine_dir}:${user_bin}:${HOME}/.cargo/bin:${HOME}/.bun/bin:${PATH:-}"

write_result() {
  local status="$1" temporary
  [[ -n "$result_file" ]] || return 0
  temporary="${result_file}.tmp.$$"
  if printf '{"pid":%d,"exitCode":%d}\n' "$$" "$status" > "$temporary"; then
    mv -f -- "$temporary" "$result_file"
  else
    rm -f -- "$temporary"
  fi
}

on_exit() {
  local status=$?
  trap - EXIT
  if [[ -n "$marker_file" ]]; then rm -f -- "$marker_file"; fi
  write_result "$status"
  exit "$status"
}

missing=()
bounded_timeout=""
check_command() {
  local command_name="$1"
  if ! command -v "$command_name" >/dev/null 2>&1; then
    missing+=("$command_name")
    return 1
  fi
  return 0
}

runtime_executable() {
  local command_name="$1"
  local candidate="${engine_bin}/${command_name}"
  [[ -x "$candidate" ]] && printf '%s\n' "$candidate"
}

setup_macos_library_path() {
  local prefix library_path candidate
  library_path="${engine_dir}/lib"
  for prefix in /opt/homebrew /usr/local; do
    [[ -d "${prefix}/lib" ]] && library_path="${library_path}:${prefix}/lib"
    for candidate in "${prefix}"/opt/*/lib; do
      [[ -d "$candidate" ]] && library_path="${library_path}:${candidate}"
    done
  done
  export DYLD_LIBRARY_PATH="${library_path}${DYLD_LIBRARY_PATH:+:${DYLD_LIBRARY_PATH}}"
}

check_gstreamer() {
  local element
  local elements=(decodebin h264parse videoconvert autoaudiosink avdec_aac)
  local gst_inspect
  gst_inspect="$(runtime_executable gst-inspect-1.0 || true)"
  if [[ -z "$gst_inspect" ]]; then
    missing+=("GStreamer (gst-inspect-1.0)")
    return 1
  fi
  [[ -n "$bounded_timeout" ]] || return 1
  export GST_PLUGIN_PATH_1_0="${engine_dir}/lib/gstreamer-1.0"
  export GST_PLUGIN_SYSTEM_PATH_1_0="${engine_dir}/lib/gstreamer-1.0"
  if [[ "$(uname -s)" == "Darwin" ]]; then
    setup_macos_library_path
  fi
  if [[ -x "${engine_bin}/gst-plugin-scanner" ]]; then
    export GST_PLUGIN_SCANNER_1_0="${engine_bin}/gst-plugin-scanner"
  fi
  if [[ "$(uname -s)" == "Darwin" ]]; then
    elements+=(osxvideosink avdec_h264)
  else
    elements+=(autovideosink)
  fi
  for element in "${elements[@]}"; do
    if ! "$bounded_timeout" --kill-after=2s 10s "$gst_inspect" "$element" >/dev/null 2>&1; then
      missing+=("GStreamer element ${element}")
    fi
  done
}

check_version_probe() {
  local command_name="$1" argument="$2" label="$3" probe_output executable
  executable="$(runtime_executable "$command_name" || true)"
  if [[ -z "$executable" ]]; then
    missing+=("$command_name")
    return 1
  fi
  if [[ -z "$bounded_timeout" ]]; then
    missing+=("GNU timeout utility for bounded runtime probes")
    return 1
  fi
  if ! probe_output="$("$bounded_timeout" --kill-after=2s 10s "$executable" "$argument" 2>&1)" || [[ -z "$probe_output" ]]; then
    missing+=("$label (unusable or version probe timed out)")
    return 1
  fi
}

check_uxplay() {
  local executable version_output version_number
  executable="$(runtime_executable uxplay || true)"
  if [[ -z "$executable" ]]; then
    missing+=("uxplay ${UXPLAY_VERSION}")
    return 1
  fi
  if [[ -z "$bounded_timeout" ]]; then
    missing+=("GNU timeout utility for bounded runtime probes")
    return 1
  fi
  if ! version_output="$("$bounded_timeout" --kill-after=2s 10s "$executable" -v 2>&1)"; then
    missing+=("uxplay version probe failed or timed out")
    return 1
  fi
  version_number="$(printf '%s\n' "$version_output" | sed -nE 's/.*UxPlay version ([0-9]+(\.[0-9]+)+).*/\1/p' | head -n 1 || true)"
  if [[ -z "$version_number" ]] || ! awk -v actual="$version_number" -v minimum="$UXPLAY_VERSION" 'BEGIN { split(actual, a, "."); split(minimum, b, "."); for (i=1; i<=3; i++) { if ((a[i]+0) > (b[i]+0)) exit 0; if ((a[i]+0) < (b[i]+0)) exit 1 } exit 0 }'; then
    missing+=("uxplay ${UXPLAY_VERSION} or newer")
    return 1
  fi
}

check_runtime() {
  missing=()
  if [[ "$(uname -s)" == "Darwin" ]]; then
    bounded_timeout="$(runtime_executable gtimeout || true)"
  else
    bounded_timeout="$(runtime_executable timeout || true)"
  fi
  [[ -n "$bounded_timeout" ]] || missing+=("GNU timeout utility for bounded runtime probes")
  check_uxplay || true
  check_version_probe scrcpy --version scrcpy || true
  check_version_probe adb version adb || true
  check_gstreamer || true
  if [[ "$(uname -s)" == "Linux" ]]; then
    check_command avahi-daemon || true
    if command -v pgrep >/dev/null 2>&1 && ! pgrep -x avahi-daemon >/dev/null 2>&1; then
      missing+=("running Avahi service (avahi-daemon)")
    fi
  fi
  if ((${#missing[@]})); then
    log "Missing runtime dependencies:"
    printf '  - %s\n' "${missing[@]}"
    return 1
  fi
  log "Runtime dependencies are available."
}

check_development() {
  local node_version bun_version rust_version
  local dev_missing=()
  if ! command -v rustup >/dev/null 2>&1 || ! rustc +stable --version >/dev/null 2>&1 || ! cargo +stable --version >/dev/null 2>&1; then
    dev_missing+=("Rust stable (rustup, rustc, cargo)")
  else
    local rust_components
    rust_components="$(rustup component list --toolchain stable --installed 2>/dev/null || true)"
    printf '%s\n' "$rust_components" | grep -Eq '^rustfmt(-|$)' || dev_missing+=("Rust stable component rustfmt")
    printf '%s\n' "$rust_components" | grep -Eq '^clippy(-|$)' || dev_missing+=("Rust stable component clippy")
  fi
  bun_version="$(bun --version 2>/dev/null || true)"
  [[ "$bun_version" == "$BUN_VERSION" ]] || dev_missing+=("Bun ${BUN_VERSION}")
  node_version="$(node --version 2>/dev/null || true)"
  [[ "$node_version" == "v${NODE_VERSION}" ]] || dev_missing+=("Node.js ${NODE_VERSION}")
  case "$(uname -s)" in
    Darwin)
      xcode-select -p >/dev/null 2>&1 || dev_missing+=("Xcode Command Line Tools")
      ;;
    Linux)
      local build_command
      for build_command in cmake cc c++ patchelf xz; do
        command -v "$build_command" >/dev/null 2>&1 || dev_missing+=("Linux build command: ${build_command}")
      done
      if ! command -v pkg-config >/dev/null 2>&1; then
        dev_missing+=("Tauri Linux build libraries (pkg-config, GTK 3, WebKitGTK 4.1, Ayatana AppIndicator, xdo, OpenSSL, librsvg)")
      else
        local module
        local missing_modules=()
        for module in gtk+-3.0 webkit2gtk-4.1 xdo openssl librsvg-2.0; do
          pkg-config --exists "$module" || missing_modules+=("$module")
        done
        if ! pkg-config --exists ayatana-appindicator3-0.1 && ! pkg-config --exists appindicator3-0.0 && ! pkg-config --exists appindicator3-0.1; then
          missing_modules+=("Ayatana/libappindicator3")
        fi
        if ((${#missing_modules[@]})); then
          dev_missing+=("Tauri Linux pkg-config modules: ${missing_modules[*]}")
        fi
      fi
      ;;
  esac
  if ((${#dev_missing[@]})); then
    log "Missing development tools:"
    printf '  - %s\n' "${dev_missing[@]}"
    return 1
  fi
  rust_version="$(rustc +stable --version)"
  log "Development tools are available (${rust_version}, Bun ${bun_version}, Node ${node_version})."
}

persist_development_path() {
  local config_dir env_file temporary line profile fish_file
  config_dir="${HOME}/.config/neo-airplay"
  env_file="${config_dir}/env.sh"
  temporary="${env_file}.tmp.$$"
  mkdir -p "$config_dir"
  cat > "$temporary" <<'EOF'
case ":${PATH:-}:" in *":${HOME}/.local/bin:"*) ;; *) export PATH="${HOME}/.local/bin${PATH:+:${PATH}}" ;; esac
case ":${PATH:-}:" in *":${HOME}/.cargo/bin:"*) ;; *) export PATH="${HOME}/.cargo/bin${PATH:+:${PATH}}" ;; esac
case ":${PATH:-}:" in *":${HOME}/.bun/bin:"*) ;; *) export PATH="${HOME}/.bun/bin${PATH:+:${PATH}}" ;; esac
EOF
  chmod 0644 "$temporary"
  mv -f -- "$temporary" "$env_file"
  line='if [ -r "$HOME/.config/neo-airplay/env.sh" ]; then . "$HOME/.config/neo-airplay/env.sh"; fi'
  for profile in "${HOME}/.profile" "${HOME}/.bash_profile" "${HOME}/.bashrc" "${HOME}/.zprofile" "${HOME}/.zshrc"; do
    if [[ ! -f "$profile" ]] || ! grep -Fqx -- "$line" "$profile"; then
      printf '\n%s\n' "$line" >> "$profile"
    fi
  done
  fish_file="${HOME}/.config/fish/conf.d/neo-airplay.fish"
  mkdir -p "$(dirname "$fish_file")"
  if [[ ! -f "$fish_file" ]]; then
    cat > "$fish_file" <<'EOF'
fish_add_path --path $HOME/.local/bin $HOME/.cargo/bin $HOME/.bun/bin
EOF
  fi
  export PATH="${HOME}/.local/bin:${HOME}/.cargo/bin:${HOME}/.bun/bin:${PATH:-}"
}

if ((check_only)); then
  if check_runtime; then runtime_status=0; else runtime_status=1; fi
  if ((development)); then
    check_development || runtime_status=1
  fi
  exit "$runtime_status"
fi

trap on_exit EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
if [[ -n "$marker_file" ]]; then
  mkdir -p "$(dirname "$marker_file")"
  printf '%d\n' "$$" > "$marker_file"
fi
if [[ -n "$result_file" ]]; then mkdir -p "$(dirname "$result_file")"; fi

runtime_ready=0
development_ready=0
if check_runtime; then runtime_ready=1; fi
if ((development)) && check_development; then development_ready=1; fi
if ((runtime_ready && (!development || development_ready))); then
  if ((development)); then persist_development_path; fi
  exit 0
fi

mkdir -p "$engine_bin"

run_privileged() {
  if ((gui)); then
    command -v pkexec >/dev/null 2>&1 || fail "pkexec is required for GUI installation. Run this script in a terminal instead."
    pkexec "$@"
  elif ((EUID == 0)); then
    "$@"
  elif command -v sudo >/dev/null 2>&1; then
    sudo "$@"
  else
    fail "Administrator authorization is required. Run this script from a terminal with sudo available."
  fi
}

install_uxplay_source() {
  local archive work_dir source_dir actual_sha version_output version_number
  if [[ -x "${engine_bin}/uxplay" ]]; then
    version_output="$("${engine_bin}/uxplay" -v 2>&1 || true)"
    version_number="$(printf '%s\n' "$version_output" | sed -nE 's/.*UxPlay version ([0-9]+(\.[0-9]+)+).*/\1/p' | head -n 1 || true)"
    if [[ -n "$version_number" ]] && awk -v actual="$version_number" -v minimum="$UXPLAY_VERSION" 'BEGIN { split(actual, a, "."); split(minimum, b, "."); for (i=1; i<=3; i++) { if ((a[i]+0) > (b[i]+0)) exit 0; if ((a[i]+0) < (b[i]+0)) exit 1 } exit 0 }'; then
      return
    fi
  fi
  work_dir="$(mktemp -d "${TMPDIR:-/tmp}/neo-airplay-uxplay.XXXXXX")"
  archive="${work_dir}/uxplay.tar.gz"
  curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location "$UXPLAY_URL" --output "$archive"
  if command -v sha256sum >/dev/null 2>&1; then
    actual_sha="$(sha256sum "$archive" | awk '{print $1}')"
  else
    actual_sha="$(shasum -a 256 "$archive" | awk '{print $1}')"
  fi
  [[ "$actual_sha" == "$UXPLAY_SHA256" ]] || fail "UxPlay source archive checksum did not match the pinned release."
  mkdir -p "${work_dir}/source"
  tar -xzf "$archive" -C "${work_dir}/source" --strip-components=1
  cmake -S "${work_dir}/source" -B "${work_dir}/build" -DCMAKE_INSTALL_PREFIX="$engine_dir"
  cmake --build "${work_dir}/build" --parallel
  cmake --install "${work_dir}/build"
  rm -rf "$work_dir"
  export PATH="${engine_bin}:${PATH}"
}

stage_runtime_tool() {
  local command_name="$1" source_path="${2:-}" target_path
  target_path="${engine_bin}/${command_name}"
  [[ -x "$target_path" ]] && return 0
  if [[ -z "$source_path" ]]; then source_path="$(command -v "$command_name" 2>/dev/null || true)"; fi
  [[ -n "$source_path" && -x "$source_path" ]] || fail "Required runtime tool is not available for staging: ${command_name}."
  mkdir -p "$engine_bin"
  cp -L "$source_path" "$target_path"
  chmod 0755 "$target_path"
}

stage_runtime_tools() {
  local scrcpy_server scrcpy_prefix
  stage_runtime_tool adb
  stage_runtime_tool scrcpy
  stage_runtime_tool gst-inspect-1.0
  if [[ "$(uname -s)" == "Darwin" ]]; then
    stage_runtime_tool gtimeout "$(command -v gtimeout 2>/dev/null || true)"
  else
    stage_runtime_tool timeout "$(command -v timeout 2>/dev/null || true)"
  fi
  stage_gstreamer_plugins
  if [[ ! -f "${engine_bin}/scrcpy-server" ]]; then
    scrcpy_server="$(dirname "$(command -v scrcpy)")/scrcpy-server"
    if [[ -f "$scrcpy_server" ]]; then cp -L "$scrcpy_server" "${engine_bin}/scrcpy-server"; fi
    for scrcpy_server in /usr/share/scrcpy/scrcpy-server /usr/lib/scrcpy/scrcpy-server /usr/lib64/scrcpy/scrcpy-server; do
      if [[ -f "$scrcpy_server" ]]; then cp -L "$scrcpy_server" "${engine_bin}/scrcpy-server"; break; fi
    done
    if [[ ! -f "${engine_bin}/scrcpy-server" ]] && command -v brew >/dev/null 2>&1; then
      scrcpy_prefix="$(brew --prefix scrcpy 2>/dev/null || true)"
      if [[ -n "$scrcpy_prefix" ]]; then
        scrcpy_server="${scrcpy_prefix}/share/scrcpy/scrcpy-server"
        if [[ -n "$scrcpy_server" ]]; then cp -L "$scrcpy_server" "${engine_bin}/scrcpy-server"; fi
      fi
    fi
  fi
  if [[ ! -f "${engine_bin}/scrcpy-server" ]]; then
    fail "The scrcpy server binary could not be staged in ${engine_bin}."
  fi
}

copy_tree_dereference_links() {
  local source_root="$1" source_path relative_path target_path
  while IFS= read -r -d '' source_path; do
    relative_path="${source_path#"${source_root}/"}"
    target_path="${engine_dir}/lib/${relative_path}"
    if [[ -d "$source_path" ]]; then
      rm -rf "$target_path"
      mkdir -p "$(dirname "$target_path")"
      cp -RL "$source_path" "$target_path"
    elif [[ -f "$source_path" ]]; then
      rm -f "$target_path"
      cp -L "$source_path" "$target_path"
    else
      rm -f "$target_path"
    fi
  done < <(find "$source_root" -type l -print0)
}

stage_gstreamer_plugins() {
  local plugins_source scanner_source plugins_target
  plugins_target="${engine_dir}/lib/gstreamer-1.0"
  if [[ "$(uname -s)" == "Darwin" ]]; then
    local gstreamer_prefix
    gstreamer_prefix="$(brew --prefix gstreamer)"
    plugins_source="${gstreamer_prefix}/lib/gstreamer-1.0"
    scanner_source="${gstreamer_prefix}/libexec/gstreamer-1.0/gst-plugin-scanner"
    mkdir -p "${engine_dir}/lib"
    cp -Rf "${gstreamer_prefix}/lib/." "${engine_dir}/lib/"
    copy_tree_dereference_links "${gstreamer_prefix}/lib"
    find "${engine_dir}/lib" -type f -exec chmod u+w {} +
  else
    plugins_source="$(pkg-config --variable=pluginsdir gstreamer-1.0 2>/dev/null || true)"
    if [[ -z "$plugins_source" || ! -d "$plugins_source" ]]; then
      plugins_source="$(find /usr/lib /usr/lib64 -type d -path '*/gstreamer-1.0' -print -quit 2>/dev/null || true)"
    fi
    scanner_source="$(pkg-config --variable=pluginscannerdir gstreamer-1.0 2>/dev/null || true)"
    scanner_source="${scanner_source%/}/gst-plugin-scanner"
    if [[ ! -x "$scanner_source" ]]; then
      scanner_source="$(find /usr/lib /usr/lib64 -type f -name gst-plugin-scanner -print -quit 2>/dev/null || true)"
    fi
  fi
  [[ -d "$plugins_source" ]] || fail "GStreamer plugin directory is unavailable for staging."
  if [[ "$(uname -s)" != "Darwin" ]]; then
    mkdir -p "$plugins_target"
    cp -a "${plugins_source}/." "$plugins_target/"
  fi
  [[ -x "$scanner_source" ]] || fail "GStreamer plugin scanner is unavailable for staging."
  cp -L "$scanner_source" "${engine_bin}/gst-plugin-scanner"
  chmod 0755 "${engine_bin}/gst-plugin-scanner"
}

ensure_homebrew() {
  export PATH="/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/local/sbin:${PATH:-}"
  if command -v brew >/dev/null 2>&1; then
    eval "$(brew shellenv)"
    export PATH="$(brew --prefix)/bin:$(brew --prefix)/sbin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/local/sbin:${PATH:-}"
    return
  fi
  [[ -t 0 && -t 1 ]] || fail "Homebrew is missing. Run this installer from Terminal so the official Homebrew installer can request any required authorization."
  command -v curl >/dev/null 2>&1 || fail "curl is required to install Homebrew from its official source."
  log "Installing Homebrew from the official installer."
  /bin/bash -c "$(/usr/bin/curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"
  export PATH="/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/local/sbin:${PATH:-}"
  if ! command -v brew >/dev/null 2>&1; then
    [[ -x /opt/homebrew/bin/brew ]] && export PATH="/opt/homebrew/bin:/opt/homebrew/sbin:${PATH}"
    [[ -x /usr/local/bin/brew ]] && export PATH="/usr/local/bin:/usr/local/sbin:${PATH}"
  fi
  command -v brew >/dev/null 2>&1 || fail "Homebrew installation finished but brew is still not on PATH."
  eval "$(brew shellenv)"
  export PATH="$(brew --prefix)/bin:$(brew --prefix)/sbin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/local/sbin:${PATH:-}"
}

ensure_xcode_cli_tools() {
  if xcode-select -p >/dev/null 2>&1; then return; fi
  log "Requesting Xcode Command Line Tools installation. Complete the Apple installer dialog to continue."
  xcode-select --install >/dev/null 2>&1 || true
  local attempt
  for ((attempt=0; attempt<1800; attempt++)); do
    xcode-select -p >/dev/null 2>&1 && return
    sleep 2
  done
  fail "Xcode Command Line Tools installation did not finish within 60 minutes."
}

install_macos_runtime() {
  ensure_xcode_cli_tools
  ensure_homebrew
  export HOMEBREW_PREFIX="$(brew --prefix)"
  log "Installing GStreamer and UxPlay build dependencies with Homebrew."
  brew install --quiet cmake coreutils libplist openssl@3 pkg-config gstreamer
  install_uxplay_source
  install_scrcpy_macos
  stage_runtime_tools
}

install_scrcpy_macos() {
  local architecture asset expected_sha work_dir archive extract_dir bundle binary server adb actual_sha
  architecture="$(uname -m)"
  case "$architecture" in
    arm64|aarch64)
      asset="scrcpy-macos-aarch64-v${SCRCPY_VERSION}.tar.gz"
      expected_sha="$SCRCPY_MACOS_ARM64_SHA256" ;;
    x86_64|amd64)
      asset="scrcpy-macos-x86_64-v${SCRCPY_VERSION}.tar.gz"
      expected_sha="$SCRCPY_MACOS_X86_64_SHA256" ;;
    *) fail "The official scrcpy release does not support macOS ${architecture}." ;;
  esac
  if [[ -x "${engine_bin}/scrcpy" && -x "${engine_bin}/adb" && -f "${engine_bin}/scrcpy-server" ]]; then return; fi
  work_dir="$(mktemp -d "${TMPDIR:-/tmp}/neo-airplay-scrcpy.XXXXXX")"
  archive="${work_dir}/${asset}"
  extract_dir="${work_dir}/extract"
  mkdir -p "$extract_dir"
  curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location "https://github.com/Genymobile/scrcpy/releases/download/v${SCRCPY_VERSION}/${asset}" --output "$archive"
  if command -v shasum >/dev/null 2>&1; then
    actual_sha="$(shasum -a 256 "$archive" | awk '{print $1}')"
  else
    actual_sha="$(sha256sum "$archive" | awk '{print $1}')"
  fi
  [[ "$actual_sha" == "$expected_sha" ]] || fail "The official macOS scrcpy archive SHA-256 verification failed."
  tar -xzf "$archive" -C "$extract_dir"
  bundle="$(find "$extract_dir" -type d -name "scrcpy-macos-*-v${SCRCPY_VERSION}" -print -quit)"
  [[ -n "$bundle" ]] || fail "The official macOS scrcpy archive did not contain its distribution directory."
  binary="${bundle}/scrcpy"
  server="${bundle}/scrcpy-server"
  adb="${bundle}/adb"
  [[ -x "$binary" && -f "$server" && -x "$adb" ]] || fail "The official macOS scrcpy archive is missing required executables."
  mkdir -p "$engine_bin"
  cp -L "$binary" "${engine_bin}/scrcpy"
  cp -L "$server" "${engine_bin}/scrcpy-server"
  cp -L "$adb" "${engine_bin}/adb"
  chmod 0755 "${engine_bin}/scrcpy" "${engine_bin}/scrcpy-server" "${engine_bin}/adb"
  rm -rf "$work_dir"
}

install_scrcpy_linux() {
  local architecture work_dir archive actual_sha extract_dir binary server target_dir
  if [[ -x "${engine_bin}/scrcpy" ]] && [[ -n "$bounded_timeout" ]] && "$bounded_timeout" --kill-after=2s 10s "${engine_bin}/scrcpy" --version >/dev/null 2>&1; then return; fi
  architecture="$(uname -m)"
  if [[ "$architecture" == x86_64 || "$architecture" == amd64 ]]; then
    work_dir="$(mktemp -d "${TMPDIR:-/tmp}/neo-airplay-scrcpy.XXXXXX")"
    archive="${work_dir}/scrcpy.tar.gz"
    extract_dir="${work_dir}/extract"
    mkdir -p "$extract_dir"
    curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location "$SCRCPY_LINUX_URL" --output "$archive"
    if command -v sha256sum >/dev/null 2>&1; then
      actual_sha="$(sha256sum "$archive" | awk '{print $1}')"
    else
      actual_sha="$(shasum -a 256 "$archive" | awk '{print $1}')"
    fi
    [[ "$actual_sha" == "$SCRCPY_SHA256" ]] || fail "scrcpy source archive checksum did not match the pinned official release."
    tar -xzf "$archive" -C "$extract_dir"
    binary="$(find "$extract_dir" -type f -name scrcpy -perm -u+x -print -quit)"
    server="$(find "$extract_dir" -type f -name scrcpy-server -print -quit)"
    [[ -n "$binary" && -n "$server" ]] || fail "The official scrcpy release archive did not contain its client and server binaries."
    target_dir="$engine_bin"
    mkdir -p "$target_dir"
    cp -L "$binary" "${target_dir}/scrcpy"
    cp -L "$server" "${target_dir}/scrcpy-server"
    local adb
    adb="$(find "$extract_dir" -type f -name adb -perm -u+x -print -quit)"
    [[ -n "$adb" ]] || fail "The official scrcpy release archive did not contain its ADB executable."
    cp -L "$adb" "${target_dir}/adb"
    chmod 0755 "${target_dir}/scrcpy" "${target_dir}/scrcpy-server" "${target_dir}/adb"
    rm -rf "$work_dir"
    return
  fi
  command -v scrcpy >/dev/null 2>&1 || fail "The official static scrcpy release currently supports Linux x86_64 only, and no scrcpy package is available for ${architecture}."
}

install_bun_binary() {
  local system architecture asset expected_sha work_dir archive extracted executable target_dir
  if [[ "$(bun --version 2>/dev/null || true)" == "$BUN_VERSION" ]]; then return; fi
  system="$(uname -s)"
  architecture="$(uname -m)"
  case "$system:$architecture" in
    Linux:x86_64|Linux:amd64)
      asset="bun-linux-x64.zip"; expected_sha="36368faef7527875d5ffa52e53cd48021741f2a83eb6208a8dd64068d422a913" ;;
    Linux:aarch64|Linux:arm64)
      asset="bun-linux-aarch64.zip"; expected_sha="54328bbc2d9c8e0c9f892c544d66c57a83b84139e34909e5ee81758f1ac8fda7" ;;
    Darwin:x86_64|Darwin:amd64)
      asset="bun-darwin-x64.zip"; expected_sha="80520d7e17526308c9185d261679ac6d27798d3803a0e9f7ff9121ab8affb012" ;;
    Darwin:arm64|Darwin:aarch64)
      asset="bun-darwin-aarch64.zip"; expected_sha="90987a3a16d7db556d886ac3d551e7b6d3edf0a1cf43acaed622e8676be1d12f" ;;
    *) fail "Official Bun ${BUN_VERSION} binaries are unavailable for ${system}/${architecture}." ;;
  esac
  work_dir="$(mktemp -d "${TMPDIR:-/tmp}/neo-airplay-bun.XXXXXX")"
  archive="${work_dir}/${asset}"
  extracted="${work_dir}/extract"
  mkdir -p "$extracted"
  curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location "https://github.com/oven-sh/bun/releases/download/bun-v${BUN_VERSION}/${asset}" --output "$archive"
  if command -v sha256sum >/dev/null 2>&1; then
    actual_sha="$(sha256sum "$archive" | awk '{print $1}')"
  else
    actual_sha="$(shasum -a 256 "$archive" | awk '{print $1}')"
  fi
  [[ "$actual_sha" == "$expected_sha" ]] || fail "Bun ${BUN_VERSION} archive checksum verification failed."
  unzip -q "$archive" -d "$extracted"
  executable="$(find "$extracted" -type f -name bun -print -quit)"
  [[ -n "$executable" ]] || fail "Official Bun archive is missing its executable."
  target_dir="${HOME}/.bun/bin"
  mkdir -p "$target_dir"
  install -m 0755 "$executable" "${target_dir}/bun"
  rm -rf "$work_dir"
  export PATH="${target_dir}:${PATH}"
}

install_node_binary() {
  local system architecture asset expected_sha work_dir archive extracted node_root node_dir actual_sha
  if [[ "$(node --version 2>/dev/null || true)" == "v${NODE_VERSION}" ]]; then return; fi
  system="$(uname -s)"
  architecture="$(uname -m)"
  case "$system:$architecture" in
    Linux:x86_64|Linux:amd64)
      asset="node-v${NODE_VERSION}-linux-x64.tar.xz"; expected_sha="df450af89261115ef9f9e3830c3eeb2cc9213b63c720b1af623cb5dcbe2e02de" ;;
    Linux:aarch64|Linux:arm64)
      asset="node-v${NODE_VERSION}-linux-arm64.tar.xz"; expected_sha="a44aeb94849a299b22df10b9e622ec2f605c2183501bc40590705131de7c740f" ;;
    Darwin:x86_64|Darwin:amd64)
      asset="node-v${NODE_VERSION}-darwin-x64.tar.xz"; expected_sha="ac41874c3352937119cfec39e1a98c578fe58ce8a86d7e785b89a4706015f305" ;;
    Darwin:arm64|Darwin:aarch64)
      asset="node-v${NODE_VERSION}-darwin-arm64.tar.xz"; expected_sha="72d5d8832b41c9d9646197af614ffd751406ea4d215060eb91b98864e1919a3e" ;;
    *) fail "Official Node.js ${NODE_VERSION} binaries are unavailable for ${system}/${architecture}." ;;
  esac
  work_dir="$(mktemp -d "${TMPDIR:-/tmp}/neo-airplay-node.XXXXXX")"
  archive="${work_dir}/${asset}"
  extracted="${work_dir}/extract"
  mkdir -p "$extracted"
  curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location "https://nodejs.org/dist/v${NODE_VERSION}/${asset}" --output "$archive"
  if command -v sha256sum >/dev/null 2>&1; then
    actual_sha="$(sha256sum "$archive" | awk '{print $1}')"
  else
    actual_sha="$(shasum -a 256 "$archive" | awk '{print $1}')"
  fi
  [[ "$actual_sha" == "$expected_sha" ]] || fail "Node.js ${NODE_VERSION} archive checksum verification failed."
  tar -xJf "$archive" -C "$extracted"
  node_root="$(find "$extracted" -mindepth 1 -maxdepth 1 -type d -name "node-v${NODE_VERSION}-*" -print -quit)"
  [[ -n "$node_root" && -x "${node_root}/bin/node" ]] || fail "Official Node.js archive did not contain its node binary."
  node_dir="${HOME}/.local/lib/neo-airplay/node-v${NODE_VERSION}"
  mkdir -p "$node_dir" "$user_bin"
  cp -a "${node_root}/." "$node_dir/"
  ln -sf "${node_dir}/bin/node" "${user_bin}/node"
  ln -sf "${node_dir}/bin/npm" "${user_bin}/npm"
  ln -sf "${node_dir}/bin/npx" "${user_bin}/npx"
  rm -rf "$work_dir"
  export PATH="${user_bin}:${PATH}"
}

install_development_package_manager_tools() {
  local manager="$1"
  case "$manager" in
    apt)
      run_privileged apt-get install -y -qq --no-install-recommends build-essential cmake pkg-config patchelf xz-utils file unzip libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libxdo-dev librsvg2-dev ;;
    dnf)
      run_privileged dnf -q -y install gcc gcc-c++ cmake pkgconf-pkg-config patchelf xz file unzip gtk3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel libxdo-devel librsvg2-devel ;;
    pacman)
      run_privileged pacman -Syu --noconfirm --noprogressbar --needed base-devel cmake pkgconf patchelf xz file unzip gtk3 webkit2gtk-4.1 libappindicator-gtk3 xdotool librsvg ;;
  esac
}

linux_packages() {
  local manager="$1" architecture
  local runtime_packages=() build_packages=() development_packages=()
  architecture="$(uname -m)"
  case "$manager" in
    apt)
      runtime_packages=(avahi-daemon ca-certificates dbus procps gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-libav gstreamer1.0-plugins-good gstreamer1.0-plugins-bad)
      build_packages=(build-essential cmake pkg-config curl unzip libssl-dev libplist-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libavahi-compat-libdnssd-dev)
      development_packages=(libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libxdo-dev librsvg2-dev patchelf file xz-utils)
      ;;
    dnf)
      runtime_packages=(avahi ca-certificates dbus-daemon procps-ng gstreamer1-tools gstreamer1-plugins-base gstreamer1-plugin-libav gstreamer1-plugins-good gstreamer1-plugins-bad-free)
      build_packages=(gcc gcc-c++ cmake pkgconf-pkg-config curl unzip openssl-devel libplist-devel gstreamer1-devel gstreamer1-plugins-base-devel avahi-compat-libdns_sd-devel)
      development_packages=(gtk3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel libxdo-devel librsvg2-devel patchelf file xz)
      ;;
    pacman)
      runtime_packages=(avahi ca-certificates dbus procps gstreamer gst-plugins-base gst-libav gst-plugins-good gst-plugins-bad)
      build_packages=(base-devel cmake pkgconf curl unzip openssl libplist gstreamer gst-plugins-base)
      development_packages=(gtk3 webkit2gtk-4.1 libappindicator-gtk3 xdotool librsvg patchelf file xz)
      ;;
  esac
  if [[ "$architecture" != x86_64 && "$architecture" != amd64 ]]; then
    case "$manager" in
      apt) runtime_packages+=(adb scrcpy) ;;
      dnf|pacman) runtime_packages+=(android-tools scrcpy) ;;
    esac
  fi
  if ((development)); then
    runtime_packages+=("${development_packages[@]}")
  fi
  case "$manager" in
    apt)
      run_privileged apt-get update -qq
      run_privileged apt-get install -y -qq --no-install-recommends "${runtime_packages[@]}" "${build_packages[@]}"
      ;;
    dnf)
      if [[ "$architecture" != x86_64 && "$architecture" != amd64 ]] && ! dnf repoquery --quiet scrcpy >/dev/null 2>&1; then
        run_privileged dnf -q -y install dnf-plugins-core
        run_privileged dnf -q -y copr enable zeno/scrcpy
      fi
      run_privileged dnf -q -y install "${runtime_packages[@]}" "${build_packages[@]}"
      ;;
    pacman)
      run_privileged pacman -Syu --noconfirm --noprogressbar --needed "${runtime_packages[@]}" "${build_packages[@]}"
      ;;
  esac
}

install_linux_runtime() {
  local manager=""
  if command -v apt-get >/dev/null 2>&1; then manager=apt
  elif command -v dnf >/dev/null 2>&1; then manager=dnf
  elif command -v pacman >/dev/null 2>&1; then manager=pacman
  else fail "Supported Linux package manager not found (apt, dnf, pacman)."; fi
  linux_packages "$manager"
  install_scrcpy_linux
  install_uxplay_source
  if command -v systemctl >/dev/null 2>&1; then
    if ! systemctl is-active --quiet avahi-daemon; then
      run_privileged systemctl enable --now avahi-daemon
    fi
  elif command -v service >/dev/null 2>&1 && ! pgrep -x avahi-daemon >/dev/null 2>&1; then
    run_privileged service avahi-daemon start
  fi
  stage_runtime_tools
}

install_development_tools() {
  mkdir -p "$user_bin"
  case "$(uname -s)" in
    Darwin)
      ensure_xcode_cli_tools
      ensure_homebrew
      eval "$(brew shellenv)"
      brew install --quiet rustup-init
      if ! command -v rustup >/dev/null 2>&1; then
        rustup-init -y --no-modify-path --profile minimal --default-toolchain stable
      fi
      rustup toolchain install stable --profile minimal
      rustup component add rustfmt clippy --toolchain stable
      rustup default stable
      export PATH="${HOME}/.cargo/bin:${PATH}"
      install_bun_binary
      install_node_binary
      persist_development_path
      if ! command -v pwsh >/dev/null 2>&1; then
        brew install --quiet --cask powershell || log "PowerShell was not available from Homebrew; the PowerShell Harness entry will not be available."
      fi
      ;;
    Linux)
      install_linux_dev_tools
      ;;
    *) fail "Development tool installation is not supported on this platform." ;;
  esac
}

install_linux_dev_tools() {
  local temp_dir rust_script rustup_command
  temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/neo-airplay-devtools.XXXXXX")"
  if ! command -v rustup >/dev/null 2>&1; then
    rust_script="${temp_dir}/rustup-init.sh"
    curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location https://sh.rustup.rs --output "$rust_script"
    sh "$rust_script" -y --no-modify-path --profile minimal --default-toolchain stable
  fi
  rustup_command="$(command -v rustup 2>/dev/null || true)"
  [[ -n "$rustup_command" ]] || fail "Rustup was installed but could not be found on PATH."
  "$rustup_command" toolchain install stable --profile minimal
  "$rustup_command" component add rustfmt clippy --toolchain stable
  "$rustup_command" default stable

  install_bun_binary
  install_node_binary

  if ! command -v pwsh >/dev/null 2>&1; then
    local manager=""
    if command -v apt-get >/dev/null 2>&1 && apt-cache show powershell >/dev/null 2>&1; then manager=apt
    elif command -v dnf >/dev/null 2>&1 && dnf repoquery --quiet powershell >/dev/null 2>&1; then manager=dnf
    elif command -v pacman >/dev/null 2>&1 && pacman -Si powershell >/dev/null 2>&1; then manager=pacman
    fi
    case "$manager" in
      apt) run_privileged apt-get install -y -qq powershell ;;
      dnf) run_privileged dnf -q -y install powershell ;;
      pacman) run_privileged pacman -S --noconfirm --noprogressbar --needed powershell ;;
      *) log "PowerShell is not available from the configured Linux repositories; skipping optional pwsh install." ;;
    esac
  fi
  rm -rf "$temp_dir"
}

if ((!runtime_ready)); then
  case "$(uname -s)" in
    Darwin)
      install_macos_runtime
      ;;
    Linux)
      install_linux_runtime
      ;;
    *) fail "This installer supports macOS and Linux. Use install.ps1 on Windows." ;;
  esac
fi

if ((development)); then
  if ((!development_ready)); then
    if [[ "$(uname -s)" == "Linux" && "$runtime_ready" == "1" ]]; then
      if command -v apt-get >/dev/null 2>&1; then manager=apt
      elif command -v dnf >/dev/null 2>&1; then manager=dnf
      elif command -v pacman >/dev/null 2>&1; then manager=pacman
      else fail "Supported Linux package manager not found (apt, dnf, pacman)."; fi
      install_development_package_manager_tools "$manager"
    fi
    install_development_tools
  fi
fi

check_runtime
if ((development)); then
  persist_development_path
  export PATH="${HOME}/.cargo/bin:${HOME}/.bun/bin:${PATH}"
  if [[ "$(uname -s)" == "Darwin" ]]; then
    export PATH="${user_bin}:${PATH}"
  fi
  check_development
fi
