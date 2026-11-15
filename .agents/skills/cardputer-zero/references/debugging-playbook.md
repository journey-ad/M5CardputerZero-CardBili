# 调试手册

## 一次真机调试

1. 读取设备、APPLaunch 与当前应用状态
2. 停止 APPLaunch，记录自己启动的应用 PID
3. 在设备运行应用，抓取帧缓冲并检查对应日志
4. 结束自己启动的进程，恢复 APPLaunch

在设备 shell 使用 `trap` 可确保退出时恢复启动器：

```sh
XDG_RUNTIME_DIR=/run/user/$(id -u)
export XDG_RUNTIME_DIR
export DBUS_SESSION_BUS_ADDRESS=unix:path=$XDG_RUNTIME_DIR/bus
trap 'systemctl --user start APPLaunch.service' EXIT
systemctl --user stop APPLaunch.service
BILI_SECONDS=15 /usr/share/APPLaunch/bin/M5CardputerZero-CardBili
```

使用精确 PID 管理调试进程。宽泛的 `pkill -f` 可能匹配执行命令的 SSH shell

## 按症状排查

| 症状 | 检查方向 |
| --- | --- |
| 静态页面黑屏 | 窗口已 show、根组件尺寸正确、绘图缓冲复用、写入正确的帧缓冲 |
| 动态中文缺字 | 完整字体缓存已生成并在创建 UI 前注册 |
| 实体按键无响应 | 当前键盘节点、设备权限、真实 keycode、松开与重复事件 |
| 视频无音频 | `pacat`、用户音频服务、PCM 格式、文件描述符 3 |
| 视频无法启动 | 配套解码器路径、请求头、网络响应、账号权限与媒体 URL 有效期 |
| 安装后无图标 | desktop 内容、Exec 路径、图标位置、APPLaunch 用户服务日志 |

CardBili 桌面预览可使用 `BILI_KEYS`、`BILI_HOLD` 和 `BILI_SECONDS`；真机交互仍以实际键盘事件为准

日志与截图先写入 Git 忽略目录，分享前检查账号信息、签名 URL、Cookie 和二维码
