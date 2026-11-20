//! 界面语言
//!
//! 源码中的中文原文即翻译键（gettext 的 msgid），译文来自 `i18n/<语言>.po`，
//! 编译时打包进二进制。未翻译或缺失的条目回退到中文原文，因此新增文案无需
//! 同步修改译文文件即可正常显示
//!
//! Rust 侧使用 `tr` / `trf`，Slint 侧使用 `I18n.tr` / `I18n.f`（见 ui/state.slint），
//! 两者共用同一份译文。Slint 升级到 1.9 及以上后，这些 .po 文件也可直接用于 `@tr`

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt::Display;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::OnceLock;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    /// 简体中文，即源码原文
    #[default]
    Zh,
    En,
}

impl Language {
    pub const ALL: [Self; 2] = [Self::Zh, Self::En];


    pub fn cycle(self, step: i32) -> Self {
        let index = Self::ALL.iter().position(|item| *item == self).unwrap_or(0) as i32;
        Self::ALL[(index + step).rem_euclid(Self::ALL.len() as i32) as usize]
    }

    /// `CARDBILI_LANG=en` / `zh` 覆盖设置，便于调试与截图
    pub fn from_env() -> Option<Self> {
        let value = std::env::var("CARDBILI_LANG").ok()?;
        let value = value.to_ascii_lowercase();
        if value.starts_with("en") {
            Some(Self::En)
        } else if value.starts_with("zh") {
            Some(Self::Zh)
        } else {
            None
        }
    }
}

// 未知取值回退为中文，而不是让整个设置文件读取失败
impl<'de> Deserialize<'de> for Language {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = String::deserialize(deserializer).unwrap_or_default();
        Ok(if value.eq_ignore_ascii_case("en") { Self::En } else { Self::Zh })
    }
}

static CURRENT: AtomicU8 = AtomicU8::new(0);

pub fn set(language: Language) {
    CURRENT.store(language as u8, Ordering::Relaxed);
}

pub fn current() -> Language {
    match CURRENT.load(Ordering::Relaxed) {
        1 => Language::En,
        _ => Language::Zh,
    }
}

fn english() -> &'static HashMap<String, String> {
    static CATALOG: OnceLock<HashMap<String, String>> = OnceLock::new();
    CATALOG.get_or_init(|| parse_po(include_str!("../i18n/en.po")))
}

/// 当前语言下的文案；中文或未翻译时原样返回
pub fn tr(text: &str) -> &str {
    if current() == Language::Zh {
        return text;
    }
    english().get(text).map(String::as_str).unwrap_or(text)
}

/// 先翻译模板，再按顺序替换其中的 `{}`。不同语言的语序可能不同，
/// 因此带参数的文案整体作为一条译文，而不是拼接片段
pub fn trf(template: &str, args: &[&dyn Display]) -> String {
    fill(tr(template), args)
}

pub(crate) fn fill(template: &str, args: &[&dyn Display]) -> String {
    let mut out = String::with_capacity(template.len() + 16);
    let mut args = args.iter();
    let mut rest = template;
    while let Some(at) = rest.find("{}") {
        out.push_str(&rest[..at]);
        match args.next() {
            Some(arg) => out.push_str(&arg.to_string()),
            None => out.push_str("{}"),
        }
        rest = &rest[at + 2..];
    }
    out.push_str(rest);
    out
}

/// 解析 gettext .po 的 msgid / msgstr（含多行续写与 \n \t \" \\ 转义）；
/// 表头与空译文跳过
pub(crate) fn parse_po(source: &str) -> HashMap<String, String> {
    #[derive(PartialEq)]
    enum Field {
        None,
        Id,
        Str,
    }
    let mut catalog = HashMap::new();
    let (mut id, mut text, mut field) = (String::new(), String::new(), Field::None);
    let mut flush = |id: &mut String, text: &mut String| {
        if !id.is_empty() && !text.is_empty() {
            catalog.insert(std::mem::take(id), std::mem::take(text));
        }
        id.clear();
        text.clear();
    };
    for line in source.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("msgid ") {
            flush(&mut id, &mut text);
            id = unquote(rest);
            field = Field::Id;
        } else if let Some(rest) = line.strip_prefix("msgstr ") {
            text = unquote(rest);
            field = Field::Str;
        } else if line.starts_with('"') {
            match field {
                Field::Id => id.push_str(&unquote(line)),
                Field::Str => text.push_str(&unquote(line)),
                Field::None => {}
            }
        } else if line.is_empty() {
            field = Field::None;
        }
    }
    flush(&mut id, &mut text);
    catalog
}

fn unquote(quoted: &str) -> String {
    let inner = quoted
        .trim()
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or("");
    let mut out = String::with_capacity(inner.len());
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some(other) => out.push(other),
            None => {}
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_po_entries_with_escapes_and_continuations() {
        let catalog = parse_po(
            "msgid \"\"\nmsgstr \"Content-Type: text/plain\\n\"\n\n\
             msgid \"加载失败\"\nmsgstr \"Failed to load\"\n\n\
             msgid \"二维码生成失败\\n返回为空\"\nmsgstr \"\"\n\"QR code failed\\n\"\n\"empty reply\"\n\n\
             msgid \"未翻译\"\nmsgstr \"\"\n",
        );
        assert_eq!(catalog.get("加载失败").map(String::as_str), Some("Failed to load"));
        assert_eq!(
            catalog.get("二维码生成失败\n返回为空").map(String::as_str),
            Some("QR code failed\nempty reply")
        );
        assert!(!catalog.contains_key("未翻译"));
        assert!(!catalog.contains_key(""));
    }

    #[test]
    fn fills_placeholders_in_order() {
        assert_eq!(fill("{}人在看", &[&12]), "12人在看");
        assert_eq!(fill("{} of {}", &[&1, &"x"]), "1 of x");
        assert_eq!(fill("{} {}", &[&1]), "1 {}");
    }

    #[test]
    fn every_english_entry_keeps_its_placeholders() {
        // 译文与原文的 {} 数量必须一致，否则 trf 会丢参数或留下 {}
        for (id, text) in english() {
            assert_eq!(id.matches("{}").count(), text.matches("{}").count(), "{id} -> {text}");
        }
    }
}
