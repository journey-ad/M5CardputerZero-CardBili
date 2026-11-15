#!/usr/bin/env python3
"""生成设备发布包、对应源码与校验文件"""

import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tarfile
import tomllib

from PIL import Image

from debian import dpkg_deb

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
    manifest = json.loads((ROOT / "app-builder.json").read_text())
    if manifest["version"] != version:
        raise SystemExit("app-builder.json 与 Cargo.toml 版本不一致")
    tag = os.environ.get("RELEASE_TAG")
    if tag and tag != f"v{version}":
        raise SystemExit(f"标签 {tag} 与 Cargo 版本 {version} 不一致")

    binary = ROOT / "cardbili/target/aarch64-unknown-linux-musl/release/cardbili"
    ffmpeg = DIST / "ffmpeg"
    for required in (binary, ffmpeg / "cardbili-ffmpeg", ffmpeg / "ffmpeg-source.tar.xz"):
        if not required.is_file():
            raise SystemExit(f"缺少构建产物：{required}")

    package = DIST / f"cardbili_{version}_arm64"
    source = DIST / f"cardbili-{version}-source"
    for directory in (package, source):
        if directory.exists():
            shutil.rmtree(directory)
        directory.mkdir(parents=True)

    app = package / "usr/share/APPLaunch"
    documentation = package / "usr/share/doc/cardbili"
    copy(binary, app / "bin" / manifest["bin_name"])
    copy(ffmpeg / "cardbili-ffmpeg", app / "bin/cardbili-ffmpeg")
    copy(ROOT / "docs/cardbili.desktop", app / "applications/cardbili.desktop")
    copy(ROOT / "THIRD_PARTY_NOTICES.md", documentation / "THIRD_PARTY_NOTICES.md")
    copy(ROOT / "LICENSE", documentation / "licenses/CardBili-MIT.txt")
    copy(ROOT / "cardbili/fonts/OFL-fusion-pixel-font.txt", documentation / "licenses/fonts/OFL.txt")
    copy(ROOT / "cardbili/fonts/OFL-mona-font.txt", documentation / "licenses/fonts/OFL-mona-font.txt")
    copy(ROOT / "cardbili/assets/pixelarticons/LICENSE", documentation / "licenses/pixelarticons/LICENSE")
    shutil.copytree(ffmpeg / "licenses", documentation / "licenses", dirs_exist_ok=True)
    (app / "share/images").mkdir(parents=True)
    with Image.open(ROOT / "cardbili/assets/store-icon.png") as icon:
        for size in (100, 80):
            icon.convert("RGBA").resize((size, size), Image.Resampling.LANCZOS).save(
                app / f"share/images/cardbili_{size}.png"
            )

    files = command("git", "ls-files", "--cached", "--others", "--exclude-standard", "-z").split("\0")
    for name in files:
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
                copy(path, documentation / "licenses/rust-crates" / crate.name / relative)

    sysroot = Path(command("rustc", "--print", "sysroot"))
    rust_docs = sysroot / "share/doc/rust"
    if not (rust_docs / "licenses").is_dir():
        raise SystemExit("缺少 Rust 许可证，请运行 rustup component add rust-docs")
    shutil.copytree(rust_docs / "licenses", documentation / "licenses/rust-toolchain")
    for path in rust_docs.glob("COPYRIGHT*"):
        copy(path, documentation / "licenses/rust-toolchain" / path.name)

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
    (documentation / "BUILD-INFO.txt").write_text(build_info)
    repository = manifest["store"]["source_repo"]
    (documentation / "SOURCE.txt").write_text(
        f"CardBili {version}\n"
        "CardBili binary: GPL-3.0-only; original project source: MIT\n"
        "Bundled FFmpeg: LGPL-3.0-or-later\n\n"
        f"Corresponding source: cardbili-{version}-source.tar.gz\n"
        f"{repository}/releases/download/v{version}/cardbili-{version}-source.tar.gz\n"
        "Build and installation instructions: README.md\n"
    )
    installed_size = (sum(p.stat().st_size for p in package.rglob("*") if p.is_file()) + 1023) // 1024
    control = package / "DEBIAN"
    control.mkdir()
    (control / "control").write_text(
        f"Package: {manifest['package_name']}\n"
        f"Version: {version}\n"
        "Architecture: arm64\n"
        "Maintainer: journey-ad <journey-ad@users.noreply.github.com>\n"
        "Section: video\n"
        "Priority: optional\n"
        "Depends: ca-certificates, pulseaudio-utils\n"
        "Recommends: pipewire-pulse | pulseaudio\n"
        f"Installed-Size: {installed_size}\n"
        f"Homepage: {repository}\n"
        f"Description: {manifest['description']}\n"
        " Browse videos, play H.264/AAC media and display danmaku\n"
        " on the 320x170 screen through APPLaunch\n"
    )
    for path in package.rglob("*"):
        path.chmod(0o755 if path.is_dir() else 0o644)
    for path in (app / "bin").iterdir():
        path.chmod(0o755)
    deb = DIST / f"cardbili_{version}_arm64.deb"
    print(dpkg_deb("--root-owner-group", "--uniform-compression", "-Zxz", "--build", package, deb).strip())
    outputs = [deb, archive(source)]
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
