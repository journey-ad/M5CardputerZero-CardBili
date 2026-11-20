use crate::net;
use serde_json::Value;

// 免密钥翻译服务
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Provider {
    Google,
    Bing,
}

impl Provider {
    pub const ALL: [Provider; 2] = [Provider::Google, Provider::Bing];

    pub fn from_index(index: i32) -> Self {
        Self::ALL[index.clamp(0, Self::ALL.len() as i32 - 1) as usize]
    }
}

fn translate(provider: Provider, text: &str, from: &str, to: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    match provider {
        Provider::Google => google(text, from, to),
        Provider::Bing => bing(text, from, to),
    }
}

// Google 翻译网页接口
fn google(text: &str, from: &str, to: &str) -> Result<String, String> {
    let params = vec![
        ("client".to_string(), "gtx".to_string()),
        ("sl".to_string(), from.to_string()),
        ("tl".to_string(), to.to_string()),
        ("dt".to_string(), "t".to_string()),
        ("q".to_string(), text.to_string()),
    ];
    let url = format!(
        "https://translate.googleapis.com/translate_a/single?{}",
        net::query_string(&params)
    );
    let body = net::get(&url, None)?;
    let v: Value =
        serde_json::from_str(&body).map_err(|_| crate::i18n::tr("翻译解析失败").to_string())?;
    let mut out = String::new();
    if let Some(segments) = v[0].as_array() {
        for seg in segments {
            if let Some(piece) = seg[0].as_str() {
                out.push_str(piece);
            }
        }
    }
    if out.is_empty() {
        return Err(crate::i18n::tr("翻译结果为空").to_string());
    }
    Ok(out)
}

// Edge 翻译接口，用公开网关换取 Bearer 令牌
fn bing(text: &str, from: &str, to: &str) -> Result<String, String> {
    let token = net::get("https://edge.microsoft.com/translate/auth", None)?;
    let token = token.trim().to_string();
    let url = format!(
        "https://api-edge.cognitive.microsofttranslator.com/translate?from={}&to={}&api-version=3.0",
        from, to
    );
    let payload = serde_json::json!([{ "Text": text }]).to_string();
    let body = net::post_json_bearer(&url, &token, &payload)?;
    let v: Value =
        serde_json::from_str(&body).map_err(|_| crate::i18n::tr("翻译解析失败").to_string())?;
    let out = v[0]["translations"][0]["text"]
        .as_str()
        .unwrap_or("")
        .to_string();
    if out.is_empty() {
        return Err(crate::i18n::tr("翻译结果为空").to_string());
    }
    Ok(out)
}

// MyMemory 翻译服务
fn fallback(text: &str, from: &str, to: &str) -> Result<String, String> {
    if text.trim().is_empty() {
        return Ok(String::new());
    }
    fn mapped(lang: &str) -> &str {
        match lang {
            "zh" => "zh-CN",
            other => other,
        }
    }
    let pair = format!("{}|{}", mapped(from), mapped(to));
    let query = net::query_string(&[
        ("q".to_string(), text.to_string()),
        ("langpair".to_string(), pair),
    ]);
    let body = net::get(
        &format!("https://api.mymemory.translated.net/get?{query}"),
        None,
    )?;
    let value: Value = serde_json::from_str(&body)
        .map_err(|error| crate::i18n::trf("翻译响应解析失败: {}", &[&error]))?;
    value["responseData"]["translatedText"]
        .as_str()
        .filter(|text| !text.is_empty())
        .map(|text| text.to_string())
        .ok_or_else(|| crate::i18n::tr("翻译服务无结果").to_string())
}

fn split_lines(text: &str, count: usize) -> Result<Vec<String>, String> {
    let lines: Vec<String> = text
        .split('\n')
        .map(|line| line.trim().to_string())
        .collect();
    if lines.len() == count {
        Ok(lines)
    } else {
        Err(crate::i18n::tr("翻译结果数量不匹配").to_string())
    }
}

pub fn translate_batch(
    provider: Provider,
    texts: &[String],
    from: &str,
    to: &str,
) -> Result<Vec<String>, String> {
    let clean: Vec<String> = texts.iter().map(|text| text.replace('\n', " ")).collect();
    let joined = clean.join("\n");
    let other = match provider {
        Provider::Google => Provider::Bing,
        Provider::Bing => Provider::Google,
    };
    let parse = |text: String| split_lines(&text, texts.len());
    translate(provider, &joined, from, to)
        .and_then(parse)
        .or_else(|_| fallback(&joined, from, to).and_then(parse))
        .or_else(|_| translate(other, &joined, from, to).and_then(parse))
}
