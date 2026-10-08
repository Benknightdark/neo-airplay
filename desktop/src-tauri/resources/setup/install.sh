#!/usr/bin/env bash
set -Eeuo pipefail

readonly UXPLAY_VERSION="1.73.7"
readonly UXPLAY_COMMIT="df67c212a433cf6dda3676dd40c097900d24e645"
readonly UXPLAY_SHA256="168de53ed688f87c4022d79357ce91f9608ad5c53236bff95134d19b88efeb3f"
readonly UXPLAY_URL="https://github.com/FDH2/UxPlay/archive/${UXPLAY_COMMIT}.tar.gz"
readonly BUN_VERSION="1.4.2"
readonly NODE_VERSION="22.23.3"
readonly SCRCPY_VERSION="5.0.1"
readonly SCRCPY_SHA256="9f969d30cc574816077edecda65719c1c70b0aee1df1ba5fa00779ac07b8fd6e"
readonly SCRCPY_URL="https://github.com/Genymobile/scrcpy/releases/download/v${SCRCPY_VERSION}/scrcpy-linux-x86_64-v${SCRCPY_VERSION}.tar.gz"

development=0
check_only=0
gui=0
marker_file=""
result_file=""

while (($#)); do
  argument="$1"
  case "$argument" in
    --development) development=1 ;;
    --check) check_only=1 ;;
    --gui) gui=1 ;;
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

log() { printf '%s\n' "$*"; }
fail() { printf 'Setup failed: %s\n' "$*" >&2; exit 1; }

user_bin="${HOME}/.local/bin"
export PATH="${user_bin}:${HOME}/.cargo/bin:${HOME}/.bun/bin:${PATH:-}"

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

check_gstreamer() {
  local element
  if ! command -v gst-inspect-1.0 >/dev/null 2>&1; then
    missing+=("GStreamer (gst-inspect-1.0)")
    return 1
  fi
  [[ -n "$bounded_timeout" ]] || return 1
  for element in decodebin h264parse videoconvert autovideosink autoaudiosink avdec_aac; do
    if ! "$bounded_timeout" --kill-after=2s 10s gst-inspect-1.0 "$element" >/dev/null 2>&1; then
      missing+=("GStreamer element ${element}")
    fi
  done
}

check_version_probe() {
  local command_name="$1" argument="$2" label="$3" probe_output
  if ! command -v "$command_name" >/dev/null 2>&1; then
    missing+=("$command_name")
    return 1
  fi
  if [[ -z "$bounded_timeout" ]]; then
    missing+=("GNU timeout utility for bounded runtime probes")
    return 1
  fi
  if ! probe_output="$("$bounded_timeout" --kill-after=2s 10s "$command_name" "$argument" 2>&1)" || [[ -z "$probe_output" ]]; then
    missing+=("$label (unusable or version probe timed out)")
    return 1
  fi
}

check_uxplay() {
  local executable version_output version_number
  executable="$(command -v uxplay 2>/dev/null || true)"
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
  version_number="$(printf '%s\n' "$version_output" | grep -Eo '[0-9]+(\.[0-9]+)+' | head -n 1 || true)"
  if [[ -z "$version_number" ]] || ! awk -v actual="$version_number" -v minimum="$UXPLAY_VERSION" 'BEGIN { split(actual, a, "."); split(minimum, b, "."); for (i=1; i<=3; i++) { if ((a[i]+0) > (b[i]+0)) exit 0; if ((a[i]+0) < (b[i]+0)) exit 1 } exit 0 }'; then
    missing+=("uxplay ${UXPLAY_VERSION} or newer")
    return 1
  fi
}

check_runtime() {
  missing=()
  if [[ "$(uname -s)" == "Darwin" ]]; then
    bounded_timeout="$(command -v gtimeout 2>/dev/null || true)"
  else
    bounded_timeout="$(command -v timeout 2>/dev/null || true)"
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

mkdir -p "$user_bin"

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
  if [[ -n "$(command -v uxplay 2>/dev/null || true)" ]]; then
    version_output="$(uxplay -v 2>&1 || true)"
    version_number="$(printf '%s\n' "$version_output" | grep -Eo '[0-9]+(\.[0-9]+)+' | head -n 1 || true)"
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
  cmake -S "${work_dir}/source" -B "${work_dir}/build" -DCMAKE_INSTALL_PREFIX="$HOME/.local"
  cmake --build "${work_dir}/build" --parallel
  cmake --install "${work_dir}/build"
  rm -rf "$work_dir"
  export PATH="${user_bin}:${PATH}"
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
  log "Installing GStreamer, scrcpy, ADB, and UxPlay build dependencies with Homebrew."
  brew install --quiet cmake coreutils libplist openssl@3 pkg-config gstreamer scrcpy
  brew install --cask android-platform-tools
  install_uxplay_source
}

