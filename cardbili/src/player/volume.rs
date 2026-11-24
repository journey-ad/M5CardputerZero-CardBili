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

/// 音量操作：按百分比步进、切换静音或临时降低音量
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Change {
    Step(i32),
    ToggleMute,
    Duck,
    Restore,
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

fn apply(change: Change, saved_volume: &mut Option<i32>) -> Result<Option<Level>, String> {
    match change {
        Change::Step(step) => {
            let percent = (read_volume()? + step).clamp(0, 100);
            pactl(&["set-sink-volume", "@DEFAULT_SINK@", &format!("{percent}%")])?;
            if step > 0 {
                pactl(&["set-sink-mute", "@DEFAULT_SINK@", "0"])?;
            }
            Ok(Some(Level {
                percent,
                muted: step <= 0 && read_mute()?,
            }))
        }
        Change::ToggleMute => {
            pactl(&["set-sink-mute", "@DEFAULT_SINK@", "toggle"])?;
            Ok(Some(Level {
                percent: read_volume()?,
                muted: read_mute()?,
            }))
        }
        Change::Duck => {
            let percent = read_volume()?;
            pactl(&[
                "set-sink-volume",
                "@DEFAULT_SINK@",
                &format!("{}%", percent / 10),
            ])?;
            *saved_volume = Some(percent);
            Ok(None)
        }
        Change::Restore => {
            if let Some(percent) = saved_volume.take() {
                pactl(&["set-sink-volume", "@DEFAULT_SINK@", &format!("{percent}%")])?;
            }
            Ok(None)
        }
    }
}

pub fn worker(tx: Sender<crate::Msg>) -> SyncSender<Change> {
    let (queue, changes) = mpsc::sync_channel(8);
    thread::spawn(move || {
        let mut saved_volume = None;
        for change in changes {
            let result = apply(change, &mut saved_volume);
            crate::diagnostics::write(format_args!("[音量] 操作={change:?} 结果={result:?}"));
            let result = match result {
                Ok(Some(level)) => Ok(level),
                Ok(None) => continue,
                Err(error) => Err(error),
            };
            if tx.send(crate::Msg::Volume(result)).is_err() {
                break;
            }
        }
    });
    queue
}
