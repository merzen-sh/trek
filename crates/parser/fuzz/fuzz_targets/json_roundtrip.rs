#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Interpret fuzz input as UTF-8 JSON → convert to Lua → back to JSON.
    if let Ok(json_str) = std::str::from_utf8(data) {
        if let Ok(lua) = parser::json_to_lua(json_str) {
            if let Ok(json2) = parser::lua_to_json(&lua) {
                // Verify the re-serialised JSON is valid.
                let _: serde_json::Value = serde_json::from_str(&json2).unwrap();
            }
        }
    }
});