install_scrcpy_linux() {
  local architecture work_dir archive actual_sha extract_dir binary server target_dir
  if command -v scrcpy >/dev/null 2>&1 && [[ -n "$bounded_timeout" ]] && "$bounded_timeout" --kill-after=2s 10s scrcpy --version >/dev/null 2>&1; then return; fi
  architecture="$(uname -m)"
  if [[ "$architecture" == x86_64 || "$architecture" == amd64 ]]; then
    work_dir="$(mktemp -d "${TMPDIR:-/tmp}/neo-airplay-scrcpy.XXXXXX")"
    archive="${work_dir}/scrcpy.tar.gz"
    extract_dir="${work_dir}/extract"
    mkdir -p "$extract_dir"
    curl --proto '=https' --tlsv1.2 --fail --silent --show-error --location "$SCRCPY_URL" --output "$archive"
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
    target_dir="${HOME}/.local/lib/neo-airplay/scrcpy-${SCRCPY_VERSION}"
    mkdir -p "$target_dir" "$user_bin"
    cp -a "$(dirname "$binary")/." "$target_dir/"
    chmod 0755 "${target_dir}/scrcpy"
    chmod 0644 "${target_dir}/scrcpy-server"
    ln -sf "${target_dir}/scrcpy" "${user_bin}/scrcpy"
    ln -sf "${target_dir}/scrcpy-server" "${user_bin}/scrcpy-server"
    rm -rf "$work_dir"
    return
  fi
  if command -v apt-cache >/dev/null 2>&1 && apt-cache show scrcpy >/dev/null 2>&1; then
    run_privileged apt-get install -y -qq scrcpy
    return
  fi
  fail "The official static scrcpy release currently supports Linux x86_64 only, and no scrcpy package is available for ${architecture}."
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
      run_privileged apt-get install -y -qq --no-install-recommends libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libxdo-dev librsvg2-dev patchelf file unzip ;;
    dnf)
      run_privileged dnf -q -y install gtk3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel libxdo-devel librsvg2-devel patchelf file unzip ;;
    pacman)
      run_privileged pacman -Syu --noconfirm --noprogressbar --needed gtk3 webkit2gtk-4.1 libappindicator-gtk3 xdotool librsvg patchelf file unzip ;;
  esac
}

linux_packages() {
  local manager="$1"
  local runtime_packages=() build_packages=() development_packages=()
  case "$manager" in
    apt)
      runtime_packages=(adb avahi-daemon ca-certificates dbus procps gstreamer1.0-tools gstreamer1.0-plugins-base gstreamer1.0-libav gstreamer1.0-plugins-good gstreamer1.0-plugins-bad)
      build_packages=(build-essential cmake pkg-config curl unzip libssl-dev libplist-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libavahi-compat-libdnssd-dev)
      development_packages=(libgtk-3-dev libwebkit2gtk-4.1-dev libayatana-appindicator3-dev libxdo-dev librsvg2-dev patchelf file xz-utils)
      ;;
    dnf)
      runtime_packages=(android-tools scrcpy avahi ca-certificates dbus-daemon procps-ng gstreamer1-tools gstreamer1-plugins-base gstreamer1-plugin-libav gstreamer1-plugins-good gstreamer1-plugins-bad-free)
      build_packages=(gcc gcc-c++ cmake pkgconf-pkg-config curl unzip openssl-devel libplist-devel gstreamer1-devel gstreamer1-plugins-base-devel avahi-compat-libdns_sd-devel)
      development_packages=(gtk3-devel webkit2gtk4.1-devel libappindicator-gtk3-devel libxdo-devel librsvg2-devel patchelf file xz)
      ;;
    pacman)
      runtime_packages=(android-tools scrcpy avahi ca-certificates dbus procps gstreamer gst-plugins-base gst-libav gst-plugins-good gst-plugins-bad)
      build_packages=(base-devel cmake pkgconf curl unzip openssl libplist gstreamer gst-plugins-base)
      development_packages=(gtk3 webkit2gtk-4.1 libappindicator-gtk3 xdotool librsvg patchelf file xz)
      ;;
  esac
  if ((development)); then
    runtime_packages+=("${development_packages[@]}")
  fi
  case "$manager" in
    apt)
      run_privileged apt-get update -qq
      run_privileged apt-get install -y -qq --no-install-recommends "${runtime_packages[@]}" "${build_packages[@]}"
      ;;
    dnf)
      if ! dnf repoquery --quiet scrcpy >/dev/null 2>&1; then
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
  if [[ "$manager" == apt ]]; then install_scrcpy_linux; fi
  install_uxplay_source
  if command -v systemctl >/dev/null 2>&1; then
    if ! systemctl is-active --quiet avahi-daemon; then
      run_privileged systemctl enable --now avahi-daemon
    fi
  elif command -v service >/dev/null 2>&1 && ! pgrep -x avahi-daemon >/dev/null 2>&1; then
    run_privileged service avahi-daemon start
  fi
}

install_development_tools() {
  case "$(uname -s)" in
    Darwin)
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

case "$(uname -s)" in
  Darwin)
    install_macos_runtime
    ;;
  Linux)
    install_linux_runtime
    ;;
  *) fail "This installer supports macOS and Linux. Use install.ps1 on Windows." ;;
esac

if ((development)); then
  install_development_tools
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
