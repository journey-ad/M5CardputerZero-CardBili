//! 通过 C ABI 调用 libvoice_asr.so，实例随 Recognizer 释放

use std::ffi::{c_char, c_void, CStr, CString};
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
        pub fn voice_asr_last_error() -> *const c_char;
        pub fn voice_asr_destroy(handle: *mut c_void) -> i32;
    }
}

fn check(status: i32) -> Result<(), String> {
    if status >= 1001 {
        let message = unsafe { CStr::from_ptr(ffi::voice_asr_last_error()) };
        return Err(format!("{status}: {}", message.to_string_lossy()));
    }
    Ok(())
}

pub struct Recognizer {
    handle: *mut c_void,
}

impl Recognizer {
    pub fn new(config_json: &str) -> Result<Self, String> {
        let config = CString::new(config_json).map_err(|e| e.to_string())?;
        let mut handle = ptr::null_mut();
        check(unsafe { ffi::voice_asr_create(config.as_ptr(), &mut handle) })?;
        Ok(Self { handle })
    }

    pub fn start(&mut self, options_json: &str) -> Result<u64, String> {
        let options = CString::new(options_json).map_err(|e| e.to_string())?;
        let mut session = 0;
        check(unsafe { ffi::voice_asr_start(self.handle, options.as_ptr(), &mut session) })?;
        Ok(session)
    }

    pub fn feed_audio(&mut self, pcm: &[u8]) -> Result<(), String> {
        check(unsafe { ffi::voice_asr_feed_audio(self.handle, pcm.as_ptr(), pcm.len()) })
    }

    pub fn stop(&mut self) -> Result<(), String> {
        check(unsafe { ffi::voice_asr_stop(self.handle) })
    }

    pub fn cancel(&mut self) -> Result<(), String> {
        check(unsafe { ffi::voice_asr_cancel(self.handle) })
    }

    pub fn poll_event(&mut self, timeout_ms: u32) -> Result<Option<String>, String> {
        let mut json = ptr::null_mut();
        let status = unsafe { ffi::voice_asr_poll_event(self.handle, timeout_ms, &mut json) };
        check(status)?;
        if status == 0 {
            return Ok(None);
        }
        let text = unsafe { CStr::from_ptr(json) }
            .to_str()
            .map(str::to_owned)
            .map_err(|e| e.to_string());
        unsafe { ffi::voice_asr_string_free(json) };
        text.map(Some)
    }
}

impl Drop for Recognizer {
    fn drop(&mut self) {
        unsafe { ffi::voice_asr_destroy(self.handle) };
    }
}
