use serde::{Deserialize, Serialize};
use unicode_properties::{GeneralCategoryGroup, UnicodeGeneralCategory};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Punctuation {
    Full,
    #[default]
    OmitTrailing,
    Spaces,
}

impl Punctuation {
    pub fn label(self) -> &'static str {
        match self {
            Self::Full => crate::i18n::tr("添加完整标点"),
            Self::OmitTrailing => crate::i18n::tr("省略句末标点"),
            Self::Spaces => crate::i18n::tr("空格代替标点"),
        }
    }

    pub fn cycle(self, step: i32) -> Self {
        let all = [Self::Full, Self::OmitTrailing, Self::Spaces];
        let index = all.iter().position(|mode| *mode == self).unwrap() as i32;
        all[(index + step).rem_euclid(3) as usize]
    }
}

pub fn format_text(text: &str, punctuation: Punctuation, add_spaces: bool) -> String {
    let mut spaced = String::with_capacity(text.len());
    let mut previous = None;
    let mut gap_start = 0;
    for (index, ch) in text.char_indices() {
        if ch.is_whitespace() && !matches!(ch, '\n' | '\r') {
            continue;
        }
        let boundary = previous.is_some_and(|prev: char| {
            (is_chinese(prev) && ch.is_ascii_alphanumeric())
                || (prev.is_ascii_alphanumeric() && is_chinese(ch))
        });
        let gap = &text[gap_start..index];
        if !boundary || add_spaces {
            spaced.push_str(gap);
        }
        if boundary && add_spaces && gap.is_empty() {
            spaced.push(' ');
        }
        spaced.push(ch);
        previous = Some(ch);
        gap_start = index + ch.len_utf8();
    }
    spaced.push_str(&text[gap_start..]);

    if punctuation == Punctuation::Full {
        return spaced;
    }
    let text = spaced.trim_end_matches(|ch: char| ch.is_whitespace() || is_punctuation(ch));
    if punctuation == Punctuation::OmitTrailing {
        return text.to_string();
    }
    text.chars()
        .map(|ch| if is_punctuation(ch) { ' ' } else { ch })
        .collect()
}

fn is_chinese(ch: char) -> bool {
    matches!(ch, '〇' | '\u{3400}'..='\u{4dbf}' | '\u{4e00}'..='\u{9fff}'
        | '\u{f900}'..='\u{faff}' | '\u{20000}'..='\u{323af}')
}

fn is_punctuation(ch: char) -> bool {
    ch.general_category_group() == GeneralCategoryGroup::Punctuation
}
