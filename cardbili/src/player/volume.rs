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

fn adjust(step: i32) -> Result<i32, String> {
    let output = pactl(&["get-sink-volume", "@DEFAULT_SINK@"])?;
    let volume = output
        .split_whitespace()
        .find_map(|word| word.strip_suffix('%')?.parse::<i32>().ok())
        .ok_or(crate::i18n::tr("系统音量读取失败"))?;
    let volume = (volume + step).clamp(0, 100);
    pactl(&["set-sink-volume", "@DEFAULT_SINK@", &format!("{volume}%")])?;
    if step > 0 {
        pactl(&["set-sink-mute", "@DEFAULT_SINK@", "0"])?;
    }
    Ok(volume)
}

pub fn worker(tx: Sender<crate::Msg>) -> SyncSender<i32> {
    let (queue, steps) = mpsc::sync_channel(8);
    thread::spawn(move || {
        for step in steps {
            let result = adjust(step);
            crate::diagnostics::write(format_args!("[音量] 调整={step} 结果={result:?}"));
            if tx.send(crate::Msg::Volume(result)).is_err() {
                break;
            }
        }
    });
    queue
}
