use std::process::Command;
use std::sync::mpsc::{self, Sender, SyncSender};
use std::thread;

fn pactl(args: &[&str]) -> Result<String, String> {
    let output = Command::new("pactl")
        .args(args)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().into());
    }
    Ok(String::from_utf8_lossy(&output.stdout).into())
}

/// 音量操作：按百分比步进，或切换静音
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Step(i32),
    ToggleMute,
}

/// 调整后的系统音量
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Level {
    pub percent: i32,
    pub muted: bool,
}

fn read_volume() -> Result<i32, String> {
    let output = pactl(&["get-sink-volume", "@DEFAULT_SINK@"])?;
    output
        .split_whitespace()
        .find_map(|word| word.strip_suffix('%')?.parse::<i32>().ok())
        .ok_or_else(|| crate::i18n::tr("系统音量读取失败").into())
}

fn read_mute() -> Result<bool, String> {
    Ok(pactl(&["get-sink-mute", "@DEFAULT_SINK@"])?.contains("yes"))
}

fn apply(change: Change) -> Result<Level, String> {
    match change {
        Change::Step(step) => {
            let percent = (read_volume()? + step).clamp(0, 100);
            pactl(&["set-sink-volume", "@DEFAULT_SINK@", &format!("{percent}%")])?;
            if step > 0 {
                pactl(&["set-sink-mute", "@DEFAULT_SINK@", "0"])?;
            }
            Ok(Level {
                percent,
                muted: step <= 0 && read_mute()?,
            })
        }
        Change::ToggleMute => {
            pactl(&["set-sink-mute", "@DEFAULT_SINK@", "toggle"])?;
            Ok(Level {
                percent: read_volume()?,
                muted: read_mute()?,
            })
        }
    }
}

pub fn worker(tx: Sender<crate::Msg>) -> SyncSender<Change> {
    let (queue, changes) = mpsc::sync_channel(8);
    thread::spawn(move || {
        for change in changes {
            let result = apply(change);
            crate::diagnostics::write(format_args!("[音量] 操作={change:?} 结果={result:?}"));
            if tx.send(crate::Msg::Volume(result)).is_err() {
                break;
            }
        }
    });
    queue
}
