# CardBili FFmpeg 修改

基于 FFmpeg 7.1.5

- 将 `fftools/ffmpeg.c` 中的 `main` 重命名为 `ffmpeg_main`
- 增加 `fftools/cardbili_args.c`，从文件描述符 4 接收 NUL 分隔的参数；显式命令行参数仍传给原入口
- 在 `fftools/Makefile` 中链接新增入口
- 在 TCP 读取处累计实际接收字节，通过 `-progress` 的 `cardbili_rx_bytes` 字段输出，覆盖 HTTP 与 HTTPS 媒体连接
- 在进度报告中输出实际输入视频的 `cardbili_source_fps`，供直播统计使用
- 在解复用后输出已读取音视频包的时间戳与时长，供 Buffer 计算连续可播放范围
- 将输入 `thread_queue_size` 传给解码器的压缩包队列，点播使用 512 包预读，直播保持默认容量
- 使用 Dockerfile 中的精简静态配置，保留 H.264 / AAC、HTTP / HTTPS、音视频缩放与变速所需功能

项目内的 `args.c` 使用 MIT 许可证，FFmpeg 组合程序按其 LGPLv3 条款分发
