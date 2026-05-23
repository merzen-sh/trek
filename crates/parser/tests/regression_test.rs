use parser::{lua_to_json, json_to_lua, ConfigSession};

#[test]
fn test_float_number_discrimination() {
    let json = lua_to_json(r#"config = {
    int_val = 42,
    float_val = 0.0,
    another_float = 3.14,
    big_int = 1000,
}"#).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
    let fields = doc["fields"].as_object().unwrap();
    assert_eq!(fields["int_val"]["type"], "number");
    assert_eq!(fields["int_val"]["value"], "42");
    assert_eq!(fields["big_int"]["type"], "number");
    assert_eq!(fields["big_int"]["value"], "1000");
    assert_eq!(fields["float_val"]["type"], "float");
    assert_eq!(fields["float_val"]["value"], "0.0");
    assert_eq!(fields["another_float"]["type"], "float");
    assert_eq!(fields["another_float"]["value"], "3.14");

    // Round-trip: JSON -> Lua -> JSON preserves float/number distinction
    let lua = json_to_lua(&json).unwrap();
    let json2 = lua_to_json(&lua).unwrap();
    let doc2: serde_json::Value = serde_json::from_str(&json2).unwrap();
    assert_eq!(doc, doc2);
}

#[test]
fn test_range_with_named_keys() {
    let json = lua_to_json(r#"config = {
    --@RANGE = { min = 1, max = 256 }
    max_players = 64,
}"#).unwrap();
    let doc: serde_json::Value = serde_json::from_str(&json).unwrap();
    let field = &doc["fields"]["max_players"];
    assert_eq!(field["type"], "number");
    assert_eq!(field["value"], "64");
    let meta = field["metadata"].as_object().unwrap();
    let range = meta["range"].as_array().unwrap();
    assert_eq!(range, &[serde_json::json!("1"), serde_json::json!("256")]);

    // Round-trip preserves range values with named keys
    let lua = json_to_lua(&json).unwrap();
    assert!(lua.contains("--@RANGE = { min = 1, max = 256 }"));
}

#[test]
fn test_lossless_print_regression() {
    let original = r#"config = {
    --!Server configuration root
    --[[@TABLE = {
        allow_edit = false,
        schema = {
            { name = "key", type = "string", label = "Config key" },
            { name = "value", type = "string", label = "Config value" },
        }
    }]]
    server = {
        --!Basic server info
        identifier = "srv_01",
        --!Server region
        --@ENUM = { "us-east", "eu-west", "ap-southeast" }
        region = "us-east",
        --!Max player count
        --@RANGE = { min = 1, max = 256 }
        max_players = 64,
        --!Features enabled
        features = {
            --!Anti-cheat system
            anti_cheat = true,
            --!Voice chat
            voice = false,
        },
    },

    --!Economy system
    --[[@TABLE = {
        allow_add = true,
        allow_delete = true,
        schema = {
            { name = "id", type = "key", label = "Item ID" },
            { name = "name", type = "string", label = "Item name" },
            { name = "category", type = "enum", label = "Category", values = { "weapon", "food", "vehicle", "clothing" } },
            { name = "price", type = "number", label = "Price" },
        }
    }]]
    economy = {
        --!Shop items
        items = {
            --!Weapon category
            pistol = {
                name = "Pistol",
                category = "weapon",
                price = 500,
            },
        },
    },

    --!Discord integration
    discord = {
        --!Bot token webhook
        --[[@CFX_FUNCTION = {
              args_schema = {
                { name = "message", type = "string", label = "Message content", required = true },
                { name = "channel_id", type = "string", label = "Target channel", required = true },
              }
          }]]
        send_message = function(msg, channel)
            exports.discord:send(channel, msg)
        end,
    },

    --!Player defaults
    player_defaults = {
        --!Spawn location
        --@MAP = true
        spawn = vector3(-150.0, 50.0, 28.0),
        --!Starting money
        --@RANGE = { min = 0, max = 100000 }
        start_money = 1000,
    },
}
"#;

    let session = ConfigSession::from_source(original).expect("parse session");
    let result = session.print();

    assert!(
        result.contains("exports.discord:send(channel, msg)"),
        "function body lost"
    );
    assert!(
        result.contains("function(msg, channel)"),
        "function parameters lost"
    );
    assert!(
        result.contains("--@RANGE = { min = 1, max = 256 }"),
        "@RANGE min/max lost"
    );
    assert!(
        result.contains("--@RANGE = { min = 0, max = 100000 }"),
        "@RANGE min/max lost"
    );
    assert!(result.contains("--@MAP = true"), "@MAP lost");
    assert!(result.contains(r#"name = "key""#), "schema column name lost");
    assert!(result.contains(r#"name = "id""#), "schema column name lost");

    let session2 = ConfigSession::from_source(&result).expect("re-parse session");
    assert_eq!(result, session2.print(), "lossless print not stable");
}

#[test]
fn test_layout_has_ast_paths() {
    let source = r#"config = {
    server = {
        identifier = "srv_01",
    },
}"#;
    let session = ConfigSession::from_source(source).unwrap();
    let layout = session.get_layout();
    let server = layout.fields.get("server").expect("server field");
    match server {
        parser::models::LayoutNode::Table(t) => {
            assert_eq!(t.ast_path, vec!["server"]);
            let id = t.fields.get("identifier").expect("identifier");
            match id {
                parser::models::LayoutNode::String(s) => {
                    assert_eq!(s.ast_path, vec!["server", "identifier"]);
                }
                other => panic!("expected string layout, got {other:?}"),
            }
        }
        other => panic!("expected table layout, got {other:?}"),
    }
}

#[test]
fn test_string_patch_preserves_quotes() {
    let source = r#"config = {
    identifier = "srv_01",
}"#;
    let mut session = ConfigSession::from_source(source).expect("session");
    session
        .patch_value_at_path(&["identifier".into()], &serde_json::json!("jhghj"))
        .expect("patch");
    let out = session.print();
    assert!(
        out.contains("identifier = \"jhghj\""),
        "string field must be quoted, got:\n{out}"
    );
    assert!(
        !out.contains("identifier = jhghj"),
        "must not emit bare identifier for string value:\n{out}"
    );
}

#[test]
fn test_table_row_append_and_remove() {
    let source = r#"config = {
    items = {
        --[[@TABLE = {
            allow_add = true,
            allow_delete = true,
            schema = {
                { name = "id", type = "key", label = "ID" },
                { name = "name", type = "string", label = "Name" },
            }
        }]]
        pistol = {
            name = "Pistol",
        },
    },
}"#;
    let mut session = ConfigSession::from_source(source).expect("session");

    session
        .patch_table_append(
            &["items".into()],
            "rifle",
            &serde_json::json!({ "name": "Rifle" }),
        )
        .expect("append");
    let out = session.print();
    assert!(out.contains("rifle"), "new row key missing:\n{out}");
    assert!(out.contains("Rifle"), "new row value missing:\n{out}");

    session
        .patch_table_remove_row(&["items".into()], "pistol")
        .expect("remove");
    let out2 = session.print();
    assert!(!out2.contains("pistol"), "row should be removed:\n{out2}");
}

#[test]
fn test_get_value_at_path() {
    let source = r#"config = {
    server = {
        identifier = "srv_01",
    },
}"#;
    let session = ConfigSession::from_source(source).unwrap();
    let value = session
        .get_value_at_path(&["server".into(), "identifier".into()])
        .unwrap();
    assert_eq!(value, serde_json::json!("srv_01"));
}
