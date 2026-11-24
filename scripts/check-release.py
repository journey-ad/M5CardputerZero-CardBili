#!/usr/bin/env python3
"""校验商店元数据、Debian 安装包和对应源码"""

import argparse
import configparser
import hashlib
import io
import json
from pathlib import Path
import runpy
import shutil
import struct
import tarfile
import tempfile
import tomllib
import urllib.request

from debian import dpkg_deb

ROOT = Path(__file__).resolve().parent.parent
DIST = ROOT / "dist"
POLICY_COMMIT = "429702b0b56e0151c57fc381ff759fe4749e3287"
POLICY_HASHES = {
    "store_meta_policy.py": "748c9a2c5cb204b5e6ad1d19a44f82d61acf3e66f034ae73fc2fbe1cfdb74505",
    "install_path_policy.py": "3cedaa05f85fe4a98c97f5d77221c53dd53d7772a775d2995fadb1f7c602b4c5",
}


def require(condition, message):
    if not condition:
        raise SystemExit(message)


def policy(name):
    path = DIST / "store-policy" / name
    if not path.is_file():
        url = f"https://raw.githubusercontent.com/CardputerZero/packages/{POLICY_COMMIT}/.github/scripts/{name}"
        with urllib.request.urlopen(url, timeout=30) as response:
            data = response.read()
        require(hashlib.sha256(data).hexdigest() == POLICY_HASHES[name], f"官方规则校验失败：{name}")
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(data)
    require(hashlib.sha256(path.read_bytes()).hexdigest() == POLICY_HASHES[name], f"官方规则缓存校验失败：{path}")
    return runpy.run_path(str(path))


def check_metadata(manifest):
    cargo = tomllib.loads((ROOT / "cardbili/Cargo.toml").read_text())["package"]
    lock = tomllib.loads((ROOT / "cardbili/Cargo.lock").read_text())
    locked = next(package for package in lock["package"] if package["name"] == cargo["name"])
    require(manifest["package_name"] == cargo["name"] == "cardbili", "包名应为 cardbili")
    require(manifest["version"] == cargo["version"] == locked["version"], "manifest、Cargo.toml 与 Cargo.lock 版本不一致")
    store = manifest["store"]
    rules = policy("store_meta_policy.py")
    with tempfile.TemporaryDirectory(prefix="store-check-", dir=DIST) as temporary:
        directory = Path(temporary)
        meta = dict(store, title=manifest["app_name"])
        meta["icon"] = Path(store["icon"]).name
        shutil.copyfile(ROOT / store["icon"], directory / meta["icon"])
        (directory / "screenshots").mkdir()
        meta["screenshots"] = []
        for screenshot in store["screenshots"]:
            relative = f"screenshots/{Path(screenshot).name}"
            shutil.copyfile(ROOT / screenshot, directory / relative)
            meta["screenshots"].append(relative)
        (directory / "meta.json").write_text(json.dumps(meta, ensure_ascii=False))
        errors, warnings = rules["check_meta"](manifest["package_name"], directory, "-")
        for warning in warnings:
            print(f"提示：{warning}")
        require(not errors, "\n".join(errors))
    print("商店元数据、图标、截图和版本校验通过")
    print("应用名、分享码和发布账号的归属由商店提交检查确认")
    return rules


