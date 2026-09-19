//! Synchronous, same-thread JSON boundary. Returned strings belong to Rust.
use crate::app::App;
use std::ffi::{CStr, CString, c_char};

#[unsafe(no_mangle)]
pub extern "C" fn nap_core_new() -> *mut App {
    Box::into_raw(Box::default())
}

/// # Safety
/// `core` must be null or a pointer from [`nap_core_new`] that has not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nap_core_free(core: *mut App) {
    if !core.is_null() {
        // SAFETY: Qt returns exactly one pointer created by nap_core_new.
        drop(unsafe { Box::from_raw(core) });
    }
}

/// # Safety
/// `core` must be null or a live pointer from [`nap_core_new`] that nothing else is using, and
/// `request` null or a NUL-terminated string. The reply must go back to [`nap_string_free`].
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

/// Analyze one buffer for the visualizers: `bands` spectrum bars from the channel mix, and into
/// `levels` the left and right VU deflections followed by the left and right ladder levels.
/// Hot path, so it bypasses the JSON boundary.
///
/// # Safety
/// `left` and `right` must each point to `frames` samples, `bands` to `band_count` writable
/// floats, and `levels` to four; null pointers are ignored.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nap_analyze(
    left: *const f32,
    right: *const f32,
    frames: usize,
    rate: u32,
    bands: *mut f32,
    band_count: usize,
    levels: *mut f32,
) {
    if left.is_null() || right.is_null() || bands.is_null() || levels.is_null() {
        return;
    }
    // SAFETY: the adapter passes two `frames`-long channel arrays, a `band_count`-long output and
    // a four-element output, all alive and unaliased for the duration of this call.
    let (left, right, bands, levels) = unsafe {
        (
            std::slice::from_raw_parts(left, frames),
            std::slice::from_raw_parts(right, frames),
            std::slice::from_raw_parts_mut(bands, band_count),
            std::slice::from_raw_parts_mut(levels, 4),
        )
    };
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mix: Vec<f32> = left.iter().zip(right).map(|(l, r)| (l + r) / 2.0).collect();
        bands.copy_from_slice(&crate::meter::spectrum(&mix, rate, band_count));
        use crate::meter::{ladder, vu};
        levels.copy_from_slice(&[vu(left), vu(right), ladder(left), ladder(right)]);
    }));
}

/// # Safety
/// `text` must be null or a string returned by [`nap_core_request`], freed only once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nap_string_free(text: *mut c_char) {
    if !text.is_null() {
        // SAFETY: only the strings returned by nap_core_request are passed here, once.
        drop(unsafe { CString::from_raw(text) });
    }
}
