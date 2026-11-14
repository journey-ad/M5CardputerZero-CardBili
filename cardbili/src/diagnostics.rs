use std::fmt;
use std::fs::OpenOptions;
use std::io::{BufRead, BufReader, Write};
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

const LOG_PATH: &str = "/tmp/cardbili-player.log";

pub(crate) fn write(message: fmt::Arguments<'_>) {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let line = format!("[{}.{:03}] {message}\n", now.as_secs(), now.subsec_millis());
    eprint!("{line}");
    let result = OpenOptions::new()
        .create(true)
        .append(true)
        .open(LOG_PATH)
        .and_then(|mut file| file.write_all(line.as_bytes()));
    if let Err(error) = result {
        eprintln!("写入诊断日志失败 {LOG_PATH}: {error}");
    }
}

pub(crate) fn spawn(
    kind: &'static str,
    args: &[String],
    cookies: &str,
    stdout: Stdio,
    audio: Option<OwnedFd>,
) -> Option<Child> {
    let redact = |text: &str| {
        if cookies.is_empty() {
            text.to_string()
        } else {
            text.replace(cookies, "<Cookie 已隐藏>")
        }
    };
    let logged_args: Vec<_> = args.iter().map(|arg| redact(arg)).collect();
    write(format_args!("[{kind}] ffmpeg 参数: {logged_args:?}"));
    let mut command = Command::new("ffmpeg");
    command
        .args(args)
        .stdin(Stdio::piped())
        .stdout(stdout)
        .stderr(Stdio::piped());
    if let Some(audio) = audio.as_ref() {
        let fd = audio.as_raw_fd();
        // 子进程的描述符 3 接收解码后的 PCM 音频
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(fd, 3) < 0 || libc::fcntl(3, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    match command.spawn() {
        Ok(mut child) => {
            let pid = child.id();
            write(format_args!("[{kind}] ffmpeg 已启动 pid={pid}"));
            if let Some(stderr) = child.stderr.take() {
                let cookies = cookies.to_string();
                thread::spawn(move || {
                    for line in BufReader::new(stderr).lines() {
                        match line {
                            Ok(line) => {
                                let line = if cookies.is_empty() {
                                    line
                                } else {
                                    line.replace(&cookies, "<Cookie 已隐藏>")
                                };
                                write(format_args!("[{kind} pid={pid}] {line}"));
                            }
                            Err(error) => {
                                write(format_args!("[{kind} pid={pid}] 读取 stderr 失败: {error}"));
                                break;
                            }
                        }
                    }
                });
            }
            Some(child)
        }
        Err(error) => {
            write(format_args!("[{kind}] ffmpeg 启动失败: {error}"));
            None
        }
    }
}