def check_package(deb, manifest, metadata_rules):
    require(deb.is_file(), f"安装包不存在：{deb}")
    require(deb.stat().st_size < 100 * 1024 * 1024, "安装包超过商店的 100 MiB 限制")
    fields = {}
    for line in dpkg_deb("--field", deb).splitlines():
        if line and not line[0].isspace() and ":" in line:
            key, value = line.split(":", 1)
            fields[key] = value.strip()
    for field, value in {"Package": manifest["package_name"], "Version": manifest["version"], "Architecture": "arm64"}.items():
        require(fields.get(field) == value, f"Debian {field} 应为 {value}")
    require(deb.name == f"cardbili_{manifest['version']}_arm64.deb", "安装包文件名应与包名、版本和架构一致")
    require(fields.get("Maintainer"), "缺少 Debian Maintainer")
    require(not metadata_rules["check_maintainer"](fields["Maintainer"]), "Debian Maintainer 未通过官方检查")
    dependencies = {item.strip() for item in fields.get("Depends", "").split(",")}
    require({"ca-certificates", "pulseaudio-utils", "alsa-utils", "libopus0", "libssl3t64"} <= dependencies, "缺少证书、音频或语音 SDK 依赖")
    path_rules = policy("install_path_policy.py")
    with tarfile.open(fileobj=io.BytesIO(dpkg_deb("--fsys-tarfile", deb, text=False))) as payload:
        files = {}
        for member in payload.getmembers():
            name = member.name.removeprefix("./")
            require(member.uid == member.gid == 0, f"包内文件应属于 root:root：{name}")
            require(not member.mode & 0o6000, f"包内存在 setuid/setgid：{name}")
            if member.isdir():
                continue
            require(member.isfile(), f"包内存在特殊文件：{name}")
            issue = path_rules["install_path_issue"](name, manifest["package_name"])
            require(not issue, f"安装路径不符合官方要求：{name} {issue}")
            require(not name.endswith(".service"), f"CardBili 由 APPLaunch 启动，请移除附带服务：{name}")
            files[name] = member

        def content(name):
            require(name in files, f"安装包缺少文件：{name}")
            return payload.extractfile(files[name]).read()

        app_root = "usr/share/APPLaunch/"
        desktop = configparser.ConfigParser(interpolation=None)
        desktop.read_string(content(app_root + "applications/cardbili.desktop").decode())
        entry = desktop["Desktop Entry"]
        require(entry.get("Name") == manifest["app_name"] and entry.get("Terminal") == "false", "APPLaunch 名称或 Terminal 配置错误")
        require(entry.get("Exec") == f"/{app_root}bin/{manifest['bin_name']}", "APPLaunch Exec 与程序路径不一致")
        icon = content(app_root + entry["Icon"])
        require(icon[:8] == b"\x89PNG\r\n\x1a\n", "启动器图标应为 PNG")
        width, height = struct.unpack(">II", icon[16:24])
        require(width == height and width > 0, "启动器图标应为正方形")
        for name in (manifest["bin_name"], "cardbili-ffmpeg"):
            path = app_root + "bin/" + name
            data = content(path)
            require(data[:6] == b"\x7fELF\x02\x01" and struct.unpack("<H", data[18:20])[0] == 183, f"程序应为 AArch64 ELF：{name}")
            require(files[path].mode == 0o755, f"程序权限应为 0755：{name}")
        voice = content(app_root + "bin/libvoice_asr.so")
        require(voice[:6] == b"\x7fELF\x02\x01" and struct.unpack("<H", voice[18:20])[0] == 183, "语音 SDK 应为 AArch64 ELF")
        content("usr/share/doc/cardbili/THIRD_PARTY_NOTICES.md")
        content("usr/share/doc/cardbili/SOURCE.txt")
    print("Debian 架构、依赖、安装路径、权限和 APPLaunch 入口校验通过")

    source = deb.parent / f"cardbili-{manifest['version']}-source.tar.gz"
    require(source.is_file(), f"缺少对应源码附件：{source}")
    with tarfile.open(source, "r:gz") as bundle:
        names = set(bundle.getnames())
        prefix = f"cardbili-{manifest['version']}-source/"
        for name in ("app-builder.json", "LICENSE", "cardbili/Cargo.lock", "vendor", "ffmpeg-source.tar.xz", "scripts/package-release.py"):
            require(prefix + name in names, f"源码附件缺少：{name}")
    checksums = dict(line.split("  ", 1)[::-1] for line in (deb.parent / "SHA256SUMS").read_text().splitlines())
    for path in (deb, source):
        with path.open("rb") as content_file:
            digest = hashlib.file_digest(content_file, "sha256").hexdigest()
        require(checksums.get(path.name) == digest, f"SHA-256 校验失败：{path.name}")
    print("对应源码附件与 SHA-256 校验通过")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--deb", type=Path, help="同时校验指定安装包及其配套源码和 SHA256SUMS")
    args = parser.parse_args()
    DIST.mkdir(exist_ok=True)
    manifest = json.loads((ROOT / "app-builder.json").read_text())
    rules = check_metadata(manifest)
    if args.deb:
        check_package(args.deb.resolve(), manifest, rules)


if __name__ == "__main__":
    main()
