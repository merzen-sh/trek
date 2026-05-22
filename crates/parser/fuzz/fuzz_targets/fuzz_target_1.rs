#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // Feed raw Lua source through Lua → JSON → Lua round-trip.
    // If the input is valid-ish Lua, the round-trip must not panic.
    if let Ok(lua_str) = std::str::from_utf8(data) {
        if let Ok(json) = parser::lua_to_json(lua_str) {
            if let Ok(lua2) = parser::json_to_lua(&json) {
                // Re-parse the output Lua to ensure it remains valid.
                let _ = parser::lua_to_json(&lua2);
            }
        }
    }
});
