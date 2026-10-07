#!/usr/bin/env bash
set -eo pipefail

if [ "$(uname -s)" != "Darwin" ]; then
  printf '%s\n' 'This installer only runs on macOS.' >&2
  exit 2
fi
if ! command -v brew >/dev/null 2>&1; then
  printf '%s\n' 'Install Homebrew first, then run this script again.' >&2
  exit 2
fi
if ! xcode-select -p >/dev/null 2>&1; then
  printf '%s\n' 'Install the Xcode command line tools before building UxPlay.' >&2
  exit 2
fi

case "$(uname -m)" in
  arm64|x86_64) ;;
  *)
    printf 'Unsupported macOS architecture: %s\n' "$(uname -m)" >&2
    exit 2
    ;;
esac

for formula in cmake pkgconf libplist openssl@3 gstreamer; do
  if ! brew list --versions "$formula" >/dev/null 2>&1; then
    brew install "$formula"
  fi
done

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
RELEASE_SETTINGS="$SCRIPT_DIR/uxplay-release.json"
if [ ! -f "$RELEASE_SETTINGS" ]; then
  printf 'Missing pinned UxPlay source settings: %s\n' "$RELEASE_SETTINGS" >&2
  exit 2
fi
release_value() {
  awk -F '"' -v key="$1" '$2 == key { print $4; exit }' "$RELEASE_SETTINGS"
}
UXPLAY_VERSION="$(release_value Version)"
UXPLAY_ASSET="$(release_value AssetName)"
UXPLAY_URL="$(release_value Url)"
UXPLAY_SHA256="$(release_value Sha256)"
SHA_LENGTH="$(printf '%s' "$UXPLAY_SHA256" | wc -c | tr -d '[:space:]')"
if [ "$UXPLAY_VERSION" != "1.73.7" ] || [ "$SHA_LENGTH" -ne 64 ]; then
  printf '%s\n' 'Pinned UxPlay source settings are invalid.' >&2
  exit 2
fi

TEMP_BASE="$TMPDIR"
if [ -z "$TEMP_BASE" ]; then TEMP_BASE="/tmp"; fi
TEMP_ROOT="$(mktemp -d "$TEMP_BASE/neo-airplay-uxplay.XXXXXX")"
trap 'rm -rf "$TEMP_ROOT"' EXIT
ARCHIVE="$TEMP_ROOT/$UXPLAY_ASSET"
curl --fail --location --retry 3 --output "$ARCHIVE" "$UXPLAY_URL"
ACTUAL_SHA256="$(shasum -a 256 "$ARCHIVE" | awk '{ print $1 }')"
if [ "$ACTUAL_SHA256" != "$UXPLAY_SHA256" ]; then
  printf '%s\n' 'UxPlay source archive SHA-256 verification failed.' >&2
  exit 1
fi

tar -xzf "$ARCHIVE" -C "$TEMP_ROOT"
SOURCE_DIR="$TEMP_ROOT/UxPlay-$UXPLAY_VERSION"
if [ ! -f "$SOURCE_DIR/CMakeLists.txt" ]; then
  printf '%s\n' 'The UxPlay source archive has an unexpected directory layout.' >&2
  exit 1
fi

ENGINE_DIR="$HOME/Library/Application Support/NeoAirPlay/engine"
OPENSSL_PREFIX="$(brew --prefix openssl@3)"
LIBPLIST_PREFIX="$(brew --prefix libplist)"
GSTREAMER_PREFIX="$(brew --prefix gstreamer)"
export PKG_CONFIG_PATH="$OPENSSL_PREFIX/lib/pkgconfig:$LIBPLIST_PREFIX/lib/pkgconfig:$GSTREAMER_PREFIX/lib/pkgconfig"
export CPPFLAGS="-I$OPENSSL_PREFIX/include $CPPFLAGS"
export LDFLAGS="-L$OPENSSL_PREFIX/lib $LDFLAGS"

cmake -S "$SOURCE_DIR" -B "$TEMP_ROOT/build" \
  -DCMAKE_BUILD_TYPE=Release \
  -DCMAKE_INSTALL_PREFIX="$ENGINE_DIR" \
  -DCMAKE_INSTALL_BINDIR=uxplay \
  -DNO_MARCH_NATIVE=ON
cmake --build "$TEMP_ROOT/build" --parallel
cmake --install "$TEMP_ROOT/build"

if [ ! -x "$ENGINE_DIR/uxplay/uxplay" ]; then
  printf 'UxPlay was not installed at %s\n' "$ENGINE_DIR/uxplay/uxplay" >&2
  exit 1
fi
printf 'Installed UxPlay v%s at %s\n' "$UXPLAY_VERSION" "$ENGINE_DIR/uxplay/uxplay"
