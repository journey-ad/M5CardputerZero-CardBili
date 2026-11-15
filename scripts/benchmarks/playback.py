#!/usr/bin/env python3
"""在临时源码中注入播放计时探针，并在设备上采集进程资源使用量"""

import argparse
from functools import partial
from http.server import SimpleHTTPRequestHandler, ThreadingHTTPServer
import json
import os
from pathlib import Path
import shutil
import subprocess
import threading
import time


def replace(path, old, new):
    text = path.read_text()
    assert text.count(old) == 1, (path, old)
    path.write_text(text.replace(old, new))


def prepare(root, destination):
    app = destination / "cardbili"
    app.mkdir(parents=True, exist_ok=True)
    for name in ["src", "ui", "fonts", "assets", "build"]:
        shutil.copytree(root / "cardbili" / name, app / name, dirs_exist_ok=True)
    for name in ["Cargo.toml", "Cargo.lock", "build.rs"]:
        shutil.copy2(root / "cardbili" / name, app / name)
    shutil.copy2(Path(__file__).with_suffix(".rs"), app / "src/app/bench.rs")
    path = app / "src/app.rs"
    replace(path, "mod about;", "mod bench;\nmod about;")
    replace(path, "    let mut pending = None;", "    bench::start(&mut app);\n    let mut pending = None;")
    replace(path, "        app.tick();", "        bench::tick(&mut app);\n        app.tick();")
    replace(path, "        app.sync_view();", "        let fresh_frame = app.latest.is_some();\n        let sync_start = Instant::now();\n        app.sync_view();\n        bench::record(0, sync_start);")
    replace(path, "        pending = rx", "        if fresh_frame { bench::frame(); }\n        pending = rx")
    replace(path, "    app.close_cdn();", "    bench::finish();\n    app.close_cdn();")
    path = app / "src/app/state.rs"
    replace(path, "        if tab == HomeTab::Live {", "        if false && tab == HomeTab::Live {")
    replace(path, "        } else if tab.is_video() {", "        } else if false && tab.is_video() {")
    path = app / "src/app/playback/view.rs"
    replace(path, "        // --- 应用最新帧 ---", "        let handoff_start = Instant::now();\n        // --- 应用最新帧 ---")
    replace(path, "                g_player.set_has_frame(true);", "                g_player.set_has_frame(true);\n                super::super::bench::record(1, handoff_start);")
    path = app / "src/app/platform.rs"
    replace(path, "            let _ = r.render(&mut self.buffer, WIDTH as usize);", "            let start = Instant::now();\n            let _ = r.render(&mut self.buffer, WIDTH as usize);\n            super::bench::record(2, start);")
    replace(path, "                let out = unsafe {", "                let start = Instant::now();\n                let out = unsafe {")
    replace(path, "                    eprintln!(\"提交显示帧失败: {error}\");\n                    return false;\n                }", "                    eprintln!(\"提交显示帧失败: {error}\");\n                    return false;\n                }\n                super::bench::record(3, start);")
    print(app)


def processes(pid):
    # /proc 的 CPU tick 使用采样窗口差值，百分比按单核满载计算
    found = {"app": pid}
    children = Path(f"/proc/{pid}/task/{pid}/children")
    if children.exists():
        for child in children.read_text().split():
            name = Path(f"/proc/{child}/comm").read_text().strip()
            if "ffmpeg" in name:
                found["ffmpeg"] = int(child)
            elif name == "pacat":
                found["pacat"] = int(child)
    return found


def snapshot(pids):
    result = {}
    for name, pid in pids.items():
        stat = Path(f"/proc/{pid}/stat").read_text().rsplit(")", 1)[1].split()
        result[name] = {
            "ticks": int(stat[11]) + int(stat[12]),
            "rss_kib": int(stat[21]) * os.sysconf("SC_PAGE_SIZE") / 1024,
        }
    return result


def run(binary, media, controls, output):
    binary, media, output = binary.resolve(), media.resolve(), output.resolve()
    class Handler(SimpleHTTPRequestHandler):
        def log_message(self, *args):
            pass

    # 本机 HTTP 保留正式播放器的输入参数，同时固定媒体内容
    server = ThreadingHTTPServer(("127.0.0.1", 0), partial(Handler, directory=str(media.parent)))
    # server_close 等待媒体连接线程结束后再退出解释器
    server.daemon_threads = False
    threading.Thread(target=server.serve_forever, daemon=True).start()
    env = os.environ.copy()
    private = output.parent / "runtime"
    for kind in ["CONFIG", "CACHE", "STATE"]:
        env[f"XDG_{kind}_HOME"] = str(private / kind.lower())
    env.update(
        BENCH_MEDIA=f"http://127.0.0.1:{server.server_port}/{media.name}",
        BENCH_CONTROLS=controls,
        BILI_TAB="0",
    )
    env.pop("CARDBILI_DEBUG", None)
    output.parent.mkdir(parents=True, exist_ok=True)
    with output.with_suffix(".log").open("w") as log:
        process = subprocess.Popen([str(binary)], env=env, stdout=log, stderr=log)
        try:
            time.sleep(5.5)
            pids = processes(process.pid)
            begin = snapshot(pids)
            started = time.monotonic()
            rss = {name: [data["rss_kib"]] for name, data in begin.items()}
            for _ in range(28):
                time.sleep(1)
                end = snapshot(pids)
                for name, data in end.items():
                    rss[name].append(data["rss_kib"])
            elapsed = time.monotonic() - started
            resources = {
                name: {
                    "cpu_pct": (end[name]["ticks"] - data["ticks"]) / os.sysconf("SC_CLK_TCK") / elapsed * 100,
                    "rss_mean_kib": sum(rss[name]) / len(rss[name]),
                    "rss_max_kib": max(rss[name]),
                } for name, data in begin.items()
            }
            code = process.wait(timeout=10)
            assert code == 0, code
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=5)
            server.shutdown()
            server.server_close()
    lines = output.with_suffix(".log").read_text().splitlines()
    probe = next(json.loads(line.removeprefix("BENCH ")) for line in lines if line.startswith("BENCH "))
    result = {
        "binary": str(binary), "controls": controls,
        "resource_seconds": elapsed, "resources": resources, "timings": probe,
        "temperature_millicelsius": int(Path("/sys/class/thermal/thermal_zone0/temp").read_text()),
        "cpu_frequency_khz": int(Path("/sys/devices/system/cpu/cpufreq/policy0/scaling_cur_freq").read_text()),
    }
    output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n")
    print(json.dumps(result, ensure_ascii=False), flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    prepare_args = commands.add_parser("prepare")
    prepare_args.add_argument("root", type=Path)
    prepare_args.add_argument("destination", type=Path)
    run_args = commands.add_parser("run")
    run_args.add_argument("binary", type=Path)
    run_args.add_argument("media", type=Path)
    run_args.add_argument("controls", choices=["hidden", "visible"])
    run_args.add_argument("output", type=Path)
    args = parser.parse_args()
    if args.command == "prepare":
        prepare(args.root, args.destination)
    else:
        run(args.binary, args.media, args.controls, args.output)
