# Rust 与 Slint 设备构建

CardBili 目标为 `aarch64-unknown-linux-musl`，使用 Slint 1.8 软件渲染器，输出静态 ELF

## CardBili 构建入口

在 CardBili 仓库根执行：

```sh
rustup target add aarch64-unknown-linux-musl
bash scripts/build-device.sh
python3 scripts/package-release.py
python3 scripts/check-release.py --deb dist/cardbili_0.1.0_arm64.deb
```

依赖 Rust、LLVM、Docker、Python 3.11 或更新版本及 Pillow；Rust 许可证由 `rust-docs` 组件提供。macOS 使用 Homebrew LLVM，Linux 使用系统 clang / llvm-ar；脚本根据 `rustc --print sysroot` 与宿主三元组定位 rust-lld。交叉编译 C 依赖时使用支持目标平台的 LLVM

musl 将 dl 符号放在 libc 中，脚本使用 llvm-ar 生成空的 libdl.a 满足依赖链接参数。构建产物位于 `cardbili/target/` 与 `dist/`，这些目录由 Git 忽略

打包输出 `.deb`、对应源码包和 SHA-256 校验文件。`dpkg-deb` 可在本机或 Docker 中运行；商店元数据维护在根目录 `app-builder.json`

## 环境配置

macOS 安装 LLVM、Python 和 FFmpeg，并启动 Docker：

```sh
brew install llvm python ffmpeg
python3 -m venv .secrets/venv
.secrets/venv/bin/pip install Pillow
rustup target add aarch64-unknown-linux-musl
rustup component add rust-docs
```

使用该虚拟环境时，用 `.secrets/venv/bin/python` 执行打包脚本

Ubuntu 24.04：

```sh
sudo apt-get update
sudo apt-get install -y clang llvm python3-pil dpkg pkg-config libxkbcommon-dev libwayland-dev libx11-dev libxcursor-dev libxrandr-dev libxi-dev
rustup target add aarch64-unknown-linux-musl
rustup component add rust-docs
```

x86_64 主机运行 ARM64 FFmpeg 构建容器时需要 Docker ARM64 模拟支持，CI 使用 ARM64 runner

## 当前字体与图标方案

- Cargo：关闭 Slint 默认 features，启用 `std`、`renderer-software`、`compat-1-2`
- `build.rs`：`EmbedForSoftwareRenderer` 用于图标等静态资源
- `build/ui_fonts.rs`：从仓库中的 Fusion Pixel Font 生成完整位图字形
- `src/fonts.rs`：在创建界面前注册位图字体，覆盖动态中文文本
- `MinimalSoftwareWindow`：使用 `ReusedBuffer`，启动时 `show()`，保持绘图缓冲跨帧存活

其他 Slint 项目按自己的字体加载方式处理动态字形，检查构建时子集是否包含运行时内容

## 验证

使用 `file` 或 `readelf -h` 确认 ELF 为 AArch64。桌面预览使用 `cargo run --locked --features desktop`，实体显示与输入在设备验证

公开设备二进制时核对 Slint 许可证。CardBili 源码为 MIT，默认含 Slint 的设备组合程序按 GPLv3 分发，并附对应源码

## 源码附件

源码包包含当前工作区的项目文件、Cargo vendored 依赖与修改后的 FFmpeg 源码，发布前提交全部版本内容

`cardbili/.cargo/config.toml` 指向附件中的 `vendor/`。FFmpeg 源码位于 `ffmpeg-source.tar.xz`，构建选项见 `cardbili/ffmpeg/Dockerfile`，修改记录见 `cardbili/ffmpeg/CHANGES.md`
