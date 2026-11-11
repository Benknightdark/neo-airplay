#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
desktop_dir="$(cd "$script_dir/.." && pwd)"

if [[ "${NEO_AIRPLAY_SKIP_SETUP:-}" != "1" ]]; then
  path_prefix=()
  if [[ -n "${HOME:-}" ]]; then
    path_prefix+=("$HOME/.local/bin" "$HOME/.bun/bin" "$HOME/.cargo/bin")
  fi
  if [[ "$(uname -s)" == "Darwin" ]]; then
    path_prefix+=(/opt/homebrew/opt/node@24/bin /usr/local/opt/node@24/bin /opt/homebrew/bin /usr/local/bin)
  fi
  if ((${#path_prefix[@]} > 0)); then
    path_addition="$(IFS=:; printf '%s' "${path_prefix[*]}")"
    export PATH="${path_addition}${PATH:+:$PATH}"
  fi
fi

fail() {
  printf '%s\n' "$1" >&2
  exit 1
}

command -v bun >/dev/null 2>&1 || fail 'Bun is required. Install the version documented in README.md.'
command -v cargo >/dev/null 2>&1 || fail 'Rust and Cargo are required. Install the stable toolchain documented in README.md.'
command -v node >/dev/null 2>&1 || fail 'Node.js 24 is required for frontend unit tests. Install the version documented in README.md.'

if ! node_version="$(node --version 2>/dev/null)"; then
  fail 'Unable to read the Node.js version required for frontend unit tests.'
fi
if [[ ! "$node_version" =~ ^v24\.([0-9]+)\.([0-9]+)$ ]]; then
  fail "Node.js 24.22.2 or newer within the 24.x line is required for frontend unit tests. Found '$node_version'."
fi

node_minor="${BASH_REMATCH[1]}"
node_patch="${BASH_REMATCH[2]}"
if ((10#$node_minor < 22 || (10#$node_minor == 22 && 10#$node_patch < 2))); then
  fail "Node.js 24.22.2 or newer within the 24.x line is required for frontend unit tests. Found '$node_version'."
fi

cd "$desktop_dir"
bun run --silent verify
