#!/usr/bin/env python3
"""生成设备发布包、对应源码与校验文件"""

import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tomllib

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
DIST = ROOT / "dist"


def command(*args, cwd=ROOT):
    return subprocess.check_output(args, cwd=cwd, text=True).strip()


def copy(source, destination):
    destination.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy2(source, destination)


def archive(directory):
    output = DIST / f"{directory.name}.tar.gz"
    with tarfile.open(output, "w:gz") as bundle:
        bundle.add(directory, arcname=directory.name)
    return output


def main():
    version = tomllib.loads((ROOT / "cardbili/Cargo.toml").read_text())["package"]["version"]
    tag = os.environ.get("RELEASE_TAG")
    if tag and tag != f"v{version}":
        raise SystemExit(f"标签 {tag} 与 Cargo 版本 {version} 不一致")

    binary = ROOT / "cardbili/target/aarch64-unknown-linux-musl/release/cardbili"
    ffmpeg = DIST / "ffmpeg"
    for required in (binary, ffmpeg / "cardbili-ffmpeg", ffmpeg / "ffmpeg-source.tar.xz"):
        if not required.is_file():
            raise SystemExit(f"缺少构建产物：{required}")

    package = DIST / f"cardbili-{version}-aarch64-linux-musl"
    source = DIST / f"cardbili-{version}-source"
    for directory in (package, source):
        if directory.exists():
            shutil.rmtree(directory)
        directory.mkdir(parents=True)

    copy(binary, package / "M5CardputerZero-CardBili")
    copy(ffmpeg / "cardbili-ffmpeg", package / "cardbili-ffmpeg")
    copy(ROOT / "docs/cardbili.desktop", package / "cardbili.desktop")
    copy(ROOT / "scripts/install.sh", package / "install.sh")
    copy(ROOT / ".secrets/bilibili.env.example", package / "bilibili.env.example")
    copy(ROOT / "THIRD_PARTY_NOTICES.md", package / "THIRD_PARTY_NOTICES.md")
    copy(ROOT / "LICENSE", package / "licenses/CardBili-MIT.txt")
    copy(ROOT / "cardbili/fonts/OFL-fusion-pixel-font.txt", package / "licenses/fonts/OFL.txt")
    copy(ROOT / "cardbili/assets/pixelarticons/LICENSE", package / "licenses/pixelarticons/LICENSE")
    shutil.copytree(ffmpeg / "licenses", package / "licenses", dirs_exist_ok=True)
    with Image.open(ROOT / "cardbili/assets/app-icon.png") as icon:
        for size in (100, 80):
            icon.convert("RGBA").resize((size, size), Image.Resampling.LANCZOS).save(
                package / f"cardbili_{size}.png"
            )

    tracked = command("git", "ls-files", "-z").split("\0")
    for name in tracked:
        path = Path(name)
        if not name or not (ROOT / path).is_file():
            continue
        if path.parts[0] in {".secrets", "dist"}:
            if not name.endswith(".env.example"):
                continue
        if name.startswith("cardbili/target/"):
            continue
        copy(ROOT / path, source / path)

    vendor_config = command(
        "cargo", "vendor", "--locked", "--respect-source-config", str(source / "vendor"),
        cwd=ROOT / "cardbili",
    )
    vendor_config = vendor_config.replace(str(source / "vendor"), "../vendor")
    config = source / "cardbili/.cargo/config.toml"
    with config.open("a") as output:
        output.write("\n" + vendor_config + "\n")

    for crate in sorted((source / "vendor").iterdir()):
        for path in crate.rglob("*"):
            relative = path.relative_to(crate)
            if path.is_file() and any(
                part.upper().startswith(("LICENSE", "COPYING", "COPYRIGHT", "NOTICE", "AUTHORS"))
                for part in relative.parts
            ):
                copy(path, package / "licenses/rust-crates" / crate.name / relative)

    sysroot = Path(command("rustc", "--print", "sysroot"))
    rust_docs = sysroot / "share/doc/rust"
    if not (rust_docs / "licenses").is_dir():
        raise SystemExit("缺少 Rust 许可证，请运行 rustup component add rust-docs")
    shutil.copytree(rust_docs / "licenses", package / "licenses/rust-toolchain")
    for path in rust_docs.glob("COPYRIGHT*"):
        copy(path, package / "licenses/rust-toolchain" / path.name)

    copy(ffmpeg / "ffmpeg-source.tar.xz", source / "ffmpeg-source.tar.xz")
    copy(ffmpeg / "build-info.txt", source / "ffmpeg-build-info.txt")
    build_info = "\n".join([
        f"CardBili {version}",
        f"Source commit: {command('git', 'rev-parse', 'HEAD')}",
        f"Working tree modified: {bool(command('git', 'status', '--porcelain'))}",
        command("rustc", "-vV"),
        command("cargo", "--version"),
    ]) + "\n"
    (source / "BUILD-INFO.txt").write_text(build_info)
    (package / "BUILD-INFO.txt").write_text(build_info)
    outputs = [archive(package), archive(source)]
    checksums = []
    for path in outputs:
        with path.open("rb") as content:
            digest = hashlib.file_digest(content, "sha256").hexdigest()
        checksums.append(f"{digest}  {path.name}\n")
        print(path.name)
    (DIST / "SHA256SUMS").write_text("".join(checksums))
    print("SHA256SUMS")


if __name__ == "__main__":
    main()
