fn main() {
    let source = include_str!("./lua/full.lua");

    match parser::lua_to_json(&source) {
        Ok(json) => {
            println!("=== JSON Output ===");
            println!("{json}");

            println!("\n=== Round-trip: JSON → Lua ===");
            match parser::json_to_lua(&json) {
                Ok(lua) => {
                    println!("{lua}");

                    println!("\n=== Re-parse round-trip JSON ===");
                    match parser::lua_to_json(&lua) {
                        Ok(json2) => {
                            let v1: serde_json::Value = serde_json::from_str(&json).unwrap();
                            let v2: serde_json::Value = serde_json::from_str(&json2).unwrap();
                            if v1 == v2 {
                                println!("✅ Round-trip successful! JSON matches.");
                            } else {
                                println!("❌ Round-trip JSON mismatch.");
                                println!("--- Original ---\n{json}");
                                println!("--- Re-parsed ---\n{json2}");
                            }
                        }
                        Err(e) => eprintln!("Re-parse error: {e}"),
                    }
                }
                Err(e) => eprintln!("Error: {e}"),
            }
        }
        Err(e) => {
            eprintln!("Error: {e}");
        }
    }
}
