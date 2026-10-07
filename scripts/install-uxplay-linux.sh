#!/usr/bin/env bash
set -eo pipefail

if [ "$(uname -s)" != "Linux" ]; then
  printf '%s\n' 'This installer only runs on Linux.' >&2
  exit 2
fi
if [ "$(uname -m)" != "x86_64" ]; then
  printf 'Unsupported Linux architecture: %s\n' "$(uname -m)" >&2
  exit 2
fi

INSTALL_FROM_SOURCE=0
if [ "$1" = "--build-from-source" ]; then
  INSTALL_FROM_SOURCE=1
elif [ -n "$1" ]; then
  printf 'Unknown argument: %s\nUsage: %s [--build-from-source]\n' "$1" "$0" >&2
  exit 2
fi

if [ "$INSTALL_FROM_SOURCE" -eq 0 ]; then
  if [ -r /etc/os-release ]; then
    . /etc/os-release
  fi
  case "$ID:$ID_LIKE" in
    ubuntu:*|debian:*|*:debian*)
      sudo apt-get update
      sudo apt-get install -y uxplay
      ;;
    fedora:*|rhel:*|centos:*|*:fedora*|*:rhel*)
      sudo dnf install -y uxplay
      ;;
    arch:*|manjaro:*|*:arch*)
      if command -v paru >/dev/null 2>&1; then
        paru -S --needed uxplay
      elif command -v yay >/dev/null 2>&1; then
        yay -S --needed uxplay
      else
        printf '%s\n' 'UxPlay is available from the Arch User Repository. Install paru or yay, then run this script again.' >&2
        exit 2
      fi
      ;;
    *)
      printf 'No supported package command was detected for %s. Install UxPlay with your distribution package manager or use --build-from-source.\n' "$ID" >&2
      exit 2
      ;;
  esac
  printf '%s\n' 'UxPlay package installation finished. Restart the AirPlay receiver to detect it.'
  exit 0
fi

if [ -r /etc/os-release ]; then
  . /etc/os-release
fi
case "$ID:$ID_LIKE" in
  ubuntu:*|debian:*|*:debian*)
    sudo apt-get update
    sudo apt-get install -y \
      build-essential cmake pkg-config \
      libssl-dev libplist-dev libavahi-compat-libdns_sd-dev \
      libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libx11-dev \
      gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
      gstreamer1.0-plugins-bad gstreamer1.0-libav
    ;;
  fedora:*|rhel:*|centos:*|*:fedora*|*:rhel*)
    sudo dnf install -y \
      cmake gcc gcc-c++ make pkgconf-pkg-config \
      openssl-devel libplist-devel avahi-compat-libdns_sd-devel \
      gstreamer1-devel gstreamer1-plugins-base-devel \
      gstreamer1-plugins-good gstreamer1-plugins-bad-free gstreamer1-libav
    ;;
  arch:*|manjaro:*|*:arch*)
    sudo pacman -Syu --needed --noconfirm \
      base-devel cmake pkgconf openssl libplist avahi \
      gstreamer gst-plugins-base gst-plugins-good gst-plugins-bad gst-libav
    ;;
  *)
    printf 'Source build dependencies are not mapped for %s.\n' "$ID" >&2
    exit 2
    ;;
esac

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
ACTUAL_SHA256="$(sha256sum "$ARCHIVE" | awk '{ print $1 }')"
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

DATA_ROOT="$XDG_DATA_HOME"
if [ -z "$DATA_ROOT" ]; then DATA_ROOT="$HOME/.local/share"; fi
ENGINE_DIR="$DATA_ROOT/NeoAirPlay/engine"
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
