use crate::api::translate::{self, Provider};
use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};
use std::time::Duration;

#[derive(Default)]
struct State {
    cache: HashMap<(String, &'static str), Option<String>>,
    queue: Vec<(String, &'static str)>,
    working: bool,
    dirty: bool,
    provider: i32,
}

static STATE: LazyLock<Mutex<State>> = LazyLock::new(|| Mutex::new(State::default()));
const BATCH: usize = 20;

/// 英文界面的列表与详情标题共用播放标题缓存
pub fn display(title: &str) -> String {
    if crate::i18n::current() == crate::i18n::Language::Zh {
        return title.to_string();
    }
    display_playback(title)
}

/// 未命中的标题先显示原文，后台按目标语言合并请求
pub fn display_playback(title: &str) -> String {
    let has_cjk = title
        .chars()
        .any(|c| ('\u{4e00}'..='\u{9fff}').contains(&c));
    let target = match crate::i18n::current() {
        crate::i18n::Language::En if has_cjk => "en",
        crate::i18n::Language::Zh if !has_cjk && title.chars().any(|c| c.is_ascii_alphabetic()) => {
            "zh"
        }
        _ => return title.to_string(),
    };
    let mut state = STATE.lock().unwrap();
    let key = (title.to_string(), target);
    if let Some(cached) = state.cache.get(&key) {
        return cached.as_deref().unwrap_or(title).to_string();
    }
    state.cache.insert(key.clone(), None);
    state.queue.push(key);
    if !state.working {
        state.working = true;
        std::thread::spawn(work);
    }
    title.to_string()
}

pub fn take_dirty() -> bool {
    std::mem::take(&mut STATE.lock().unwrap().dirty)
}

pub fn set_provider(provider: i32) {
    let mut state = STATE.lock().unwrap();
    if state.provider != provider {
        state.provider = provider;
        state.cache.clear();
        state.queue.clear();
        state.dirty = true;
    }
}

fn work() {
    loop {
        std::thread::sleep(Duration::from_millis(80));
        let (titles, target, provider) = {
            let mut state = STATE.lock().unwrap();
            let Some((_, target)) = state.queue.first() else {
                state.working = false;
                return;
            };
            let target = *target;
            let mut titles = Vec::new();
            state.queue.retain(|(title, language)| {
                if *language == target && titles.len() < BATCH {
                    titles.push(title.clone());
                    false
                } else {
                    true
                }
            });
            (titles, target, state.provider)
        };
        let source = if target == "en" { "zh" } else { "en" };
        let service = Provider::from_index(provider);
        let translated = match translate::translate_batch(service, &titles, source, target) {
            Ok(lines) => {
                crate::diagnostics::write(format_args!("[标题] 批量翻译 {} 条", titles.len()));
                lines.into_iter().map(Some).collect::<Vec<_>>()
            }
            Err(_) => {
                crate::diagnostics::write(format_args!(
                    "[标题] 批量失败，逐条翻译 {} 条",
                    titles.len()
                ));
                titles
                    .iter()
                    .map(|title| {
                        match translate::translate_batch(
                            service,
                            std::slice::from_ref(title),
                            source,
                            target,
                        ) {
                            Ok(mut lines) => Some(lines.remove(0)),
                            Err(error) => {
                                crate::diagnostics::write(format_args!("[标题] 翻译失败: {error}"));
                                None
                            }
                        }
                    })
                    .collect()
            }
        };
        let mut state = STATE.lock().unwrap();
        if state.provider == provider {
            for (title, text) in titles.into_iter().zip(translated) {
                if let Some(text) = text {
                    state.cache.insert((title, target), Some(text));
                    state.dirty = true;
                } else {
                    state.cache.remove(&(title, target));
                }
            }
        }
    }
}
