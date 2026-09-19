//! The C ABI the Qt adapter calls, exercised the way the adapter uses it.
use nap::ffi::{nap_analyze, nap_core_free, nap_core_new, nap_core_request, nap_string_free};
use serde_json::{Value, json};
use std::ffi::{CStr, CString};
use std::ptr;

fn request(core: *mut nap::app::App, body: &Value) -> Value {
    let text = CString::new(body.to_string()).unwrap();
    // SAFETY: the core is live, the request is NUL-terminated, and the reply is freed exactly once.
    unsafe {
        let reply = nap_core_request(core, text.as_ptr());
        let value = serde_json::from_str(CStr::from_ptr(reply).to_str().unwrap()).unwrap();
        nap_string_free(reply);
        value
    }
}

#[test]
fn requests_answer_in_json_and_never_unwind() {
    let core = nap_core_new();
    assert_eq!(request(core, &json!({"op":"volume", "volume": 7}))["volume"], 1.0);
    assert_eq!(request(core, &json!({"op":"nonsense"}))["error"], "unknown core operation");
    assert_eq!(request(core, &json!({"op":"insert"}))["error"], "No file loaded");
    // SAFETY: null and malformed requests are part of the contract; frees accept null.
    unsafe {
        let garbled = CString::new("{not json").unwrap();
        for reply in [
            nap_core_request(core, garbled.as_ptr()),
            nap_core_request(core, ptr::null()),
            nap_core_request(ptr::null_mut(), garbled.as_ptr()),
        ] {
            let value: Value = serde_json::from_str(CStr::from_ptr(reply).to_str().unwrap()).unwrap();
            assert!(value["error"].is_string());
            nap_string_free(reply);
        }
        nap_string_free(ptr::null_mut());
        nap_core_free(ptr::null_mut());
        nap_core_free(core);
    }
}

#[test]
fn analysis_fills_bars_and_levels() {
    let tone: Vec<f32> = (0..2048).map(|i| 0.5 * (i as f32 * 0.13).sin()).collect();
    let quiet = vec![0.0f32; 2048];
    let (mut bars, mut levels) = ([0f32; 24], [0f32; 4]);
    // SAFETY: every pointer names a live buffer of the stated length.
    unsafe { nap_analyze(tone.as_ptr(), quiet.as_ptr(), 2048, 48000, bars.as_mut_ptr(), 24, levels.as_mut_ptr()) };
    assert!(bars.iter().any(|b| *b > 0.5));
    assert!(levels[0] > 0.5 && levels[1] == 0.0 && levels[2] > 0.5 && levels[3] == 0.0, "{levels:?}");
    let untouched = bars;
    // SAFETY: null inputs are rejected before anything is read or written.
    unsafe { nap_analyze(ptr::null(), quiet.as_ptr(), 2048, 48000, bars.as_mut_ptr(), 24, levels.as_mut_ptr()) };
    assert_eq!(bars, untouched);
}
