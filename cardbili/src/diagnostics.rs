use std::fmt;
use std::fs::{File, OpenOptions};
use std::io::{self, BufRead, BufReader, IsTerminal, Write};
#[cfg(not(feature = "desktop"))]
use std::os::fd::FromRawFd;
#[cfg(unix)]
use std::os::fd::AsRawFd;
#[cfg(all(unix, not(feature = "desktop")))]
use std::os::fd::OwnedFd;
#[cfg(not(feature = "desktop"))]
use std::os::unix::net::UnixStream;
#[cfg(unix)]
use std::os::unix::process::CommandExt;
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(unix)]
pub(crate) type AudioPipe = std::os::fd::OwnedFd;
#[cfg(not(unix))]
pub(crate) type AudioPipe = ();

const MAX_FILES: usize = 5;

struct Logger {
    file: Option<File>,
    started: bool,
    mirror: bool,
}

static LOG: Mutex<Logger> = Mutex::new(Logger {
    file: None,
    started: false,
    mirror: false,
});

pub(crate) fn path() -> &'static std::path::Path {
    static PATH: std::sync::OnceLock<std::path::PathBuf> = std::sync::OnceLock::new();
    PATH.get_or_init(|| crate::util::state_dir().join("player.log"))
}

pub(crate) fn set_enabled(enabled: bool) -> io::Result<()> {
    let mut log = LOG.lock().unwrap();
    if enabled {
        std::fs::create_dir_all(path().parent().unwrap())?;
        if !log.started {
            for index in (1..MAX_FILES).rev() {
                let source = if index == 1 {
                    path().to_path_buf()
                } else {
                    path().with_file_name(format!("player.{}.log", index - 1))
                };
                let target = path().with_file_name(format!("player.{index}.log"));
                match std::fs::rename(source, target) {
                    Ok(()) => {}
                    Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
        }
        log.file = Some(OpenOptions::new().create(true).append(true).open(path())?);
        log.started = true;
        log.mirror = std::io::stderr().is_terminal();
    } else {
        log.file = None;
        log.mirror = false;
    }
    Ok(())
}

// 是否同时把日志写到 stderr，便于从终端启动时直接查看
pub(crate) fn set_mirror(enabled: bool) {
    LOG.lock().unwrap().mirror = enabled;
}

pub(crate) fn write(message: fmt::Arguments<'_>) {
    let mut log = LOG.lock().unwrap();
    if log.file.is_none() && !log.mirror {
        return;
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let line = format!("[{}.{:03}] {message}\n", now.as_secs(), now.subsec_millis());
    if let Some(file) = log.file.as_mut() {
        if let Err(error) = file.write_all(line.as_bytes()) {
            eprintln!("写入诊断日志失败 {}: {error}", path().display());
        }
    }
    if log.mirror {
        let _ = io::stderr().write_all(line.as_bytes());
    }
}

static INSTALL_ATTEMPTED: std::sync::Once = std::sync::Once::new();

// 桌面版找不到 ffmpeg 时，尝试用系统包管理器安装一次
fn ensure_ffmpeg_installed() {
    INSTALL_ATTEMPTED.call_once(|| {
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;
            let script = "winget install --id Gyan.FFmpeg -e --accept-source-agreements --accept-package-agreements; Write-Host 'FFmpeg 安装完成，按回车关闭窗口'; Read-Host | Out-Null";
            match Command::new("powershell")
                .args(["-NoProfile", "-Command", script])
                .creation_flags(CREATE_NEW_CONSOLE)
                .spawn()
            {
                Ok(_) => write(format_args!(
                    "[ffmpeg] 未找到 ffmpeg，已打开 PowerShell 调用 winget 安装"
                )),
                Err(error) => write(format_args!("[ffmpeg] 打开安装窗口失败: {error}")),
            }
        }
        #[cfg(target_os = "macos")]
        {
            let script = "tell application \"Terminal\" to do script \"brew install ffmpeg\"";
            match Command::new("osascript")
                .args([
                    "-e",
                    script,
                    "-e",
                    "tell application \"Terminal\" to activate",
                ])
                .spawn()
            {
                Ok(_) => write(format_args!(
                    "[ffmpeg] 未找到 ffmpeg，已打开终端调用 brew 安装"
                )),
                Err(error) => write(format_args!("[ffmpeg] 打开安装窗口失败: {error}")),
            }
        }
        #[cfg(not(any(windows, target_os = "macos")))]
        {
            write(format_args!("[ffmpeg] 未找到 ffmpeg，请先安装"));
        }
    });
}

pub(crate) fn spawn(
    kind: &'static str,
    args: &[String],
    cookies: &str,
    stdout: Stdio,
    audio: Option<AudioPipe>,
) -> Option<Child> {
    #[cfg(not(unix))]
    let _ = &audio;
    let mut redactions = Vec::new();
    if !cookies.is_empty() {
        redactions.push((cookies.to_string(), "<Cookie 已隐藏>".to_string()));
    }
    for arg in args {
        if arg.starts_with("http://") || arg.starts_with("https://") {
            if let Some((path, _)) = arg.split_once('?') {
                redactions.push((arg.clone(), format!("{path}?<签名已隐藏>")));
            }
        }
    }
    let redact = move |text: &str| {
        redactions
            .iter()
            .fold(text.to_string(), |text, (secret, label)| {
                text.replace(secret, label)
            })
    };
    let logged_args: Vec<_> = args.iter().map(|arg| redact(arg)).collect();
    write(format_args!("[{kind}] ffmpeg 参数: {logged_args:?}"));
    #[cfg(feature = "desktop")]
    let executable = std::path::PathBuf::from("ffmpeg");
    #[cfg(not(feature = "desktop"))]
    let executable = std::env::current_exe()
        .ok()?
        .with_file_name("cardbili-ffmpeg");
    write(format_args!("[{kind}] 解码器={}", executable.display()));
    let mut command = Command::new(executable);
    command
        .stdin(Stdio::piped())
        .stdout(stdout)
        .stderr(Stdio::piped());
    #[cfg(feature = "desktop")]
    command.args(args);
    #[cfg(not(feature = "desktop"))]
    let (arguments, mut writer, payload) = match (|| -> std::io::Result<_> {
        let mut payload = Vec::new();
        for arg in args {
            payload.extend_from_slice(std::ffi::CString::new(arg.as_str())?.as_bytes_with_nul());
        }
        let (reader, writer) = UnixStream::pair()?;
        // 描述符 3 用于音频，4 用于参数，传输源保留在 5 及以上
        let fd = unsafe { libc::fcntl(reader.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 5) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error());
        }
        Ok((unsafe { OwnedFd::from_raw_fd(fd) }, writer, payload))
    })() {
        Ok(value) => value,
        Err(error) => {
            write(format_args!("[{kind}] 创建参数管道失败: {error}"));
            return None;
        }
    };
    #[cfg(unix)]
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
    #[cfg(not(feature = "desktop"))]
    {
        let fd = arguments.as_raw_fd();
        unsafe {
            command.pre_exec(move || {
                if libc::dup2(fd, 4) < 0 || libc::fcntl(4, libc::F_SETFD, 0) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    match command.spawn() {
        Ok(mut child) => {
            #[cfg(not(feature = "desktop"))]
            {
                let result = writer.write_all(&payload);
                drop(writer);
                if let Err(error) = result {
                    write(format_args!("[{kind}] 传递解码参数失败: {error}"));
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
            }
            let pid = child.id();
            write(format_args!("[{kind}] ffmpeg 已启动 pid={pid}"));
            if let Some(stderr) = child.stderr.take() {
                thread::spawn(move || {
                    for line in BufReader::new(stderr).lines() {
                        match line {
                            Ok(line) => {
                                let line = redact(&line);
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
            if cfg!(feature = "desktop") && error.kind() == io::ErrorKind::NotFound {
                ensure_ffmpeg_installed();
            }
            None
        }
    }
}
