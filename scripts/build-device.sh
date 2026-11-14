#!/usr/bin/env bash
set -euo pipefail
ROOT=$(cd "$(dirname "$0")/.." && pwd)
HOST=$(rustc -vV | sed -n 's/^host: //p')
CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER="$(rustc --print sysroot)/lib/rustlib/$HOST/bin/rust-lld"
export CARGO_TARGET_AARCH64_UNKNOWN_LINUX_MUSL_LINKER
if [[ $(uname -s) == Darwin ]]; then
    LLVM_PREFIX=$(brew --prefix llvm)
    export PATH="$LLVM_PREFIX/bin:$PATH"
fi
export CC_aarch64_unknown_linux_musl=${CC_aarch64_unknown_linux_musl:-clang}
export AR_aarch64_unknown_linux_musl=${AR_aarch64_unknown_linux_musl:-llvm-ar}
RUST_FLAGS=(
    -C link-self-contained=yes
    --remap-path-prefix "$ROOT=/src/cardbili"
    --remap-path-prefix "$HOME=/build/home"
)
CARGO_ENCODED_RUSTFLAGS=$(IFS=$'\x1f'; printf '%s' "${RUST_FLAGS[*]}")
export CARGO_ENCODED_RUSTFLAGS
cd "$ROOT/cardbili"
# musl 在 libc 中提供 dl 符号，空归档满足依赖中的 -ldl 链接参数
SYS_LIB="$ROOT/cardbili/target/cardbili-syslib"
mkdir -p "$SYS_LIB"
"$AR_aarch64_unknown_linux_musl" crs "$SYS_LIB/libdl.a"
cargo rustc --locked --release --target aarch64-unknown-linux-musl -- -L "native=$SYS_LIB"
cd "$ROOT"
docker build --platform linux/arm64 --output type=local,dest=dist/ffmpeg cardbili/ffmpeg
