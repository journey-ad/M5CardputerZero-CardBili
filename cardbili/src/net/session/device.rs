use super::{fill_random, insert};
use crate::diagnostics;
use crate::net;

pub fn ensure_buvid(client: &ureq::Agent) -> Result<(), Box<dyn std::error::Error>> {
    if client
        .cookie_jar_lock()
        .get("bilibili.com", "/", "buvid3")
        .is_some()
    {
        return Ok(());
    }
    let mut bytes = [0u8; 20];
    fill_random(&mut bytes)?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes[..16]
        .iter()
        .map(|byte| format!("{byte:02X}"))
        .collect();
    let suffix = u32::from_le_bytes(bytes[16..].try_into().unwrap()) % 100_000;
    let buvid = format!(
        "{}-{}-{}-{}-{}{suffix:05}infoc",
        &hex[..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..]
    );
    insert(client, "buvid3", &buvid)?;
    Ok(())
}

pub fn prepare_anonymous(client: &ureq::Agent) -> Result<(), String> {
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        let text =
            net::get_with_agent(client, "https://api.bilibili.com/x/frontend/finger/spi", 25)?;
        let value: serde_json::Value = serde_json::from_str(&text)?;
        if value["code"].as_i64() != Some(0) {
            return Err(crate::i18n::tr("设备信息获取失败").into());
        }
        for (key, name) in [("b_3", "buvid3"), ("b_4", "buvid4")] {
            let value = value["data"][key]
                .as_str()
                .filter(|value| !value.is_empty())
                .ok_or(crate::i18n::tr("设备信息不完整"))?;
            insert(client, name, value)?;
        }
        insert(client, "b_nut", &net::now_secs().to_string())?;
        Ok(())
    })();
    if let Err(error) = result {
        diagnostics::write(format_args!("[登录] 获取匿名设备信息失败: {error}"));
    }
    ensure_buvid(client).map_err(|error| crate::i18n::trf("设备信息生成失败\n{}", &[&error]))
}

pub fn initialize(client: &ureq::Agent) {
    use base64::Engine;
    let result = (|| -> Result<(), Box<dyn std::error::Error>> {
        ensure_buvid(client)?;
        let mut bytes = [0u8; 44];
        fill_random(&mut bytes)?;
        bytes[32..40].copy_from_slice(&[0, 0, 0, 0, 73, 69, 78, 68]);
        let png_end = base64::engine::general_purpose::STANDARD.encode(bytes);
        let payload = serde_json::json!({
            "3064": 1,
            "39c8": "333.1387.fp.risk",
            "3c43": { "adca": "Linux", "bfe9": &png_end[png_end.len() - 50..] }
        });
        let value = net::post_json_with_agent(
            client,
            "https://api.bilibili.com/x/internal/gaia-gateway/ExClimbWuzhi",
            &net::DEFAULT_HEADERS,
            &serde_json::json!({ "payload": payload.to_string() }),
        )?;
        diagnostics::write(format_args!(
            "[登录] 设备激活 code={:?}",
            value["code"].as_i64()
        ));
        Ok(())
    })();
    if let Err(error) = result {
        diagnostics::write(format_args!("[登录] 设备激活失败: {error}"));
    }
}
