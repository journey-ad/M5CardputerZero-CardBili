use serde::{Deserialize, Serialize};
use std::sync::mpsc;
use std::thread;

mod format;
pub use format::{format_text, Punctuation};

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
#[allow(dead_code)]
mod voice_asr;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Backend {
    Qwen,
    #[default]
    Doubao,
    Wechat,
}

impl Backend {
    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    pub fn id(self) -> &'static str {
        match self {
            Self::Qwen => "qwen",
            Self::Doubao => "doubao",
            Self::Wechat => "wechat",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Qwen => crate::i18n::tr("千问"),
            Self::Doubao => crate::i18n::tr("豆包"),
            Self::Wechat => crate::i18n::tr("微信"),
        }
    }

    pub fn cycle(self, step: i32) -> Self {
        let all = [Self::Qwen, Self::Doubao, Self::Wechat];
        let index = all.iter().position(|backend| *backend == self).unwrap() as i32;
        all[(index + step).rem_euclid(3) as usize]
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum Event {
    Started,
    Partial { text: String },
    Final { text: String },
    Error { message: String },
    Ended { reason: String },
}

enum Command {
    Start(Backend, u64),
    Stop,
    Cancel,
}

pub struct Session(mpsc::Sender<Command>);

impl Session {
    pub fn start(backend: Backend, request: u64, tx: mpsc::Sender<crate::Msg>) -> Self {
        let (commands, rx) = mpsc::channel();
        thread::spawn(move || {
            let mut next = Some(Command::Start(backend, request));
            while let Some(command) = next.take().or_else(|| rx.recv().ok()) {
                if let Command::Start(backend, request) = command {
                    match recognize(backend, request, &tx, &rx) {
                        Ok(command) => next = command,
                        Err(message) => {
                            let _ = tx.send(crate::Msg::Voice(request, Event::Error { message }));
                        }
                    }
                }
            }
        });
        Self(commands)
    }

    pub fn stop(&self) {
        let _ = self.0.send(Command::Stop);
    }

    pub fn restart(&self, backend: Backend, request: u64) {
        let _ = self.0.send(Command::Start(backend, request));
    }

    pub fn cancel(&self) {
        let _ = self.0.send(Command::Cancel);
    }
}

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
fn recognize(
    backend: Backend,
    request: u64,
    tx: &mpsc::Sender<crate::Msg>,
    commands: &mpsc::Receiver<Command>,
) -> Result<Option<Command>, String> {
    let mut recognizer =
        voice_asr::Recognizer::new(r#"{"audio":{"mixer":{"adc":232,"pga":12,"alc":0}}}"#)?;
    let options = serde_json::json!({
        "backend": backend.id(),
        "input": "microphone",
    });
    recognizer.start(&options.to_string())?;
    loop {
        match commands.try_recv() {
            Ok(command @ Command::Start(..)) => return Ok(Some(command)),
            Ok(Command::Cancel) | Err(mpsc::TryRecvError::Disconnected) => {
                return Ok(None);
            }
            Ok(Command::Stop) => {
                if let Err(error) = recognizer.stop() {
                    if !error.starts_with("1003:") {
                        return Err(error);
                    }
                }
            }
            Err(mpsc::TryRecvError::Empty) => {}
        }
        if let Some(json) = recognizer.poll_event(20)? {
            let event: Event = serde_json::from_str(&json).map_err(|error| error.to_string())?;
            let ended = matches!(event, Event::Ended { .. });
            if tx.send(crate::Msg::Voice(request, event)).is_err() || ended {
                return Ok(None);
            }
        }
    }
}

#[cfg(not(all(target_os = "linux", target_arch = "aarch64")))]
fn recognize(
    _backend: Backend,
    _request: u64,
    _tx: &mpsc::Sender<crate::Msg>,
    _commands: &mpsc::Receiver<Command>,
) -> Result<Option<Command>, String> {
    Err(crate::i18n::tr("语音输入请在 Cardputer Zero 上使用").into())
}
