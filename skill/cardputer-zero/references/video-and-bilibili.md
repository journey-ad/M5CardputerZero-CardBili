# 音视频与 Bilibili

## CardBili 播放链路

当前实现使用一个精简 FFmpeg 进程处理视频流与音频流：

- 视频输出 RGB24，320 × 170，每帧 163200 字节，经软件渲染写入 RGB565 屏幕
- 音频输出 44.1 kHz、双声道、s16le PCM，通过文件描述符 3 交给 `pacat`
- 音频实际播放时间用于画面同步，`setpts` 与 `atempo` 处理倍速
- `scale` 使用 `fast_bilinear`，`pad` 保持画面比例
- 定制入口从文件描述符 4 接收 NUL 分隔的参数，媒体 URL 与请求头通过管道传递

Linux 的 `cardbili-ffmpeg` 与主程序放在同一目录。macOS 桌面预览使用 PATH 中的 FFmpeg，采用系统音频输出

## 网络接口

接口行为可能随服务端变化，先依据当前源码定位请求。视频请求需要正确的 User-Agent、Referer 和 Origin；登录请求使用当前会话 Cookie，媒体 URL 可能含短期签名

WBI 签名从导航接口取得图像文件名并生成 mixin key，对排序后的参数计算签名。应用请求实现位于 `src/api/wbi.rs` 和 `src/api/playback.rs`

TV/HD 扫码登录使用 `auth_code` 与 `poll` 接口，程序内置与 PiliPlus 一致的 Android HD 客户端通用签名参数，用户通过 B 站 App 扫码完成登录。实现位于 `src/api/auth.rs`，参考 [PiliPlus](https://github.com/bggRGjQaUbCoE/PiliPlus/blob/217fb54f51f6764176319bbdedab2a7522d75401/lib/http/login.dart)

## 登录数据

登录态写入 XDG 配置目录下的 `cardbili/auth.dat`，主密钥写入 XDG 状态目录下的 `cardbili/master.key`。当前格式为 XChaCha20-Poly1305 认证加密，Unix 权限为 `0600`

收集诊断信息时隐藏 Cookie、令牌、扫码内容和媒体 URL 查询参数。公开视频截图与日志使用匿名数据或经过检查的内容
