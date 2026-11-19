use crate::net::{
    self,
    session::{self, Credentials, Login},
};
use md5::{Digest, Md5};
use serde_json::Value;

const AUTH_CODE: &str = "https://passport.bilibili.com/x/passport-tv-login/qrcode/auth_code";
const POLL: &str = "https://passport.bilibili.com/x/passport-tv-login/qrcode/poll";
// Android HD 客户端通用签名参数，来源见 https://github.com/bggRGjQaUbCoE/PiliPlus/blob/217fb54f51f6764176319bbdedab2a7522d75401/lib/common/constants.dart#L7
const APP_KEY: &str = "dfca71928277209b";
const APP_SECRET: &str = "b5475a8825547a4fc26c7d518eaaa02e";

fn signed(mut params: Vec<(String, String)>) -> Vec<(String, String)> {
    params.push(("appkey".into(), APP_KEY.into()));
    params.push(("ts".into(), net::now_secs().to_string()));
    params.sort_by(|a, b| a.0.cmp(&b.0));
    let signature = Md5::digest(format!("{}{APP_SECRET}", net::query_string(&params)));
    params.push(("sign".into(), format!("{signature:x}")));
    params
}

pub fn qr_generate(client: &ureq::Agent) -> Result<(String, String), String> {
    session::prepare_anonymous(client)?;
    let params = signed(vec![
        ("local_id".into(), "0".into()),
        ("platform".into(), "android".into()),
        ("mobi_app".into(), "android_hd".into()),
    ]);
    let value = net::post_query_with_agent(client, AUTH_CODE, &params)?;
    if value["code"].as_i64() != Some(0) {
        return Err(crate::i18n::trf(
            "二维码生成失败\ncode={} {}",
            &[&value["code"], &value["message"]],
        ));
    }
    let data = &value["data"];
    let url = data["url"].as_str().unwrap_or_default();
    let key = data["auth_code"].as_str().unwrap_or_default();
    if url.is_empty() || key.is_empty() {
        return Err(crate::i18n::tr("二维码生成失败\n返回为空").into());
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
    let params = signed(vec![
        ("auth_code".into(), key.into()),
        ("local_id".into(), "0".into()),
    ]);
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
        _ => Poll::Err(crate::i18n::trf("登录状态异常\ncode={} {}", &[&code, &value["message"]])),
    }
}

fn login(data: &Value) -> Result<Login, String> {
    let cookies = data["cookie_info"]["cookies"]
        .as_array()
        .ok_or(crate::i18n::tr("登录信息不完整，请重新扫码"))?;
    let token = |name: &str| -> Result<String, String> {
        data[name]
            .as_str()
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .ok_or_else(|| crate::i18n::tr("登录令牌不完整，请重新扫码").into())
    };
    let mut credentials = Credentials {
        access_token: Some(token("access_token")?),
        refresh_token: Some(token("refresh_token")?),
        ..Default::default()
    };
    for cookie in cookies {
        let name = cookie["name"]
            .as_str()
            .ok_or(crate::i18n::tr("登录 Cookie 格式错误，请重新扫码"))?;
        let value = cookie["value"]
            .as_str()
            .ok_or(crate::i18n::tr("登录 Cookie 格式错误，请重新扫码"))?;
        credentials.cookies.insert(name.into(), value.into());
    }
    for required in ["SESSDATA", "bili_jct", "DedeUserID"] {
        if credentials
            .cookies
            .get(required)
            .is_none_or(|value| value.is_empty())
        {
            return Err(crate::i18n::tr("登录信息不完整，请重新扫码").into());
        }
    }
    // 登录账号与扫码前的匿名设备各自维护 Cookie
    let client = net::new_agent();
    for (name, value) in &credentials.cookies {
        session::insert(&client, name, value).map_err(|_| crate::i18n::tr("登录 Cookie 格式错误，请重新扫码"))?;
    }
    session::ensure_buvid(&client).map_err(|_| crate::i18n::tr("设备信息生成失败，请重试"))?;
    session::initialize(&client);
    Ok(Login {
        credentials,
        client,
    })
}
