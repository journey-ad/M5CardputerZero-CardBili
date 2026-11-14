# Mac 交叉编译 Rust + Slint → Cardputer Zero（aarch64 musl）

目标：在 macOS（Apple Silicon）编译静态 `aarch64-unknown-linux-musl` ELF，设备只放产物（无需装工具链）。

## 一次性工具链准备
- 需要 rustup/cargo（如 `~/.cargo/bin`）。为设备加 musl target：
  ```sh
  rustup target add aarch64-unknown-linux-musl   # 写 ~/.rustup，沙箱下需 danger-full-access
  ```
- musl 目标自包含（Scrt1.o/crt*.o/libc.a/libunwind.a），可静态链接，规避设备 glibc 2.41 版本问题。
- rust-lld 路径：`/home/developer/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/rust-lld`

## 工程结构
```
cardbili/
  Cargo.toml  build.rs  .cargo/config.toml
  ui/app.slint
  src/{main.rs,net.rs,bili.rs,player.rs}
  fonts/zpix.ttf          # 或 DroidSansFallbackFull.ttf
  syslib/libdl.a          # 占位桩
```

### Cargo.toml 关键
```toml
[dependencies]
slint = { version = "~1.8", default-features = false,
          features = ["std","renderer-software","software-renderer-systemfonts","compat-1-2"] }
# 其它：memmap2、serde、serde_json、md-5、qrcodegen、image(jpeg,png)
[build-dependencies]
slint-build = "~1.8"
[profile.release]
opt-level = "z"; lto = true; codegen-units = 1; strip = true
```
- **必须锁定 `~1.8`**：`slint="1"` 会解析到 1.18，其 fontique→`yeslogic-fontconfig-sys` 交叉编译要 pkg-config sysroot，会失败。1.8 用 fontdue，免 fontconfig。
- `compat-1-2` 不加会 compile_error。

### build.rs（关键：完整嵌入字体，勿子集化）
```rust
fn main() {
    slint_build::compile_with_config(
        "ui/app.slint",
        slint_build::CompilerConfiguration::new()
            .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles), // 不是 EmbedForSoftwareRenderer
    ).unwrap();
}
```
- `EmbedForSoftwareRenderer`（=EmbedTextures）会**只嵌入被 .slint 字面量引用到的字形**，运行时动态字符串（列表标题等）的中文全缺。必须用 `EmbedFiles` 完整嵌入，再配合 feature `software-renderer-systemfonts` 才能在运行时注册/使用这些字形。

### .cargo/config.toml
```toml
[target.aarch64-unknown-linux-musl]
linker = "/home/developer/.rustup/toolchains/stable-aarch64-apple-darwin/lib/rustlib/aarch64-apple-darwin/bin/rust-lld"
rustflags = ["-C","link-self-contained=yes","-L","native=/ABS/PATH/cardbili/syslib"]
```

### libdl.a 桩（musl 无独立 libdl）
链接会报 `unable to find library -ldl`。用占位成员造一个：
```sh
printf dummy > syslib/_dummy.txt
/usr/bin/ar crs syslib/libdl.a syslib/_dummy.txt   # Apple ar 不能建空归档
```
链接器只告警 `archive member '_dummy.txt' is neither ET_REL nor LLVM bitcode`，可通过。

## .slint / Rust 关键用法
- `.slint` 顶部：`import "../fonts/zpix.ttf";` + `default-font-family: "Zpix";`
- 小屏建议用 **zpix 像素字体**（`https://github.com/SolidZORO/zpix-pixel-font` v3.3.0，family "Zpix"）。装 Mac `~/Library/Fonts/zpix.ttf`（slint-build 需在 Mac 解析该 family）和设备 `/usr/share/fonts/truetype/zpix/zpix.ttf`。
- 启动 stderr 打印 `Error opening libfontconfig.so.1: Dynamic loading not supported`（musl 静态不支持 dlopen）——**无害**，回退内嵌字体。
- Slint 不支持 `%`，用 `Math.mod(i, cols)`；网格尺寸用传入的 `length` 属性（Rust `ui.set_cell_w(...)`）。
- Rust 平台适配：
  ```rust
  let window = slint::platform::MinimalSoftwareWindow::new(
      slint::platform::software_renderer::RepaintBufferType::ReusedBuffer); // 已返回 Rc，勿再 Rc::new
  window.set_size(slint::PhysicalSize::new(320,170));
  slint::platform::set_platform(Box::new(MyPlatform{window:window.clone()})).unwrap();
  let ui = AppWindow::new().unwrap();
  ui.show().unwrap();                 // 关键：不 show 则 window_item 为空，render 返回空 -> 全黑
  ```
- 渲染循环（关键：常驻 buffer + 仅重绘时写屏）：
  ```rust
  let mut buffer = vec![slint::platform::software_renderer::Rgb565Pixel(0); FB_BYTES/2];
  // 每帧：
  if window.draw_if_needed(|r| { r.render(&mut buffer, WIDTH as usize); }) {
      fbmap.copy_from_slice(bytemuck_cast_bytes(&buffer)); // 写 /dev/fb0
  }
  ```
  `Rgb565Pixel` 在 `slint::platform::software_renderer::Rgb565Pixel`（顶层未导出）。
- Slint 1.8 **没有** `Image::from_rgb565`；视频帧用 ffmpeg `-pix_fmt rgb24` + `SharedPixelBuffer::<slint::Rgb8Pixel>::new(320,170)` + `Image::from_rgb8`。读帧 163200B，写屏 108800B。
- QR：`qrcodegen` 生成 `QrCode`，逐 module 放大画到 `SharedPixelBuffer<Rgb8Pixel>`，`Image::from_rgb8`。

## 构建 & 部署
```sh
export PATH=$HOME/.cargo/bin:$PATH
export http_proxy=https_proxy=http://127.0.0.1:7890
cd /home/developer/src/cardbili-dev/cardbili
cargo build --release --target aarch64-unknown-linux-musl
# 产物 target/aarch64-unknown-linux-musl/release/cardbili （静态 musl ELF）
```
部署：一键 `skill/cardputer-zero/scripts/deploy.sh`（scp 到 `/var/tmp/cardbili` → 远程 `sudo install` 到 `/usr/share/APPLaunch/bin/M5CardputerZero-CardBili` → 用户级 `systemctl --user restart APPLaunch.service` → md5 校验）。
含 zpix 嵌入后产物约 6.6MB；纯 Slint 无大字体约 1MB。
