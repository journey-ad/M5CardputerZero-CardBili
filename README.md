# CardBili for CardputerZero — 开发包

B 站客户端（Rust + Slint，320x170 直写 framebuffer），运行在 M5Stack CardputerZero（aarch64 Debian）。

## 目录结构

```
cardbili-dev/
├── README.md                    本说明
├── cardbili/                    完整源码（Rust + Slint）
│   ├── Cargo.toml / Cargo.lock
│   ├── build.rs                 Slint 编译，EmbedFiles 内嵌资源
│   ├── src/                     main/model/api/util/tasks/app/player/net
│   ├── ui/app.slint             全部界面
│   ├── assets/                  方向三角图标 tri_*.png
│   ├── fonts/                   zpix.ttf（实际使用）+ 其他中文字体
│   └── syslib/libdl.a
├── skill/cardputer-zero/        可复用技能（SKILL.md + references + scripts）
├── docs/
│   ├── cardbili.desktop         APPLaunch 启动项
│   ├── cb_deploy.sh             部署脚本参考
│   ├── build-config.example.toml 交叉编译 .cargo/config 示例
│   ├── prototype/cyberbili.py   早期 Python 原型（历史参考）
│   └── screenshots/             真机效果截图
└── dist/                        已编译产物，可直接装到设备
    ├── M5CardputerZero-CardBili aarch64 musl 静态 ELF
    ├── cardbili.desktop
    └── cardbili_{100,80}.png    APPLaunch 图标
```

## 构建

目标：`aarch64-unknown-linux-musl`（静态 ELF）。

```bash
rustup target add aarch64-unknown-linux-musl
cd cardbili
cargo build --release --target aarch64-unknown-linux-musl
# 产物 target/aarch64-unknown-linux-musl/release/cardbili
```

`.cargo/config` 里的 linker 路径是 macOS 专用的（rust-lld），换环境需按平台修改，见 `docs/build-config.example.toml`。Linux 上一般只需 musl 工具链；macOS 可用 `filosottile/musl-cross` 或 rust-lld（`-C link-self-contained=yes -L native=syslib`）。

## 安装到设备

设备地址、ssh 用户与口令属于本地环境敏感信息，存放在项目根的 `.secrets/device.env`（已在 `.gitignore` 忽略，不入库），字段模板见 `docs/device.env.example`。设备 IP 由手机热点临时分配，重连可能变化，以手机热点「已连接设备」列表为准。

```bash
source .secrets/device.env

scp dist/M5CardputerZero-CardBili "$DEVICE_USER@$DEVICE_IP:/tmp/"
scp dist/cardbili.desktop          "$DEVICE_USER@$DEVICE_IP:/tmp/"
scp dist/cardbili_100.png dist/cardbili_80.png "$DEVICE_USER@$DEVICE_IP:/tmp/"

ssh "$DEVICE_USER@$DEVICE_IP"
echo "$DEVICE_SUDO_PASS" | sudo -S install -m0755 /tmp/M5CardputerZero-CardBili /usr/share/APPLaunch/bin/
echo "$DEVICE_SUDO_PASS" | sudo -S install -m0644 /tmp/cardbili.desktop /usr/share/APPLaunch/applications/cardbili.desktop
echo "$DEVICE_SUDO_PASS" | sudo -S install -m0644 /tmp/cardbili_100.png /usr/share/APPLaunch/share/images/cardbili_100.png
echo "$DEVICE_SUDO_PASS" | sudo -S install -m0644 /tmp/cardbili_80.png  /usr/share/APPLaunch/share/images/cardbili_80.png

export XDG_RUNTIME_DIR=/run/user/1000 DBUS_SESSION_BUS_ADDRESS=unix:path=/run/user/1000/bus
systemctl --user restart APPLaunch.service
```

## 运行/调试

launcher 只打开 `/dev/fb0` 与 `/dev/input/event3`。调试时先停 launcher，直接跑：

```bash
systemctl --user stop APPLaunch.service
APPLAUNCH_LINUX_FBDEV_DEVICE=/dev/fb0 ./cardbili
python3 /tmp/fbcap.py /tmp/shot.png   # 读 /dev/fb0 截图（见 skill/scripts/fbcap.py）
systemctl --user start APPLaunch.service
```

测试用环境变量：

- `BILI_TAB=0..3` 初始标签（推荐/热门/动态/我的）
- `BILI_VIEW=0|1|2` 列表/三列/两列
- `BILI_DETAIL=<bvid>` 直接进详情
- `BILI_ABOUT=1` 直接进关于页
- `BILI_HELP=1` 打开按键说明（注意：只判断变量是否存在）
- `BILI_DUMP=1` 打印条目数/评论数/翻页信息
- `BILI_DSCROLL=<px>` / `BILI_DSEND=1` 详情滚动位置
- `BILI_LOGIN_DEBUG=1` 打印扫码轮询 cookie 细节

## 按键

- 方向键（Z/X/C/F 物理键为 左/下/右/上）：选择卡片、详情滚动
- `Tab`：切换页签（循环）
- `V`：列表/三列/两列切换
- 回车：播放；空格：视频详情；ESC：返回
- 底部 4/5/6/7/8 键对应底栏：刷新/登录/说明/返回（登录后隐藏「登录」）

## 登录态

扫码登录后整串 Cookie（含 SESSDATA）写入 `~/.config/cardbili/cookie.txt`，启动自动读取；SESSDATA 过期需重新扫码。

## 开发要点（详见 skill）

面板不是 Wayland，直接写 `/dev/fb0`（RGB565 小端，320x170，stride 640）；无内建方向键，键位映射见 `skill/references/key-layout.md`；Rust+Slint 用软件渲染器 + EmbedFiles + 常驻 buffer 整帧 pwrite。更多硬件/调试/视频播放经验见 `skill/cardputer-zero/references/`。
