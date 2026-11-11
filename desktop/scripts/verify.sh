#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
desktop_dir="$(cd "$script_dir/.." && pwd)"

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

node_version_file="$desktop_dir/.node-version"
[[ -r "$node_version_file" ]] || fail "Node.js version file is unavailable: $node_version_file"
read -r required_node_version < "$node_version_file"
[[ -n "$required_node_version" ]] || fail "Node.js version file is empty: $node_version_file"

if [[ ! "$required_node_version" =~ ^([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
  fail "Invalid Node.js version in .node-version: '$required_node_version'."
fi
required_node_major="${BASH_REMATCH[1]}"
required_node_minor="${BASH_REMATCH[2]}"
required_node_patch="${BASH_REMATCH[3]}"

if [[ "${NEO_AIRPLAY_SKIP_SETUP:-}" != "1" ]]; then
  path_prefix=()
  if [[ -n "${HOME:-}" ]]; then
    path_prefix+=("$HOME/.local/bin" "$HOME/.bun/bin" "$HOME/.cargo/bin")
  fi
  if [[ "$(uname -s)" == "Darwin" ]]; then
    path_prefix+=(
      "/opt/homebrew/opt/node@${required_node_major}/bin"
      "/usr/local/opt/node@${required_node_major}/bin"
      /opt/homebrew/bin
      /usr/local/bin
    )
  fi
  if ((${#path_prefix[@]} > 0)); then
    path_addition="$(IFS=:; printf '%s' "${path_prefix[*]}")"
    export PATH="${path_addition}${PATH:+:$PATH}"
  fi
fi

command -v bun >/dev/null 2>&1 || fail 'Bun is required. Install the version documented in README.md.'
command -v cargo >/dev/null 2>&1 || fail 'Rust and Cargo are required. Install the stable toolchain documented in README.md.'
command -v node >/dev/null 2>&1 || fail "Node.js ${required_node_major} is required for frontend unit tests. Install the version documented in README.md."

if ! node_version="$(node --version 2>/dev/null)"; then
  fail 'Unable to read the Node.js version required for frontend unit tests.'
fi
if [[ ! "$node_version" =~ ^v([0-9]+)\.([0-9]+)\.([0-9]+)$ ]]; then
  fail "Node.js ${required_node_version} or newer within the ${required_node_major}.x line is required for frontend unit tests. Found '$node_version'."
fi

node_major="${BASH_REMATCH[1]}"
node_minor="${BASH_REMATCH[2]}"
node_patch="${BASH_REMATCH[3]}"
if ((10#$node_major != 10#$required_node_major || 10#$node_minor < 10#$required_node_minor || (10#$node_minor == 10#$required_node_minor && 10#$node_patch < 10#$required_node_patch))); then
  fail "Node.js ${required_node_version} or newer within the ${required_node_major}.x line is required for frontend unit tests. Found '$node_version'."
fi

cd "$desktop_dir"
bun run --silent verify
