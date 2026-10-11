#!/bin/bash
set -euo pipefail
script_dir="$(cd "$(dirname "$0")" && pwd)"
project_dir="$(cd "$script_dir/../.." && pwd)"
/bin/bash "$script_dir/install.sh" --development
export PATH="$HOME/.local/bin:$HOME/.bun/bin:$HOME/.cargo/bin:/opt/homebrew/opt/node@22/bin:/usr/local/opt/node@22/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
cd "$project_dir/desktop"
bun install --frozen-lockfile --silent
printf '%s\n' '開發環境已準備完成。'
