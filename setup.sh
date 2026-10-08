#!/bin/bash
set -euo pipefail
project_dir="$(cd "$(dirname "$0")" && pwd)"
/bin/bash "$project_dir/desktop/src-tauri/resources/setup/install.sh" --development
export PATH="$HOME/.local/bin:$HOME/.bun/bin:$HOME/.cargo/bin:/opt/homebrew/opt/node@22/bin:/usr/local/opt/node@22/bin:/opt/homebrew/bin:/usr/local/bin:$PATH"
cd "$project_dir/desktop"
bun install --frozen-lockfile --silent
printf '%s\n' '開發環境已準備完成。請開啟新終端，再執行 cd desktop && bun run tauri dev。'
