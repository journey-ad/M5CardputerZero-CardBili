# 第三方组件与分发说明

CardBili 原创代码、文档、脚本与 skill 使用根目录的 MIT 许可证。下列第三方组件各自保留原许可证与版权声明

| 组件 | 许可证与来源 | 分发方式 |
| --- | --- | --- |
| Slint 1.8 | GPL-3.0-only、Slint Royalty-free 2.0 或商业许可证，[上游](https://github.com/slint-ui/slint/tree/v1.8.0/LICENSES) | 默认设备构建选择 GPL-3.0-only |
| FFmpeg 7.1.5 | 当前构建启用 `--enable-version3`，使用 LGPLv3，[上游](https://ffmpeg.org/legal.html) | 独立可执行文件，附修改源码与许可证 |
| OpenSSL | Apache-2.0，[上游](https://www.openssl.org/source/license.html) | 静态链接到 FFmpeg，附许可证 |
| Fusion Pixel Font | SIL OFL-1.1，[上游](https://github.com/TakWolf/fusion-pixel-font) | 保留 `cardbili/fonts/OFL-fusion-pixel-font.txt`，包括 8px 子集与 10px / 12px 字体 |
| Mona Emoji | SIL OFL-1.1，[2026.10.09 发布包](https://github.com/MonadABXY/mona-font/releases/tag/2026.10.09) | 使用官方原版 Mona12Emoji 与 Mona12ColorEmoji，构建时生成 12px 字形，保留 `cardbili/fonts/OFL-mona-font.txt` |
| Pixelarticons | MIT，[上游](https://github.com/halfmage/pixelarticons) | 保留 `cardbili/assets/pixelarticons/LICENSE` |
| Rust 依赖 | 各 crate 的许可证 | 源码包包含 `Cargo.lock`、vendored 源码与许可证，设备包包含许可证副本 |
| Rust 标准库与 GCC 运行库 | 各组件许可证，GCC 使用 Runtime Library Exception | 设备包保留 Rust 工具链版权与运行库例外条款 |
| voice-asr SDK 0.2.0 | [发布包与源码](https://github.com/journey-ad/M5CardputerZero-VoiceIme/releases/tag/v0.2.0) | 原样分发发布包中的 ARM64 `libvoice_asr.so`，通过 Rust C ABI 调用；系统 glibc、OpenSSL、Opus 由包依赖提供 |

## MIT 源码与设备二进制

MIT 允许将项目代码与 GPLv3 组件组合分发。默认构建静态链接 Slint，因此发布的 CardBili 组合可执行文件按 GPL-3.0-only 分发；项目原创源码继续保留 MIT 许可

Slint Royalty-free 2.0 适用于其定义的桌面、移动和 Web 应用，并明确排除嵌入式系统。本项目的设备构建采用 GPLv3 路径；采用其他 Slint 授权时，应按对应条款另行确认分发条件

源码附件包含构建脚本、字体资源、锁定的 Rust 依赖，以及 FFmpeg 修改源码。设备用户可以通过 APT 安装自己重新构建的 Debian 包，项目没有签名锁定步骤

## FFmpeg 修改

`cardbili/ffmpeg/args.c` 增加从文件描述符 4 读取 NUL 分隔参数的入口，原 `main` 改名为 `ffmpeg_main`，构建配置链接该入口。这样应用可通过管道向解码器传递请求头与媒体地址

FFmpeg 通过独立进程与 CardBili 通信。当前构建未启用 GPL 或 nonfree 组件；`--enable-version3` 用于所选 OpenSSL 组合。修改后的完整 FFmpeg 源码及原许可证随源码包提供，构建版本信息位于 `ffmpeg-build-info.txt`

## 内容与接口参考

Bilibili 接口行为参考了 [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus) 等社区资料。网络返回的视频、封面、头像、弹幕与账号内容保留其原有权利归属

`docs/screenshots/` 展示应用界面，视频、封面、评论与头像等内容保留原有权利归属
