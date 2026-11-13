# 视频播放与 B 站接口

## ffmpeg 播放配方

### 两进程（推荐，cardbili 采用）
视频：`-re` 实时吐帧，scale+letterbox 到 320x170，输出 rgb 到 stdout。
```
ffmpeg -hide_banner -loglevel error -nostdin \
  -protocol_whitelist file,pipe,crypto,http,https,tcp,tls \
  -user_agent "<UA>" -headers "Referer: https://www.bilibili.com/\r\n" -headers "Origin: https://www.bilibili.com\r\n" \
  -re -i "<url>" -map 0:v:0 \
  -vf "fps=30,scale=320:170:force_original_aspect_ratio=decrease:force_divisible_by=2:flags=fast_bilinear,pad=320:170:(ow-iw)/2:(oh-ih)/2:color=black" \
  -pix_fmt rgb24 -f rawvideo pipe:1
```
音频：`... -map 0:a:0 -ac 2 -ar 44100 -f pulse default`
- 读帧：320*170*3=**163200** 字节/帧（rgb24）；若直接 rgb565le 则 108800 字节/帧。
- letterbox 实测：640x360 → 302x170 居中，左右各约 9px 黑边。`flags=fast_bilinear` 必带，否则掉帧。

### 单进程双输出（也验证可用）
把视频 `-map 0:v:0 -vf "fps=20,scale=320:170:flags=fast_bilinear" -pix_fmt rgb565le -threads:v 1 -f rawvideo pipe:1` 与音频 `-map 0:a:0 -ac 2 -ar 44100 -f pulse default` 放同一个 ffmpeg。注：某次组合单进程 + pulse 异常慢（3s 片段 88s），稳定性不如两进程；视频-only 很快，音频实时。

### 其它注意
- `-headers` 值里冒号后要有空格并整体加引号，写成 `-headers Referer:https://...` 会 403。
- 硬件解码 h264_v4l2m2m 在本机失败（输出 <1 帧）；软件解码约 7× 实时（360P）。
- `-re` 让 ffmpeg 按实时吐帧，python/rust 即读即写即可；不加会瞬间灌满管道。
- 音量：`pactl --format=json list sink-inputs` 按 `application.process.id==pid` 找本进程的 sink-input，`pactl set-sink-input-volume <index> <pct>%`。

## B 站接口
请求头必备：`User-Agent: Mozilla/5.0 (X11; Linux aarch64) ... Chrome/120 ...` + `Referer: https://www.bilibili.com/`（拉流加 `Origin`）。

- 热门：`GET https://api.bilibili.com/x/web-interface/popular?ps=24&pn=1`
- 详情：`x/web-interface/view?bvid=BV..` → data.{cid,title,duration,pages}
- 取流：`x/player/playurl?bvid=BV..&cid=..&qn=16&fnval=1&platform=html5&high_quality=1` → `data.durl[].url`、`durl[].length`(ms)；多段需顺序拼接播放。
- 清晰度：匿名 `platform=html5` 的 `accept_quality` 只有 `[64,16]`(720P/360P)；qn=1/6/32 都被回落 16；**匿名最低 360P，没有 240P**。普通 UGC 匿名返回完整时长单段 durl；受限内容（纪录片/番剧）匿名无流，需登录。
- 推荐：`x/web-interface/wbi/index/top/feed/rcmd`（需 WBI 签名；ps=12,fresh_type=4,feed_version=V8,...）
- 历史：`x/web-interface/history/cursor`；收藏夹：`x/v3/fav/folder/created/list-all` + `x/v3/fav/resource/list`；动态：`x/polymer/web-dynamic/v1/feed/all`；nav：`x/web-interface/nav`

## WBI 签名
mixin 重排表：`[46,47,18,2,53,8,23,32,15,50,10,31,58,3,45,35,27,43,5,49,33,9,42,19,29,28,14,39,12,38,41,13]`
1. `GET x/web-interface/nav` → `data.wbi_img.img_url` / `sub_url`，各取文件名主名（去扩展名），拼接后按上表重排得 `mixin_key`。
2. 参数加 `wts`=当前秒；按 key 升序排序；value 过滤字符 `!'()*`；各自 percent-encode；`k=v` 用 `&` 连接。
3. `w_rid = md5(query + mixin_key)`。

## 扫码登录（web QR，简单，无需 appkey 签名）
- 生成：`GET https://passport.bilibili.com/x/passport-login/web/qrcode/generate` → `data.url`（含 qrcode_key）+ `data.qrcode_key`。
- 轮询：`GET .../x/passport-login/web/qrcode/poll?qrcode_key=<key>`，`data.code`：`0` 成功 / `86101` 未扫码 / `86090` 已扫待确认 / `86038` 过期。
- 成功：用 `curl -c jar` 收 Set-Cookie（`SESSDATA`、`bili_jct`、`DedeUserID`、`DedeUserID__ckMd5`、`buvid3`），并解析 `data.url` 查询串；持久化到 `~/.config/<app>/cookie.txt`。
- QR 渲染：Rust `qrcodegen::QrCode::encode_text(url, QrCodeEcc::Low)` 逐 module 放大画图。
