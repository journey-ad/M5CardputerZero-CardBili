use crate::net::{
    self,
    session::{self, Credentials, Login},
};
use md5::{Digest, Md5};
use serde_json::Value;

const AUTH_CODE: &str = "https://passport.bilibili.com/x/passport-tv-login/qrcode/auth_code";
const POLL: &str = "https://passport.bilibili.com/x/passport-tv-login/qrcode/poll";
fn signed(mut params: Vec<(String, String)>) -> Result<Vec<(String, String)>, String> {
    let credential = |name| {
        std::env::var(name)
            .ok()
            .filter(|value| !value.trim().is_empty())
            .ok_or_else(|| "请配置 BILI_APP_KEY 和 BILI_APP_SECRET 后登录".to_string())
    };
    let app_key = credential("BILI_APP_KEY")?;
    let app_secret = credential("BILI_APP_SECRET")?;
    params.push(("appkey".into(), app_key));
    params.push(("ts".into(), net::now_secs().to_string()));
    params.sort_by(|a, b| a.0.cmp(&b.0));
    let signature = Md5::digest(format!("{}{app_secret}", net::query_string(&params)));
    params.push(("sign".into(), format!("{signature:x}")));
    Ok(params)
}

pub fn qr_generate(client: &ureq::Agent) -> Result<(String, String), String> {
    session::prepare_anonymous(client)?;
    let params = signed(vec![
        ("local_id".into(), "0".into()),
        ("platform".into(), "android".into()),
        ("mobi_app".into(), "android_hd".into()),
    ])?;
    let value = net::post_query_with_agent(client, AUTH_CODE, &params)?;
    if value["code"].as_i64() != Some(0) {
        return Err(format!(
            "二维码生成失败\ncode={} {}",
            value["code"], value["message"]
        ));
    }
    let data = &value["data"];
    let url = data["url"].as_str().unwrap_or_default();
    let key = data["auth_code"].as_str().unwrap_or_default();
    if url.is_empty() || key.is_empty() {
        return Err("二维码生成失败\n返回为空".into());
    }
    Ok((url.into(), key.into()))
}

pub enum Poll {
    Success(Login),
    Scanned,
    Pending,
    Expired,
    Err(String),
}

pub fn qr_poll(key: &str, client: &ureq::Agent) -> Poll {
    let params = match signed(vec![
        ("auth_code".into(), key.into()),
        ("local_id".into(), "0".into()),
    ]) {
        Ok(params) => params,
        Err(error) => return Poll::Err(error),
    };
    let value = match net::post_query_with_agent(client, POLL, &params) {
        Ok(value) => value,
        Err(error) => return Poll::Err(error),
    };
    let code = value["code"].as_i64().unwrap_or(-1);
    crate::diagnostics::write(format_args!("[登录] TV/HD 二维码状态 code={code}"));
    match code {
        0 => match login(&value["data"]) {
            Ok(login) => Poll::Success(login),
            Err(error) => Poll::Err(error),
        },
        86039 => Poll::Pending,
        86090 => Poll::Scanned,
        86038 => Poll::Expired,
        _ => Poll::Err(format!("登录状态异常\ncode={code} {}", value["message"])),
    }
}

fn login(data: &Value) -> Result<Login, String> {
    let cookies = data["cookie_info"]["cookies"]
        .as_array()
        .ok_or("登录信息不完整，请重新扫码")?;
    let token = |name: &str| -> Result<String, String> {
        data[name]
            .as_str()
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| "登录令牌不完整，请重新扫码".into())
    };
    let mut credentials = Credentials {
        access_token: Some(token("access_token")?),
        refresh_token: Some(token("refresh_token")?),
        ..Default::default()
    };
    for cookie in cookies {
        let name = cookie["name"]
            .as_str()
            .ok_or("登录 Cookie 格式错误，请重新扫码")?;
        let value = cookie["value"]
            .as_str()
            .ok_or("登录 Cookie 格式错误，请重新扫码")?;
        credentials.cookies.insert(name.into(), value.into());
    }
    for required in ["SESSDATA", "bili_jct", "DedeUserID"] {
        if credentials
            .cookies
            .get(required)
            .is_none_or(|value| value.is_empty())
        {
            return Err("登录信息不完整，请重新扫码".into());
        }
    }
    // 登录账号与扫码前的匿名设备各自维护 Cookie
    let client = net::new_agent();
    for (name, value) in &credentials.cookies {
        session::insert(&client, name, value).map_err(|_| "登录 Cookie 格式错误，请重新扫码")?;
    }
    session::ensure_buvid(&client).map_err(|_| "设备信息生成失败，请重试")?;
    session::initialize(&client);
    Ok(Login {
        credentials,
        client,
    })
}
