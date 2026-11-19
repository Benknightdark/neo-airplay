#!/usr/bin/env bash
set -Eeuo pipefail
umask 077

readonly DEFAULT_UXPLAY_VERSION="1.73.7"
readonly DEFAULT_UXPLAY_COMMIT="df67c212a433cf6dda3676dd40c097900d24e645"
readonly DEFAULT_UXPLAY_SHA256="168de53ed688f87c4022d79357ce91f9608ad5c53236bff95134d19b88efeb3f"
readonly BUN_VERSION="1.4.2"
readonly NODE_VERSION_FILE="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/.node-version"
NODE_VERSION=""
readonly DEFAULT_SCRCPY_VERSION="5.0.1"
readonly DEFAULT_SCRCPY_LINUX_SHA256="9f969d30cc574816077edecda65719c1c70b0aee1df1ba5fa00779ac07b8fd6e"
readonly DEFAULT_SCRCPY_MACOS_ARM64_SHA256="33611e51977a8289e2e124b220f727895181ef2eb9060e0987a4dc2db9cf0d6b"
readonly DEFAULT_SCRCPY_MACOS_X86_64_SHA256="31a5467a9e907f162b093267cbbab1276b15c6e709c2ea8159566a8d165beb50"
readonly HOMEBREW_INSTALL_COMMIT="eaa2af62ca1d5349c9743f82b1b0c4e5d365fb2d"
readonly HOMEBREW_INSTALL_SHA256="8fd2f1a5287a7c2e3d7362924c677edcc04c5a0e16448c77132d056bc88cf1ed"
readonly HOMEBREW_INSTALL_URL="https://raw.githubusercontent.com/Homebrew/install/${HOMEBREW_INSTALL_COMMIT}/install.sh"
readonly RUSTUP_VERSION="1.29.3"
readonly RUSTUP_LINUX_X64_SHA256="b1c8cb14e30889e7e2999e6b69e62d9e9b9ffc0f4df8180564d306d9680e6c55"
readonly RUSTUP_LINUX_ARM64_SHA256="9f608c142e0e38a78957d0661ba1dfaf0b6fb37184cfaa4b9a0aa57af6184282"

UXPLAY_VERSION="$DEFAULT_UXPLAY_VERSION"
UXPLAY_COMMIT="$DEFAULT_UXPLAY_COMMIT"
UXPLAY_SHA256="$DEFAULT_UXPLAY_SHA256"
SCRCPY_VERSION="$DEFAULT_SCRCPY_VERSION"
SCRCPY_SHA256_OVERRIDE=""
only_tool=""
expect_uxplay_version=""
allow_unverified_uxplay=0
uxplay_ref_arg=""
uxplay_sha_arg=""
scrcpy_custom_requested=0
uxplay_custom_requested=0

development=0
check_only=0
warm_registry=0
gui=0
upgrade=0
engine_dir="${NEO_AIRPLAY_ENGINE_DIR:-}"
registry_path=""
marker_file=""
result_file=""
cleanup_paths=()
run_install_lifecycle=0

