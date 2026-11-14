#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)

NAME=${1:?usage: package-desktop.sh <name> <target> <binary>}
TARGET=${2:?}
BINARY=${3:?}

mkdir -p "$ROOT/dist"
OUT="$ROOT/dist/cardbili-desktop-$NAME"
cp "$ROOT/cardbili/target/$TARGET/release/$BINARY" "$OUT"
chmod +x "$OUT"
