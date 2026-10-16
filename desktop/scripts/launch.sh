#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd "$(dirname "$0")" && pwd)"
project_dir="$(cd "$script_dir/../.." && pwd)"
mode="${1:-}"

if [[ "$mode" != "dev" && "$mode" != "release" && "$mode" != "bundle" ]]; then
  printf 'Usage: bash desktop/scripts/launch.sh <dev|release|bundle> [tauri arguments...]\n' >&2
  exit 2
fi
shift
if [[ "$mode" == "bundle" && $# -gt 0 ]]; then
  printf 'The bundle launcher does not accept additional arguments.\n' >&2
  exit 2
fi

/bin/bash "$script_dir/setup.sh"
export PATH="$HOME/.local/bin:$HOME/.bun/bin:$HOME/.cargo/bin:/opt/homebrew/opt/node@22/bin:/usr/local/opt/node@22/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
export NEO_AIRPLAY_SETUP_COMPLETE=1
cd "$project_dir/desktop"

case "$mode" in
  dev) bun run tauri dev "$@" ;;
  release) bun run release "$@" ;;
  bundle) bun run bundle "$@" ;;
esac
