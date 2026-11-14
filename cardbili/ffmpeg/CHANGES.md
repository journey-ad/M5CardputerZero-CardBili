# CardBili FFmpeg 修改

基于 FFmpeg 7.1.5

- 将 `fftools/ffmpeg.c` 中的 `main` 重命名为 `ffmpeg_main`
- 增加 `fftools/cardbili_args.c`，从文件描述符 4 接收 NUL 分隔的参数；显式命令行参数仍传给原入口
- 在 `fftools/Makefile` 中链接新增入口
- 使用 Dockerfile 中的精简静态配置，保留 H.264 / AAC、HTTP / HTTPS、音视频缩放与变速所需功能

项目内的 `args.c` 使用 MIT 许可证，FFmpeg 组合程序按其 LGPLv3 条款分发
