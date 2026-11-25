//! 通过 C ABI 调用 voice-asr 动态库，实例随 Recognizer 释放

use std::ffi::{c_char, c_void, CStr, CString};
use std::fmt;
use std::ptr;

mod ffi {
    use super::{c_char, c_void};

    #[link(name = "voice_asr")]
    unsafe extern "C" {
        pub fn voice_asr_create(config: *const c_char, out: *mut *mut c_void) -> i32;
        pub fn voice_asr_start(
            handle: *mut c_void,
            options: *const c_char,
            session: *mut u64,
        ) -> i32;
        pub fn voice_asr_feed_audio(handle: *mut c_void, pcm: *const u8, length: usize) -> i32;
        pub fn voice_asr_stop(handle: *mut c_void) -> i32;
        pub fn voice_asr_cancel(handle: *mut c_void) -> i32;
        pub fn voice_asr_poll_event(
            handle: *mut c_void,
            timeout_ms: u32,
            json: *mut *mut c_char,
        ) -> i32;
        pub fn voice_asr_string_free(value: *mut c_char);
        pub fn voice_asr_version() -> *const c_char;
        pub fn voice_asr_last_error() -> *const c_char;
        pub fn voice_asr_destroy(handle: *mut c_void) -> i32;
    }
}

pub const BAD_ARGUMENT: i32 = 1001;
pub const BUSY: i32 = 1002;
pub const NOT_ACTIVE: i32 = 1003;
pub const UNSUPPORTED: i32 = 1004;
pub const AUDIO_ERROR: i32 = 2001;
pub const ENGINE_ERROR: i32 = 2002;
pub const INTERNAL_ERROR: i32 = 2003;

#[derive(Debug, Clone)]
pub struct Error {
    pub code: i32,
    /// 大写下划线形式的错误标识，如 `NO_ACTIVE_SESSION`
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for Error {}

fn check(status: i32) -> Result<i32, Error> {
    if status >= BAD_ARGUMENT {
        let message = unsafe { CStr::from_ptr(ffi::voice_asr_last_error()) };
        return Err(Error {
            code: status,
            message: message.to_string_lossy().into_owned(),
        });
    }
    Ok(status)
}

fn c_string(value: &str) -> Result<CString, Error> {
    CString::new(value).map_err(|_| Error {
        code: BAD_ARGUMENT,
        message: "STRING_CONTAINS_NUL".into(),
    })
}

pub fn version() -> String {
    unsafe { CStr::from_ptr(ffi::voice_asr_version()) }
        .to_string_lossy()
        .into_owned()
}

/// 识别实例，可通过 `Arc` 在多个线程间共享
pub struct Recognizer {
    handle: *mut c_void,
}

// 除 destroy 外各函数可多线程并发调用，destroy 在 Drop 中调用
unsafe impl Send for Recognizer {}
unsafe impl Sync for Recognizer {}

impl Recognizer {
    pub fn new(config_json: &str) -> Result<Self, Error> {
        let config = c_string(config_json)?;
        let mut handle = ptr::null_mut();
        check(unsafe { ffi::voice_asr_create(config.as_ptr(), &mut handle) })?;
        Ok(Self { handle })
    }

    pub fn start(&self, options_json: &str) -> Result<u64, Error> {
        let options = c_string(options_json)?;
        let mut session = 0;
        check(unsafe { ffi::voice_asr_start(self.handle, options.as_ptr(), &mut session) })?;
        Ok(session)
    }

    pub fn feed_audio(&self, pcm: &[u8]) -> Result<(), Error> {
        check(unsafe { ffi::voice_asr_feed_audio(self.handle, pcm.as_ptr(), pcm.len()) })?;
        Ok(())
    }

    pub fn stop(&self) -> Result<(), Error> {
        check(unsafe { ffi::voice_asr_stop(self.handle) })?;
        Ok(())
    }

    pub fn cancel(&self) -> Result<(), Error> {
        check(unsafe { ffi::voice_asr_cancel(self.handle) })?;
        Ok(())
    }

    /// 返回 JSON 事件字符串，超时返回 `None`
    pub fn poll_event(&self, timeout_ms: u32) -> Result<Option<String>, Error> {
        let mut json = ptr::null_mut();
        if check(unsafe { ffi::voice_asr_poll_event(self.handle, timeout_ms, &mut json) })? == 0 {
            return Ok(None);
        }
        let text = unsafe { CStr::from_ptr(json) }
            .to_string_lossy()
            .into_owned();
        unsafe { ffi::voice_asr_string_free(json) };
        Ok(Some(text))
    }
}

impl Drop for Recognizer {
    fn drop(&mut self) {
        unsafe { ffi::voice_asr_destroy(self.handle) };
    }
}