while (($#)); do
  argument="$1"
  case "$argument" in
    --development) development=1 ;;
    --check) check_only=1 ;;
    --upgrade) upgrade=1 ;;
    --warm-gstreamer-registry) warm_registry=1 ;;
    --gui) gui=1 ;;
    --allow-unverified-uxplay) allow_unverified_uxplay=1 ;;
    --only)
      (($# >= 2)) || { printf 'Missing value for --only\n' >&2; exit 2; }
      only_tool="$2"; shift ;;
    --uxplay-ref)
      (($# >= 2)) || { printf 'Missing value for --uxplay-ref\n' >&2; exit 2; }
      uxplay_ref_arg="$2"; shift ;;
    --uxplay-sha256)
      (($# >= 2)) || { printf 'Missing value for --uxplay-sha256\n' >&2; exit 2; }
      uxplay_sha_arg="$2"; shift ;;
    --scrcpy-version)
      (($# >= 2)) || { printf 'Missing value for --scrcpy-version\n' >&2; exit 2; }
      SCRCPY_VERSION="$2"; scrcpy_custom_requested=1; shift ;;
    --scrcpy-sha256)
      (($# >= 2)) || { printf 'Missing value for --scrcpy-sha256\n' >&2; exit 2; }
      SCRCPY_SHA256_OVERRIDE="$2"; scrcpy_custom_requested=1; shift ;;
    --expect-uxplay-version)
      (($# >= 2)) || { printf 'Missing value for --expect-uxplay-version\n' >&2; exit 2; }
      expect_uxplay_version="$2"; shift ;;
    --engine-dir)
      (($# >= 2)) || { printf 'Missing value for --engine-dir\n' >&2; exit 2; }
      engine_dir="$2"; shift ;;
    --registry-path)
      (($# >= 2)) || { printf 'Missing value for --registry-path\n' >&2; exit 2; }
      registry_path="$2"; shift ;;
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

if [[ -n "$only_tool" && "$only_tool" != "uxplay" && "$only_tool" != "scrcpy" ]]; then
  printf 'Invalid --only tool: %s\n' "$only_tool" >&2
  exit 2
fi

SCRCPY_VERSION="${SCRCPY_VERSION#v}"
SCRCPY_VERSION="${SCRCPY_VERSION#V}"
if [[ ! "$SCRCPY_VERSION" =~ ^[0-9]+\.[0-9]+(\.[0-9]+)?$ ]]; then
  printf 'Invalid --scrcpy-version format: %s\n' "$SCRCPY_VERSION" >&2
  exit 2
fi

if [[ -n "$SCRCPY_SHA256_OVERRIDE" && ! "$SCRCPY_SHA256_OVERRIDE" =~ ^[0-9a-fA-F]{64}$ ]]; then
  printf 'Invalid --scrcpy-sha256 checksum.\n' >&2
  exit 2
fi

if [[ -n "$uxplay_sha_arg" ]]; then
  if [[ ! "$uxplay_sha_arg" =~ ^[0-9a-fA-F]{64}$ ]]; then
    printf 'Invalid --uxplay-sha256 checksum.\n' >&2
    exit 2
  fi
  UXPLAY_SHA256="$uxplay_sha_arg"
fi

if [[ -n "$uxplay_ref_arg" ]]; then
  if [[ "$uxplay_ref_arg" == "$DEFAULT_UXPLAY_COMMIT" || "$uxplay_ref_arg" == "$DEFAULT_UXPLAY_VERSION" || "$uxplay_ref_arg" == "v${DEFAULT_UXPLAY_VERSION}" ]]; then
    UXPLAY_COMMIT="$DEFAULT_UXPLAY_COMMIT"
    UXPLAY_VERSION="$DEFAULT_UXPLAY_VERSION"
    [[ -n "$uxplay_sha_arg" ]] || UXPLAY_SHA256="$DEFAULT_UXPLAY_SHA256"
  elif [[ "$uxplay_ref_arg" =~ ^[0-9a-fA-F]{40}$ ]]; then
    uxplay_custom_requested=1
    UXPLAY_COMMIT="$(printf '%s' "$uxplay_ref_arg" | awk '{print tolower($1)}')"
    [[ -n "$uxplay_sha_arg" ]] || UXPLAY_SHA256=""
  elif [[ "$uxplay_ref_arg" =~ ^[vV]?([0-9]+\.[0-9]+(\.[0-9]+)*)$ ]]; then
    uxplay_custom_requested=1
    UXPLAY_VERSION="${BASH_REMATCH[1]}"
    UXPLAY_COMMIT="v${UXPLAY_VERSION}"
    [[ -n "$uxplay_sha_arg" ]] || UXPLAY_SHA256=""
  else
    printf 'Invalid --uxplay-ref format: %s\n' "$uxplay_ref_arg" >&2
    exit 2
  fi
fi

if ((uxplay_custom_requested)) && [[ -z "$UXPLAY_SHA256" ]] && ((allow_unverified_uxplay == 0)); then
  printf 'Custom --uxplay-ref requires --uxplay-sha256 or --allow-unverified-uxplay.\n' >&2
  exit 2
fi

if [[ -n "$expect_uxplay_version" ]]; then
  if [[ "$expect_uxplay_version" =~ ^[0-9a-fA-F]{40}$ ]]; then
    expect_uxplay_version="$(printf '%s' "$expect_uxplay_version" | awk '{print tolower($1)}')"
  elif [[ "$expect_uxplay_version" =~ ^[vV]?([0-9]+\.[0-9]+(\.[0-9]+)*)$ ]]; then
    expect_uxplay_version="${BASH_REMATCH[1]}"
  else
    printf 'Invalid --expect-uxplay-version format: %s\n' "$expect_uxplay_version" >&2
    exit 2
  fi
fi

if [[ "$UXPLAY_COMMIT" =~ ^[0-9a-fA-F]{40}$ ]]; then
  readonly UXPLAY_URL="https://github.com/FDH2/UxPlay/archive/${UXPLAY_COMMIT}.tar.gz"
else
  readonly UXPLAY_URL="https://github.com/FDH2/UxPlay/archive/refs/tags/${UXPLAY_COMMIT}.tar.gz"
fi
readonly SCRCPY_LINUX_URL="https://github.com/Genymobile/scrcpy/releases/download/v${SCRCPY_VERSION}/scrcpy-linux-x86_64-v${SCRCPY_VERSION}.tar.gz"

log() { printf '%s\n' "$*"; }
fail() { printf 'Setup failed: %s\n' "$*" >&2; exit 1; }

if [[ -z "$engine_dir" ]]; then
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  engine_dir="${script_dir}/../engine"
fi
raw_engine_dir="${engine_dir%/}"
[[ -n "$raw_engine_dir" ]] || raw_engine_dir="/"
[[ ! -L "$raw_engine_dir" ]] || fail "Engine directory must not be a symbolic link: ${raw_engine_dir}"
if [[ -e "$raw_engine_dir" && ! -d "$raw_engine_dir" ]]; then
  fail "Engine path must be a directory: ${raw_engine_dir}"
fi
engine_dir="$(cd "$raw_engine_dir" 2>/dev/null && pwd || printf '%s' "$raw_engine_dir")"
[[ ! -L "$engine_dir" ]] || fail "Engine directory must not be a symbolic link: ${engine_dir}"
engine_bin="${engine_dir}/bin"
[[ ! -L "$engine_bin" ]] || fail "Engine bin directory must not be a symbolic link: ${engine_bin}"
if [[ -n "$registry_path" ]]; then
  [[ ! -L "${registry_path%/}" ]] || fail "Registry path must not be a symbolic link: ${registry_path}"
fi

load_node_version() {
  [[ -r "$NODE_VERSION_FILE" ]] || fail "Node.js version file is unavailable: ${NODE_VERSION_FILE}"
  NODE_VERSION="$(< "$NODE_VERSION_FILE")"
  [[ -n "$NODE_VERSION" ]] || fail "Node.js version file is empty: ${NODE_VERSION_FILE}"
}

user_bin="${HOME}/.local/bin"
export NEO_AIRPLAY_ENGINE_DIR="$engine_dir"
export PATH="${user_bin}:${HOME}/.cargo/bin:${HOME}/.bun/bin:${PATH:-}"

cleanup_registered_paths() {
  local path
  if ((${#cleanup_paths[@]} > 0)); then
    for path in "${cleanup_paths[@]}"; do
      [[ -n "$path" ]] && rm -rf -- "$path"
    done
  fi
}

write_result() {
  local status="$1" temporary
  [[ -n "$result_file" ]] || return 0
  [[ ! -L "$result_file" ]] || return 1
  temporary="$(mktemp "${result_file}.tmp.XXXXXX")"
  if printf '{"pid":%d,"exitCode":%d}\n' "$$" "$status" > "$temporary"; then
    mv -f -- "$temporary" "$result_file"
  else
    rm -f -- "$temporary"
  fi
}

on_exit() {
  local status=$?
  trap - EXIT
  cleanup_registered_paths
  if ((run_install_lifecycle)); then
    if [[ -n "$marker_file" && ! -L "$marker_file" ]]; then rm -f -- "$marker_file"; fi
    write_result "$status"
  fi
  exit "$status"
}
trap on_exit EXIT

sha256_file() {
  local file_path="$1"
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file_path" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file_path" | awk '{print $1}'
  else
    fail "Neither sha256sum nor shasum is available for checksum verification."
  fi
}

download_https_unverified() {
  local url="$1" destination="$2" label="$3"
  [[ "$url" == https://* ]] || fail "Refusing non-HTTPS download URL for ${label}."
  if ! curl --proto '=https' --proto-redir '=https' --tlsv1.2 --fail --silent --show-error --location --max-redirs 5 --connect-timeout 30 --max-time 600 "$url" --output "$destination"; then
    rm -f -- "$destination"
    fail "Failed to download ${label}."
  fi
}

download_verified() {
  local url="$1" destination="$2" expected_sha="$3" label="$4" actual_sha
  [[ "$expected_sha" =~ ^[0-9a-fA-F]{64}$ ]] || fail "Invalid SHA-256 checksum configured for ${label}."
  expected_sha="$(printf '%s\n' "$expected_sha" | awk '{print tolower($1)}')"
  download_https_unverified "$url" "$destination" "$label"
  actual_sha="$(sha256_file "$destination" | awk '{print tolower($1)}')"
  if [[ "$actual_sha" != "$expected_sha" ]]; then
    rm -f -- "$destination"
    fail "${label} SHA-256 checksum verification failed."
  fi
}

resolve_scrcpy_sha256() {
  local version="$1" asset="$2" builtin_sha="$3" sums_dir sums_file resolved
  if [[ -n "$SCRCPY_SHA256_OVERRIDE" ]]; then
    printf '%s\n' "$SCRCPY_SHA256_OVERRIDE"
    return 0
  fi
  if [[ "$version" == "$DEFAULT_SCRCPY_VERSION" && -n "$builtin_sha" ]]; then
    printf '%s\n' "$builtin_sha"
    return 0
  fi
  sums_dir="$(mktemp -d "${TMPDIR:-/tmp}/neo-airplay-scrcpy-sums.XXXXXX")"
  cleanup_paths+=("$sums_dir")
  sums_file="${sums_dir}/SHA256SUMS.txt"
  download_https_unverified "https://github.com/Genymobile/scrcpy/releases/download/v${version}/SHA256SUMS.txt" "$sums_file" "Official scrcpy v${version} SHA256SUMS.txt"
  resolved="$(awk -v target="$asset" '{ name=$2; sub(/^\*/, "", name); sub(/^\.\//, "", name); if (name == target && $1 ~ /^[0-9a-fA-F]{64}$/) { print tolower($1); exit 0 } }' "$sums_file")"
  rm -rf -- "$sums_dir"
  [[ -n "$resolved" ]] || fail "Official SHA256SUMS.txt for scrcpy v${version} did not contain checksum for ${asset}."
  printf '%s\n' "$resolved"
}

stage_file_atomic() {
  local source_path="$1" target_path="$2" mode="$3" allow_source_symlink="${4:-0}" target_dir temp_file
  if ((allow_source_symlink)); then
    [[ -f "$source_path" ]] || fail "Required source file is unavailable: ${source_path}"
  else
    [[ -f "$source_path" && ! -L "$source_path" ]] || fail "Refusing unsafe or symbolic-link source file: ${source_path}"
  fi
  target_dir="$(dirname "$target_path")"
  mkdir -p "$target_dir"
  [[ ! -L "$target_dir" ]] || fail "Target directory must not be a symbolic link: ${target_dir}"
  [[ ! -L "$target_path" ]] || fail "Refusing to overwrite symbolic link: ${target_path}"
  temp_file="$(mktemp "${target_dir}/.stage.tmp.XXXXXX")"
  cleanup_paths+=("$temp_file")
  if ((allow_source_symlink)); then
    cp -L -- "$source_path" "$temp_file"
  else
    cp -p -- "$source_path" "$temp_file"
  fi
  [[ -f "$temp_file" && ! -L "$temp_file" ]] || fail "Staged file is not a regular file: ${target_path}"
  chmod "$mode" "$temp_file"
  mv -f -- "$temp_file" "$target_path"
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
  [[ -f "$candidate" && ! -L "$candidate" && -x "$candidate" ]] && printf '%s\n' "$candidate"
}

setup_macos_library_path() {
  local prefix library_path
  library_path="${engine_dir}/lib"
  for prefix in /opt/homebrew /usr/local; do
    [[ -d "${prefix}/lib" ]] && library_path="${library_path}:${prefix}/lib"
  done
  export DYLD_LIBRARY_PATH="${library_path}${DYLD_LIBRARY_PATH:+:${DYLD_LIBRARY_PATH}}"
}

check_gstreamer() {
  local element platform
  local elements=(decodebin h264parse videoconvert autoaudiosink avdec_aac)
  local gst_inspect
  platform="$(uname -s)"
  gst_inspect="$(runtime_executable gst-inspect-1.0 || true)"
  if [[ -z "$gst_inspect" ]]; then
    missing+=("GStreamer (gst-inspect-1.0)")
    return 1
  fi
  [[ -n "$bounded_timeout" ]] || return 1
  export GST_PLUGIN_PATH_1_0="${engine_dir}/lib/gstreamer-1.0"
  export GST_PLUGIN_SYSTEM_PATH_1_0="${engine_dir}/lib/gstreamer-1.0"
  if [[ "$platform" == "Darwin" ]]; then
    setup_macos_library_path
  fi
  if [[ -x "${engine_bin}/gst-plugin-scanner" ]]; then
    export GST_PLUGIN_SCANNER_1_0="${engine_bin}/gst-plugin-scanner"
  fi
  if [[ "$platform" == "Darwin" ]]; then
    elements+=(osxvideosink avdec_h264)

    local registry_directory plugin_directory plugin_file plugin_source registry_output registry_error registry_status diagnostic
    # Limit the fresh registry scan to the plugins that provide the required elements.
    local plugin_files=(
      libgstplayback.dylib
      libgstvideoparsersbad.dylib
      libgstvideoconvertscale.dylib
      libgstautodetect.dylib
      libgstlibav.dylib
      libgstosxvideo.dylib
    )
    local previous_registry="${GST_REGISTRY-}" previous_registry_1_0="${GST_REGISTRY_1_0-}"
    local had_registry="${GST_REGISTRY+x}" had_registry_1_0="${GST_REGISTRY_1_0+x}"
    local previous_plugin_path="${GST_PLUGIN_PATH-}" previous_plugin_system_path="${GST_PLUGIN_SYSTEM_PATH-}"
    local had_plugin_path="${GST_PLUGIN_PATH+x}" had_plugin_system_path="${GST_PLUGIN_SYSTEM_PATH+x}"
    if ! registry_directory="$(mktemp -d "${TMPDIR:-/tmp}/neo-airplay-gstreamer-check.XXXXXX")"; then
      missing+=("GStreamer plugin scan (could not create a temporary registry)")
      return 1
    fi
    cleanup_paths+=("$registry_directory")
    plugin_directory="${registry_directory}/plugins"
    if ! mkdir -p "$plugin_directory"; then
      missing+=("GStreamer plugin scan (could not create an isolated plugin path)")
      rm -rf "$registry_directory"
      return 1
    fi
    for plugin_file in "${plugin_files[@]}"; do
      plugin_source="${engine_dir}/lib/gstreamer-1.0/${plugin_file}"
      if [[ ! -f "$plugin_source" || -L "$plugin_source" ]]; then
        missing+=("GStreamer plugin ${plugin_file}")
      elif ! ln -s "$plugin_source" "${plugin_directory}/${plugin_file}"; then
        missing+=("GStreamer plugin ${plugin_file} (could not prepare isolated scan)")
      fi
    done
    registry_output="${registry_directory}/elements.txt"
    registry_error="${registry_directory}/diagnostics.txt"
    export GST_PLUGIN_PATH_1_0="$plugin_directory"
    export GST_PLUGIN_SYSTEM_PATH_1_0="$plugin_directory"
    export GST_PLUGIN_PATH="$plugin_directory"
    export GST_PLUGIN_SYSTEM_PATH="$plugin_directory"
    export GST_REGISTRY="${registry_directory}/registry.bin"
    export GST_REGISTRY_1_0="$GST_REGISTRY"

    if "$bounded_timeout" --kill-after=2s 30s "$gst_inspect" --print-plugin-auto-install-info > "$registry_output" 2> "$registry_error"; then
      registry_status=0
    else
      registry_status=$?
    fi

    if ((registry_status == 0)); then
      for element in "${elements[@]}"; do
        if ! grep -Fxq "element-${element}" "$registry_output"; then
          missing+=("GStreamer element ${element}")
        fi
      done
    elif ((registry_status == 124 || registry_status == 137)); then
      missing+=("GStreamer plugin scan timed out after 30 seconds")
    else
      missing+=("GStreamer plugin scan failed (exit ${registry_status})")
    fi
    if ((registry_status != 0)); then
      diagnostic=""
      element=""
      while IFS= read -r element || [[ -n "$element" ]]; do
        [[ -n "$element" ]] && diagnostic="${element:0:240}"
      done < "$registry_error"
      [[ -z "$diagnostic" ]] || log "GStreamer scan diagnostic: ${diagnostic}"
    fi

    if ! rm -rf "$registry_directory"; then
      missing+=("GStreamer temporary registry cleanup failed")
    fi
    if [[ -n "$had_registry" ]]; then export GST_REGISTRY="$previous_registry"; else unset GST_REGISTRY; fi
    if [[ -n "$had_registry_1_0" ]]; then export GST_REGISTRY_1_0="$previous_registry_1_0"; else unset GST_REGISTRY_1_0; fi
    export GST_PLUGIN_PATH_1_0="${engine_dir}/lib/gstreamer-1.0"
    export GST_PLUGIN_SYSTEM_PATH_1_0="${engine_dir}/lib/gstreamer-1.0"
    if [[ -n "$had_plugin_path" ]]; then export GST_PLUGIN_PATH="$previous_plugin_path"; else unset GST_PLUGIN_PATH; fi
    if [[ -n "$had_plugin_system_path" ]]; then export GST_PLUGIN_SYSTEM_PATH="$previous_plugin_system_path"; else unset GST_PLUGIN_SYSTEM_PATH; fi
    if ((registry_status != 0)); then return 1; fi
    return 0
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
  local executable version_output version_number min_version
  executable="$(runtime_executable uxplay || true)"
  if [[ -z "$executable" ]]; then
    missing+=("uxplay ${expect_uxplay_version:-$UXPLAY_VERSION}")
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
  if [[ -z "$version_number" ]]; then
    missing+=("uxplay ${expect_uxplay_version:-$UXPLAY_VERSION} or newer")
    return 1
  fi
  if [[ "$expect_uxplay_version" =~ ^[0-9a-fA-F]{40}$ ]]; then
    return 0
  fi
  min_version="${expect_uxplay_version:-$UXPLAY_VERSION}"
  if ! awk -v actual="$version_number" -v minimum="$min_version" 'BEGIN { split(actual, a, "."); split(minimum, b, "."); for (i=1; i<=3; i++) { if ((a[i]+0) > (b[i]+0)) exit 0; if ((a[i]+0) < (b[i]+0)) exit 1 } exit 0 }'; then
    missing+=("uxplay ${min_version} or newer")
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
  [[ -f "${engine_bin}/scrcpy-server" && ! -L "${engine_bin}/scrcpy-server" && -s "${engine_bin}/scrcpy-server" ]] || missing+=("scrcpy server (scrcpy-server)")
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
  load_node_version
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
  local config_dir env_file temporary line profile fish_file fish_dir
  config_dir="${HOME}/.config/neo-airplay"
  env_file="${config_dir}/env.sh"
  mkdir -p "$config_dir"
  [[ ! -L "$config_dir" ]] || fail "Configuration directory must not be a symbolic link: ${config_dir}"
  [[ ! -L "$env_file" ]] || fail "Environment file must not be a symbolic link: ${env_file}"
  temporary="$(mktemp "${env_file}.tmp.XXXXXX")"
  cleanup_paths+=("$temporary")
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
  fish_dir="$(dirname "$fish_file")"
  mkdir -p "$fish_dir"
  [[ ! -L "$fish_dir" && ! -L "$fish_file" ]] || fail "Fish config path must not be a symbolic link: ${fish_file}"
  if [[ ! -f "$fish_file" ]]; then
    temporary="$(mktemp "${fish_file}.tmp.XXXXXX")"
    cleanup_paths+=("$temporary")
    cat > "$temporary" <<'EOF'
fish_add_path --path $HOME/.local/bin $HOME/.cargo/bin $HOME/.bun/bin
EOF
    chmod 0644 "$temporary"
    mv -f -- "$temporary" "$fish_file"
  fi
  export PATH="${HOME}/.local/bin:${HOME}/.cargo/bin:${HOME}/.bun/bin:${PATH:-}"
}

if ((warm_registry)); then
  [[ "$(uname -s)" == "Darwin" ]] || fail "GStreamer registry warm-up is supported only on macOS."
  [[ -n "$registry_path" ]] || fail "--registry-path is required for GStreamer registry warm-up."

  local_gst_inspect="$(runtime_executable gst-inspect-1.0 || true)"
  local_timeout="$(runtime_executable gtimeout || true)"
  [[ -x "$local_gst_inspect" ]] || fail "GStreamer gst-inspect-1.0 is unavailable."
  [[ -x "$local_timeout" ]] || fail "GNU timeout utility is unavailable."

  registry_parent="$(dirname "$registry_path")"
  mkdir -p "$registry_parent"
  [[ ! -L "$registry_parent" && ! -L "$registry_path" ]] || fail "GStreamer registry path must not be a symbolic link."
  if [[ -s "$registry_path" ]]; then
    log "GStreamer registry cache already exists."
    exit 0
  fi

  export GST_PLUGIN_PATH_1_0="${engine_dir}/lib/gstreamer-1.0"
  export GST_PLUGIN_SYSTEM_PATH_1_0="${engine_dir}/lib/gstreamer-1.0"
  export GST_PLUGIN_SCANNER_1_0="${engine_bin}/gst-plugin-scanner"
  export GST_REGISTRY="$registry_path"
  export GST_REGISTRY_1_0="$registry_path"
  setup_macos_library_path

  warm_started_at=$SECONDS
  if warm_output="$("$local_timeout" --kill-after=2s 90s "$local_gst_inspect" --print-plugin-auto-install-info 2>&1)"; then
    warm_status=0
  else
    warm_status=$?
  fi
  warm_elapsed=$((SECONDS - warm_started_at))
  if ((warm_status != 0)); then
    rm -f "$registry_path"
    if ((warm_status == 124 || warm_status == 137)); then
      log "GStreamer registry warm-up timed out after 90 seconds (${warm_elapsed}s elapsed)."
    else
      log "GStreamer registry warm-up failed with exit ${warm_status} after ${warm_elapsed}s: ${warm_output:0:240}"
    fi
    exit 1
  fi
  if [[ ! -s "$registry_path" ]]; then
    rm -f "$registry_path"
    log "GStreamer registry warm-up did not create a cache file after ${warm_elapsed}s."
    exit 1
  fi
  log "GStreamer registry cache is ready after ${warm_elapsed}s."
  exit 0
fi

if ((check_only)); then
  if check_runtime; then runtime_status=0; else runtime_status=1; fi
  if ((development)); then
    check_development || runtime_status=1
  fi
  exit "$runtime_status"
fi

run_install_lifecycle=1
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
if [[ -n "$marker_file" ]]; then
  marker_dir="$(dirname "$marker_file")"
  mkdir -p "$marker_dir"
  [[ ! -L "$marker_dir" && ! -L "$marker_file" ]] || fail "Marker file path must not be a symbolic link: ${marker_file}"
  temp_marker="$(mktemp "${marker_file}.tmp.XXXXXX")"
  cleanup_paths+=("$temp_marker")
  printf '%d\n' "$$" > "$temp_marker"
  mv -f -- "$temp_marker" "$marker_file"
fi
if [[ -n "$result_file" ]]; then
  result_dir="$(dirname "$result_file")"
  mkdir -p "$result_dir"
  [[ ! -L "$result_dir" && ! -L "$result_file" ]] || fail "Result file path must not be a symbolic link: ${result_file}"
fi

runtime_ready=0
development_ready=0
if check_runtime; then runtime_ready=1; fi
if ((development)) && check_development; then development_ready=1; fi
if ((runtime_ready && (!development || development_ready) && !upgrade)); then
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
  local archive work_dir version_output version_number
  if ((upgrade == 0 && uxplay_custom_requested == 0)) && [[ "$only_tool" != "uxplay" ]] && [[ -f "${engine_bin}/uxplay" && ! -L "${engine_bin}/uxplay" && -x "${engine_bin}/uxplay" ]]; then
    version_output="$("${engine_bin}/uxplay" -v 2>&1 || true)"
    version_number="$(printf '%s\n' "$version_output" | sed -nE 's/.*UxPlay version ([0-9]+(\.[0-9]+)+).*/\1/p' | head -n 1 || true)"
    if [[ -n "$version_number" ]] && awk -v actual="$version_number" -v minimum="$UXPLAY_VERSION" 'BEGIN { split(actual, a, "."); split(minimum, b, "."); for (i=1; i<=3; i++) { if ((a[i]+0) > (b[i]+0)) exit 0; if ((a[i]+0) < (b[i]+0)) exit 1 } exit 0 }'; then
      return
    fi
  fi
  work_dir="$(mktemp -d "${TMPDIR:-/tmp}/neo-airplay-uxplay.XXXXXX")"
  cleanup_paths+=("$work_dir")
  archive="${work_dir}/uxplay.tar.gz"
  if [[ -n "$UXPLAY_SHA256" ]]; then
    download_verified "$UXPLAY_URL" "$archive" "$UXPLAY_SHA256" "UxPlay source archive"
  elif ((allow_unverified_uxplay)); then
    log "Downloading UxPlay source (${UXPLAY_COMMIT}) via HTTPS TLS 1.2+ without pinned SHA-256."
    download_https_unverified "$UXPLAY_URL" "$archive" "UxPlay source archive (${UXPLAY_COMMIT})"
  else
    fail "Refusing to download UxPlay source without SHA-256 verification."
  fi
  mkdir -p "${work_dir}/source"
  tar -xzf "$archive" -C "${work_dir}/source" --strip-components=1
  cmake -S "${work_dir}/source" -B "${work_dir}/build" -DCMAKE_INSTALL_PREFIX="$engine_dir"
  cmake --build "${work_dir}/build" --parallel
  cmake --install "${work_dir}/build"
  rm -rf -- "$work_dir"
}

external_runtime_executable() {
  local command_name="$1" directory candidate
  local path_entries=()
  IFS=: read -r -a path_entries <<< "${PATH:-}"
  for directory in "${path_entries[@]}"; do
    [[ "$directory" == "$engine_dir" || "$directory" == "${engine_dir}/"* ]] && continue
    candidate="${directory}/${command_name}"
    if [[ -x "$candidate" ]]; then
      printf '%s' "$candidate"
      return 0
    fi
  done
  return 1
}

stage_runtime_tool() {
  local command_name="$1" source_path="${2:-}" target_path
  target_path="${engine_bin}/${command_name}"
  if [[ -f "$target_path" && ! -L "$target_path" && -x "$target_path" ]] && ((upgrade == 0)); then return 0; fi
  if ((upgrade)); then
    if [[ "$source_path" == "${engine_dir}/"* ]]; then source_path=""; fi
    if [[ -z "$source_path" ]]; then
      source_path="$(external_runtime_executable "$command_name" || true)"
    fi
  fi
  if [[ -f "$target_path" && ! -L "$target_path" && -x "$target_path" && -z "$source_path" ]]; then return 0; fi
  if [[ -z "$source_path" ]]; then source_path="$(command -v "$command_name" 2>/dev/null || true)"; fi
  [[ -n "$source_path" && -x "$source_path" ]] || fail "Required runtime tool is not available for staging: ${command_name}."
  stage_file_atomic "$source_path" "$target_path" 0755 1
}

stage_runtime_tools() {
  local scrcpy_server scrcpy_prefix
  if ((upgrade)); then
    rm -rf "${engine_dir}/lib/gstreamer-1.0" "${engine_dir}/share/gstreamer-1.0"
    if [[ "$(uname -s)" == "Linux" && "$(uname -m)" != x86_64 && "$(uname -m)" != amd64 ]]; then
      rm -f "${engine_bin}/scrcpy-server"
    fi
  fi
  stage_runtime_tool adb
  stage_runtime_tool scrcpy
  stage_runtime_tool gst-inspect-1.0
  if [[ "$(uname -s)" == "Darwin" ]]; then
    stage_runtime_tool gtimeout "$(command -v gtimeout 2>/dev/null || true)"
  else
    stage_runtime_tool timeout "$(command -v timeout 2>/dev/null || true)"
  fi
  stage_gstreamer_plugins
  if [[ ! -f "${engine_bin}/scrcpy-server" || -L "${engine_bin}/scrcpy-server" || ! -s "${engine_bin}/scrcpy-server" ]]; then
    scrcpy_server="$(dirname "$(command -v scrcpy)")/scrcpy-server"
    if [[ -s "$scrcpy_server" ]]; then stage_file_atomic "$scrcpy_server" "${engine_bin}/scrcpy-server" 0644 1; fi
    for scrcpy_server in /usr/share/scrcpy/scrcpy-server /usr/lib/scrcpy/scrcpy-server /usr/lib64/scrcpy/scrcpy-server; do
      if [[ -s "$scrcpy_server" ]]; then stage_file_atomic "$scrcpy_server" "${engine_bin}/scrcpy-server" 0644 1; break; fi
    done
    if [[ ! -s "${engine_bin}/scrcpy-server" ]] && command -v brew >/dev/null 2>&1; then
      scrcpy_prefix="$(brew --prefix scrcpy 2>/dev/null || true)"
      if [[ -n "$scrcpy_prefix" ]]; then
        scrcpy_server="${scrcpy_prefix}/share/scrcpy/scrcpy-server"
        if [[ -s "$scrcpy_server" ]]; then stage_file_atomic "$scrcpy_server" "${engine_bin}/scrcpy-server" 0644 1; fi
      fi
    fi
  fi
  if [[ ! -f "${engine_bin}/scrcpy-server" || -L "${engine_bin}/scrcpy-server" || ! -s "${engine_bin}/scrcpy-server" ]]; then
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
  [[ ! -L "${engine_dir}/lib" && ! -L "$plugins_target" ]] || fail "GStreamer plugin target directory must not be a symbolic link."
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
  stage_file_atomic "$scanner_source" "${engine_bin}/gst-plugin-scanner" 0755 1
}

ensure_homebrew() {
  local work_dir installer_script
  if [[ -x /opt/homebrew/bin/brew ]]; then
    eval "$(/opt/homebrew/bin/brew shellenv)"
  elif [[ -x /usr/local/bin/brew ]]; then
    eval "$(/usr/local/bin/brew shellenv)"
  fi
  export PATH="/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/local/sbin:${PATH:-}"
  if command -v brew >/dev/null 2>&1; then
    eval "$(brew shellenv)"
    export PATH="$(brew --prefix)/bin:$(brew --prefix)/sbin:/opt/homebrew/bin:/opt/homebrew/sbin:/usr/local/bin:/usr/local/sbin:${PATH:-}"
    return
  fi
  [[ -t 0 && -t 1 ]] || fail "Homebrew is missing. Run this installer from Terminal so the official Homebrew installer can request any required authorization."
  command -v curl >/dev/null 2>&1 || fail "curl is required to install Homebrew from its official source."
  log "Installing Homebrew from the pinned official installer."
  work_dir="$(mktemp -d "${TMPDIR:-/tmp}/neo-airplay-homebrew.XXXXXX")"
  cleanup_paths+=("$work_dir")
  installer_script="${work_dir}/install.sh"
  download_verified "$HOMEBREW_INSTALL_URL" "$installer_script" "$HOMEBREW_INSTALL_SHA256" "Homebrew installer"
  /bin/bash "$installer_script"
  rm -rf -- "$work_dir"
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
  if ((upgrade)); then
    log "Refreshing Homebrew metadata before updating runtime dependencies."
    brew update --quiet
  fi
  log "Installing GStreamer and UxPlay build dependencies with Homebrew."
  brew install --quiet cmake coreutils libplist openssl@3 pkg-config gstreamer
  if ((upgrade)); then
    local package
    for package in cmake coreutils libplist openssl@3 pkg-config gstreamer; do
      brew upgrade --quiet "$package"
    done
  fi
  stage_runtime_tool gtimeout "$(command -v gtimeout 2>/dev/null || true)"
  bounded_timeout="$(runtime_executable gtimeout || true)"
  install_uxplay_source
  install_scrcpy_macos
  stage_runtime_tools
}

scrcpy_bundle_ready() {
  local scrcpy_output adb_output
  [[ -f "${engine_bin}/scrcpy" && ! -L "${engine_bin}/scrcpy" && -x "${engine_bin}/scrcpy" && -f "${engine_bin}/adb" && ! -L "${engine_bin}/adb" && -x "${engine_bin}/adb" && -f "${engine_bin}/scrcpy-server" && ! -L "${engine_bin}/scrcpy-server" && -s "${engine_bin}/scrcpy-server" && -n "$bounded_timeout" ]] || return 1
  if ! scrcpy_output="$("$bounded_timeout" --kill-after=2s 10s "${engine_bin}/scrcpy" --version 2>&1)" || [[ -z "$scrcpy_output" ]]; then
    return 1
  fi
  if ! adb_output="$("$bounded_timeout" --kill-after=2s 10s "${engine_bin}/adb" version 2>&1)" || [[ -z "$adb_output" ]]; then
    return 1
  fi
}

install_scrcpy_macos() {
  local architecture asset builtin_sha expected_sha work_dir archive extract_dir bundle binary server adb
  architecture="$(uname -m)"
  case "$architecture" in
    arm64|aarch64)
      asset="scrcpy-macos-aarch64-v${SCRCPY_VERSION}.tar.gz"
      builtin_sha="$DEFAULT_SCRCPY_MACOS_ARM64_SHA256" ;;
    x86_64|amd64)
      asset="scrcpy-macos-x86_64-v${SCRCPY_VERSION}.tar.gz"
      builtin_sha="$DEFAULT_SCRCPY_MACOS_X86_64_SHA256" ;;
    *) fail "The official scrcpy release does not support macOS ${architecture}." ;;
  esac
  if ((upgrade == 0 && scrcpy_custom_requested == 0)) && [[ "$only_tool" != "scrcpy" ]] && scrcpy_bundle_ready; then return; fi
  expected_sha="$(resolve_scrcpy_sha256 "$SCRCPY_VERSION" "$asset" "$builtin_sha")"
  work_dir="$(mktemp -d "${TMPDIR:-/tmp}/neo-airplay-scrcpy.XXXXXX")"
  cleanup_paths+=("$work_dir")
  archive="${work_dir}/${asset}"
  extract_dir="${work_dir}/extract"
  mkdir -p "$extract_dir"
  download_verified "https://github.com/Genymobile/scrcpy/releases/download/v${SCRCPY_VERSION}/${asset}" "$archive" "$expected_sha" "Official macOS scrcpy archive"
  tar -xzf "$archive" -C "$extract_dir"
  bundle="$(find "$extract_dir" -type d -name "scrcpy-macos-*-v${SCRCPY_VERSION}" -print -quit)"
  if [[ -z "$bundle" ]]; then
    bundle="$(find "$extract_dir" -mindepth 1 -maxdepth 1 -type d -print -quit)"
  fi
  [[ -n "$bundle" ]] || fail "The official macOS scrcpy archive did not contain its distribution directory."
  binary="${bundle}/scrcpy"
  server="${bundle}/scrcpy-server"
  adb="${bundle}/adb"
  [[ -x "$binary" && -f "$server" && -x "$adb" ]] || fail "The official macOS scrcpy archive is missing required executables."
  stage_file_atomic "$binary" "${engine_bin}/scrcpy" 0755 0
  stage_file_atomic "$server" "${engine_bin}/scrcpy-server" 0644 0
  stage_file_atomic "$adb" "${engine_bin}/adb" 0755 0
  rm -rf -- "$work_dir"
}

install_scrcpy_linux() {
  local architecture asset expected_sha work_dir archive extract_dir binary server adb
  if ((upgrade == 0 && scrcpy_custom_requested == 0)) && [[ "$only_tool" != "scrcpy" ]] && scrcpy_bundle_ready; then return; fi
  architecture="$(uname -m)"
  if [[ "$architecture" == x86_64 || "$architecture" == amd64 ]]; then
    asset="scrcpy-linux-x86_64-v${SCRCPY_VERSION}.tar.gz"
    expected_sha="$(resolve_scrcpy_sha256 "$SCRCPY_VERSION" "$asset" "$DEFAULT_SCRCPY_LINUX_SHA256")"
    work_dir="$(mktemp -d "${TMPDIR:-/tmp}/neo-airplay-scrcpy.XXXXXX")"
    cleanup_paths+=("$work_dir")
    archive="${work_dir}/scrcpy.tar.gz"
    extract_dir="${work_dir}/extract"
    mkdir -p "$extract_dir"
    download_verified "$SCRCPY_LINUX_URL" "$archive" "$expected_sha" "scrcpy source archive"
    tar -xzf "$archive" -C "$extract_dir"
    binary="$(find "$extract_dir" -type f -name scrcpy -perm -u+x -print -quit)"
    server="$(find "$extract_dir" -type f -name scrcpy-server -print -quit)"
    [[ -n "$binary" && -n "$server" ]] || fail "The official scrcpy release archive did not contain its client and server binaries."
    adb="$(find "$extract_dir" -type f -name adb -perm -u+x -print -quit)"
    [[ -n "$adb" ]] || fail "The official scrcpy release archive did not contain its ADB executable."
    stage_file_atomic "$binary" "${engine_bin}/scrcpy" 0755 0
    stage_file_atomic "$server" "${engine_bin}/scrcpy-server" 0644 0
    stage_file_atomic "$adb" "${engine_bin}/adb" 0755 0
    rm -rf -- "$work_dir"
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
  cleanup_paths+=("$work_dir")
  archive="${work_dir}/${asset}"
  extracted="${work_dir}/extract"
  mkdir -p "$extracted"
  download_verified "https://github.com/oven-sh/bun/releases/download/bun-v${BUN_VERSION}/${asset}" "$archive" "$expected_sha" "Bun ${BUN_VERSION} archive"
  unzip -q "$archive" -d "$extracted"
  executable="$(find "$extracted" -type f -name bun -print -quit)"
  [[ -n "$executable" ]] || fail "Official Bun archive is missing its executable."
  target_dir="${HOME}/.bun/bin"
  mkdir -p "$target_dir"
  stage_file_atomic "$executable" "${target_dir}/bun" 0755 0
  rm -rf -- "$work_dir"
  export PATH="${target_dir}:${PATH}"
}

install_node_binary() {
  local system architecture asset expected_sha work_dir archive extracted node_root node_dir
  load_node_version
  if [[ "$(node --version 2>/dev/null || true)" == "v${NODE_VERSION}" ]]; then return; fi
  system="$(uname -s)"
  architecture="$(uname -m)"
  case "$system:$architecture" in
    Linux:x86_64|Linux:amd64)
      asset="node-v${NODE_VERSION}-linux-x64.tar.xz"; expected_sha="48cc0e21e048790e54c0cf5749c2725005313d652651c40b7a21feb9697aaf75" ;;
    Linux:aarch64|Linux:arm64)
      asset="node-v${NODE_VERSION}-linux-arm64.tar.xz"; expected_sha="eb187837a2230d4047161ac0be008584e16b46f726e9c9490ee35dc6b17d9ba8" ;;
    Darwin:x86_64|Darwin:amd64)
      asset="node-v${NODE_VERSION}-darwin-x64.tar.xz"; expected_sha="6965578fd823e72c1b5387f5ab84cb25368f886e02eb8779e6f0fce6b91c27b7" ;;
    Darwin:arm64|Darwin:aarch64)
      asset="node-v${NODE_VERSION}-darwin-arm64.tar.xz"; expected_sha="1f5ac075be50bed4780ea3be19a007fda95ea4d527d28d76d0b91de47d915115" ;;
    *) fail "Official Node.js ${NODE_VERSION} binaries are unavailable for ${system}/${architecture}." ;;
  esac
  work_dir="$(mktemp -d "${TMPDIR:-/tmp}/neo-airplay-node.XXXXXX")"
  cleanup_paths+=("$work_dir")
  archive="${work_dir}/${asset}"
  extracted="${work_dir}/extract"
  mkdir -p "$extracted"
  download_verified "https://nodejs.org/dist/v${NODE_VERSION}/${asset}" "$archive" "$expected_sha" "Node.js ${NODE_VERSION} archive"
  tar -xJf "$archive" -C "$extracted"
  node_root="$(find "$extracted" -mindepth 1 -maxdepth 1 -type d -name "node-v${NODE_VERSION}-*" -print -quit)"
  [[ -n "$node_root" && -x "${node_root}/bin/node" && ! -L "${node_root}/bin/node" ]] || fail "Official Node.js archive did not contain its node binary."
  node_dir="${HOME}/.local/lib/neo-airplay/node-v${NODE_VERSION}"
  mkdir -p "$node_dir" "$user_bin"
  [[ ! -L "$node_dir" && ! -L "$user_bin" ]] || fail "Node.js installation target must not be a symbolic link."
  cp -a "${node_root}/." "$node_dir/"
  ln -sf "${node_dir}/bin/node" "${user_bin}/node"
  ln -sf "${node_dir}/bin/npm" "${user_bin}/npm"
  ln -sf "${node_dir}/bin/npx" "${user_bin}/npx"
  rm -rf -- "$work_dir"
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
  local runtime_packages=() build_packages=() development_packages=() dnf_arguments=()
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
        fail "Official DNF repositories do not provide scrcpy for ${architecture}. Install scrcpy manually from a trusted source before retrying."
      fi
      if ((upgrade)); then dnf_arguments+=(--refresh); fi
      run_privileged dnf -q -y "${dnf_arguments[@]}" install "${runtime_packages[@]}" "${build_packages[@]}"
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
  stage_runtime_tool timeout "$(command -v timeout 2>/dev/null || true)"
  bounded_timeout="$(runtime_executable timeout || true)"
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
  local temp_dir rust_installer rustup_command rustup_target rustup_sha architecture
  temp_dir="$(mktemp -d "${TMPDIR:-/tmp}/neo-airplay-devtools.XXXXXX")"
  cleanup_paths+=("$temp_dir")
  if ! command -v rustup >/dev/null 2>&1; then
    architecture="$(uname -m)"
    case "$architecture" in
      x86_64|amd64)
        rustup_target="x86_64-unknown-linux-gnu"
        rustup_sha="$RUSTUP_LINUX_X64_SHA256" ;;
      aarch64|arm64)
        rustup_target="aarch64-unknown-linux-gnu"
        rustup_sha="$RUSTUP_LINUX_ARM64_SHA256" ;;
      *) fail "Official rustup ${RUSTUP_VERSION} binaries are unavailable for Linux/${architecture}." ;;
    esac
    rust_installer="${temp_dir}/rustup-init"
    download_verified "https://static.rust-lang.org/rustup/archive/${RUSTUP_VERSION}/${rustup_target}/rustup-init" "$rust_installer" "$rustup_sha" "Rustup ${RUSTUP_VERSION} installer"
    chmod 0700 "$rust_installer"
    "$rust_installer" -y --no-modify-path --profile minimal --default-toolchain stable
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

if [[ "$only_tool" == "scrcpy" ]]; then
  mkdir -p "$engine_bin"
  case "$(uname -s)" in
    Darwin)
      bounded_timeout="$(runtime_executable gtimeout || command -v gtimeout 2>/dev/null || true)"
      install_scrcpy_macos
      ;;
    Linux)
      bounded_timeout="$(runtime_executable timeout || command -v timeout 2>/dev/null || true)"
      install_scrcpy_linux
      stage_runtime_tool scrcpy
      stage_runtime_tool adb
      ;;
    *) fail "This installer supports macOS and Linux. Use install.ps1 on Windows." ;;
  esac
  if [[ ! -x "${engine_bin}/scrcpy" || ! -x "${engine_bin}/adb" || ! -s "${engine_bin}/scrcpy-server" ]]; then
    fail "scrcpy tools were not staged completely in ${engine_bin}."
  fi
  exit 0
fi

if [[ "$only_tool" == "uxplay" ]]; then
  mkdir -p "$engine_bin"
  case "$(uname -s)" in
    Darwin)
      ensure_xcode_cli_tools
      ensure_homebrew
      export HOMEBREW_PREFIX="$(brew --prefix)"
      if command -v cmake >/dev/null 2>&1 && pkg-config --exists gstreamer-1.0 2>/dev/null && [[ -d "${engine_dir}/lib/gstreamer-1.0" ]]; then
        install_uxplay_source
      else
        install_macos_runtime
      fi
      ;;
    Linux)
      if command -v cmake >/dev/null 2>&1 && pkg-config --exists gstreamer-1.0 2>/dev/null && [[ -d "${engine_dir}/lib/gstreamer-1.0" ]]; then
        install_uxplay_source
      else
        install_linux_runtime
      fi
      ;;
    *) fail "This installer supports macOS and Linux. Use install.ps1 on Windows." ;;
  esac
  [[ -x "${engine_bin}/uxplay" ]] || fail "UxPlay executable was not staged in ${engine_bin}."
  exit 0
fi

if ((!runtime_ready || upgrade)); then
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
