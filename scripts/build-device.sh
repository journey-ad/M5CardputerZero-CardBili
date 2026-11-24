#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
docker build --platform linux/arm64 -t cardbili-device-builder -f "$ROOT/scripts/device.Dockerfile" "$ROOT/scripts"
docker run --rm --platform linux/arm64 \
    -v "$ROOT:/work" -v cardbili-device-cargo:/usr/local/cargo \
    cardbili-device-builder bash -c '
        cd cardbili
        RUSTFLAGS="--remap-path-prefix /work=/src/cardbili" cargo build --locked --release --target aarch64-unknown-linux-gnu
    '
if [[ ${SKIP_FFMPEG:-0} != 1 ]]; then
    docker build --platform linux/arm64 --output "type=local,dest=$ROOT/dist/ffmpeg" "$ROOT/cardbili/ffmpeg"
fi
