use super::{fill_random, Credentials};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

fn path() -> PathBuf {
    crate::util::config_dir().join("auth.dat")
}

const STORAGE_MAGIC: &[u8; 4] = b"CBAU";
const STORAGE_VERSION: u8 = 2;
const NONCE_LEN: usize = 24;
const MASTER_KEY_LEN: usize = 32;
const STORAGE_AAD: &[u8] = b"cardbili.auth.v2";

fn master_key_path() -> PathBuf {
    crate::util::state_dir().join("master.key")
}

// 以仅本人可读的权限写入文件，Windows 依赖用户目录自身的访问控制
fn write_private(path: &std::path::Path, bytes: &[u8]) -> std::io::Result<()> {
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(path)?;
    #[cfg(unix)]
    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
    file.write_all(bytes)?;
    Ok(())
}

fn read_key() -> std::io::Result<[u8; MASTER_KEY_LEN]> {
    std::fs::read(master_key_path())?
        .try_into()
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::InvalidData, "主密钥文件长度异常"))
}

// 首次保存时生成 32 字节随机主密钥并单独保存
fn master_key() -> Result<[u8; MASTER_KEY_LEN], Box<dyn std::error::Error>> {
    let path = master_key_path();
    match read_key() {
        Ok(key) => Ok(key),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut key = [0u8; MASTER_KEY_LEN];
            fill_random(&mut key)?;
            std::fs::create_dir_all(path.parent().unwrap())?;
            let temporary = path.with_extension("tmp");
            write_private(&temporary, &key)?;
            std::fs::rename(&temporary, &path)?;
            Ok(key)
        }
        Err(error) => Err(error.into()),
    }
}

// XChaCha20-Poly1305 认证加密，随机 nonce 每次不同，AAD 绑定格式与版本
fn encode(credentials: &Credentials) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let plain = serde_json::to_vec(credentials)?;
    let key = master_key()?;
    let cipher = XChaCha20Poly1305::new(&key.into());
    let mut nonce = [0u8; NONCE_LEN];
    fill_random(&mut nonce)?;
    let ciphertext = cipher
        .encrypt(
            XNonce::from_slice(&nonce),
            Payload {
                msg: &plain,
                aad: STORAGE_AAD,
            },
        )
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::Other, error.to_string()))?;
    let mut packed = Vec::with_capacity(4 + 1 + NONCE_LEN + ciphertext.len());
    packed.extend_from_slice(STORAGE_MAGIC);
    packed.push(STORAGE_VERSION);
    packed.extend_from_slice(&nonce);
    packed.extend_from_slice(&ciphertext);
    Ok(packed)
}

fn invalid_storage() -> Box<dyn std::error::Error> {
    std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        "invalid credential storage",
    )
    .into()
}

fn decode(encoded: &[u8]) -> Result<Credentials, Box<dyn std::error::Error>> {
    if encoded.len() < 4 + 1 + NONCE_LEN + 16
        || !encoded.starts_with(STORAGE_MAGIC)
        || encoded[4] != STORAGE_VERSION
    {
        return Err(invalid_storage());
    }
    let nonce = &encoded[5..5 + NONCE_LEN];
    let ciphertext = &encoded[5 + NONCE_LEN..];
    let key = read_key().map_err(|_| invalid_storage())?;
    let cipher = XChaCha20Poly1305::new(&key.into());
    let plain = cipher
        .decrypt(
            XNonce::from_slice(nonce),
            Payload {
                msg: ciphertext,
                aad: STORAGE_AAD,
            },
        )
        .map_err(|_| invalid_storage())?;
    serde_json::from_slice(&plain).map_err(|_| invalid_storage())
}

pub(super) fn save(credentials: &Credentials) -> Result<(), Box<dyn std::error::Error>> {
    let encoded = encode(credentials)?;
    let path = path();
    std::fs::create_dir_all(path.parent().unwrap())?;
    let temporary = path.with_extension("tmp");
    write_private(&temporary, &encoded)?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

pub(super) fn load() -> Result<Option<Credentials>, Box<dyn std::error::Error>> {
    match std::fs::read(path()) {
        Ok(bytes) => decode(&bytes).map(Some),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error.into()),
    }
}
