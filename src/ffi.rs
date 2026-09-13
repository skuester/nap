//! Synchronous, same-thread JSON boundary. Returned strings belong to Rust.
use crate::app::App;
use std::ffi::{CStr, CString, c_char};

#[unsafe(no_mangle)]
pub extern "C" fn nap_core_new() -> *mut App {
    Box::into_raw(Box::default())
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nap_core_free(core: *mut App) {
    if !core.is_null() {
        // SAFETY: Qt returns exactly one pointer created by nap_core_new.
        drop(unsafe { Box::from_raw(core) });
    }
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nap_core_request(core: *mut App, request: *const c_char) -> *mut c_char {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        if core.is_null() || request.is_null() {
            return Err("null core request".to_string());
        }
        // SAFETY: the adapter keeps its NUL-terminated QByteArray and exclusive core alive for this call.
        let text = unsafe { CStr::from_ptr(request) }.to_str().map_err(|e| e.to_string())?;
        let value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        unsafe { &mut *core }.dispatch(&value)
    }))
    .unwrap_or_else(|_| Err("internal core error".into()));
    let value = result.unwrap_or_else(|error| serde_json::json!({"error": error}));
    CString::new(value.to_string()).expect("JSON escapes NUL").into_raw()
}

#[unsafe(no_mangle)]
pub unsafe extern "C" fn nap_string_free(text: *mut c_char) {
    if !text.is_null() {
        // SAFETY: only the strings returned by nap_core_request are passed here, once.
        drop(unsafe { CString::from_raw(text) });
    }
}
