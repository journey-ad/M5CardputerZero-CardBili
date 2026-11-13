# 调试手册与踩坑

## 真机抓帧循环（核心）
```sh
export XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
systemctl --user stop APPLaunch.service; sleep 1
cd /tmp
APPLAUNCH_LINUX_FBDEV_DEVICE=/dev/fb0 BILI_SECONDS=9 ./app >/tmp/v.log 2>&1 &
P=$!; sleep 6
python3 /tmp/fbcap.py /tmp/shot.png
kill $P
systemctl --user start APPLaunch.service     # 一定要恢复 launcher
```
- 抓完放大看：`PIL resize((960,510), NEAREST)`。
- 测试可直接用环境变量控制：`BILI_SECONDS`(定时退出)、`BILI_DUMP=1`(打印数据)、`BILI_TAB`/`BILI_VIEW`/`BILI_HELP`(初始 UI 状态)、`BILI_AUTOPLAY=<bvid>`、`BILI_TEST_LOGIN=1`。

## 黑屏排查清单
1. 渲染循环新建全零 buffer 且无条件写 `/dev/fb0` → 改成常驻 buffer + 仅 `draw_if_needed` 为真时写。
2. 组件没 `ui.show()` → `window_item` 为空，`SoftwareRenderer::render` 直接返回空。加 `ui.show()`。
3. 根 `Rectangle` 没设 `width/height: parent.*` → 尺寸为 0。
4. 中文字形缺失 → 见下"字体子集化"。
5. 进程看似卡住：`ps` 显示 `D` 后转 `S`、`wchan=hrtimer_nanosleep` 是正常 sleep，不是死锁；查渲染/写屏逻辑。

## 字体子集化（中文/动态字符串不显示）
- 症状：`.slint` 字面量字符串中文正常；来自 property/model 的字符串只显示拉丁，CJK 全缺（占正确字宽=排版对但无墨）。底栏"20 条"只显示"20"。
- 根因：`EmbedResourcesKind::EmbedForSoftwareRenderer` 只嵌入被字面量引用到的字形。
- 修复：`EmbedFiles`（完整嵌入字体）+ Cargo feature `software-renderer-systemfonts`；字体改用同时覆盖中英的 zpix（或 Noto Sans CJK）。

## SSH 不能注入按键
launcher 只收 `/dev/input/event3` 真实事件，uinput 注入无效；按键/交互必须真机手按或用一个读同一键盘设备的小测试脚本 + FakeKeys 模拟（对渲染逻辑做单元验证，但真实按键语义仍需手测）。

## pkill -f 误杀自己
远程 bash 命令字符串里若含目标进程名（如 `M5CardputerZero-XXX`），`pkill -f <name>` 会把这条远程 shell 自己一起杀掉，导致后续 `systemctl start` 没执行、launcher 不恢复。**用精确 PID kill**，别用 `pkill -f`。

## 沙箱 / 权限
- 默认 workspace-write 会阻止写 `~/.cargo`、`~/.rustup`、`/opt/homebrew`、`~/.dsh/skills` 等（报 "Operation not permitted"）。需要时单次用 `sandbox_permissions=danger-full-access` 并说明理由。
- 规避：`export CARGO_HOME=<workspace>/.cargo-home` 把 cargo 缓存放工作区内。

## 代理 / GitHub
- Mac 代理 `http://127.0.0.1:7890`（`http_proxy`/`https_proxy`）。
- `github.com`、`api.github.com` 可经代理；**`raw.githubusercontent.com` 被阻断**（CONNECT 后 TLS SSL_ERROR_SYSCALL）→ 用 `api.github.com/repos/<owner>/<repo>/contents/<path>`（返回 base64）抓源码。

## 确认应用已注册
装完 desktop 后重启 launcher，可读 launcher 进程内存（sudo 读 `/proc/<pid>/mem` 扫描堆）搜应用名出现次数；但最终以真机翻页看到图标为准。

## 其它
- 面板非 wayland，别用 grim 截图面板；用 fbcap。
- ffmpeg 拉 B 站必须带正确 `Referer`/`User-Agent` 头，否则 403。
- 停 launcher 后写 fb0 的画面会保留；记得测试完 `start` 恢复。
