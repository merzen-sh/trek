use proptest::prelude::*;
use std::collections::HashMap;
use parser::models::*;
use parser::{json_to_lua, lua_to_json};

// ===========================================================================
// Normalization helpers — collapse semantically-empty metadata to None
// so round-trip comparison works (Lua has no way to represent empty meta).
// ===========================================================================

fn desc_empty(v: &[String]) -> bool {
    v.is_empty() || v.iter().all(|s| s.trim().is_empty())
}

fn norm_scalar(meta: &mut Option<ScalarMeta>) {
    if let Some(m) = meta {
        if m.description.as_ref().is_some_and(|v| desc_empty(v)) {
            m.description = None;
        }
        if m.range.as_ref().is_some_and(|v| v.is_empty()) {
            m.range = None;
        }
        if m.range.is_none() && m.description.is_none() {
            *meta = None;
        }
    }
}

fn norm_enum(meta: &mut Option<EnumMeta>) {
    if let Some(m) = meta {
        if m.description.as_ref().is_some_and(|v| desc_empty(v)) {
            m.description = None;
        }
    }
}

fn norm_table_meta(meta: &mut Option<TableMeta>) {
    if let Some(m) = meta {
        if m.description.as_ref().is_some_and(|v| desc_empty(v)) {
            m.description = None;
        }
        if m.schema.is_none() && m.description.is_none() {
            *meta = None;
        }
    }
}

fn norm_node(node: &mut ConfigNode) {
    match node {
        ConfigNode::String(n) => norm_scalar(&mut n.metadata),
        ConfigNode::Number(n) => norm_scalar(&mut n.metadata),
        ConfigNode::Float(n) => norm_scalar(&mut n.metadata),
        ConfigNode::Boolean(n) => norm_scalar(&mut n.metadata),
        ConfigNode::Vector2(n) => norm_scalar(&mut n.metadata),
        ConfigNode::Vector3(n) => norm_scalar(&mut n.metadata),
        ConfigNode::Enum(n) => norm_enum(&mut n.metadata),
        ConfigNode::Table(n) => {
            norm_table_meta(&mut n.metadata);
            for (_, child) in &mut n.rows {
                norm_node(child);
            }
        }
        ConfigNode::CfxFunction(_) => {}
    }
}

fn normalize(doc: &mut ConfigDoc) {
    if let Some(m) = &doc.meta {
        if m.is_empty() {
            doc.meta = None;
        }
    }
    for (_, node) in &mut doc.fields {
        norm_node(node);
    }
}

// ===========================================================================
// Strategies
// ===========================================================================

fn description_strategy() -> impl Strategy<Value = Option<Vec<String>>> {
    // Pre-trim each line — the parser strips leading/trailing whitespace
    // when extracting descriptions from Lua single-line comments.
    proptest::option::of(
        proptest::collection::vec("[ -~]{1,64}", 1..3)
            .prop_map(|v| v.into_iter().map(|s| s.trim().to_string()).collect()),
    )
}

fn range_strategy() -> impl Strategy<Value = Option<Vec<String>>> {
    // Use non-negative numbers only — Lua parses `-0` as a UnaryOperator
    // expression whose to_string() includes trivia whitespace, breaking
    // round-trip fidelity.
    proptest::option::of(proptest::collection::vec(
        "[0-9]+(\\.[0-9]+)?",
        1..3,
    ))
}

fn scalar_meta_strategy() -> impl Strategy<Value = Option<ScalarMeta>> {
    (description_strategy(), range_strategy()).prop_map(|(description, range)| {
        let m = ScalarMeta { description, range };
        if m.description.is_none() && m.range.is_none() {
            None
        } else {
            Some(m)
        }
    })
}

fn string_node() -> impl Strategy<Value = ConfigNode> {
    // Avoid chars needing Lua escaping (quotes, backslashes, newlines, tabs).
    (
        prop_oneof![
            "[a-zA-Z0-9_ ]{0,16}",
            Just(String::new()),
        ],
        scalar_meta_strategy(),
    )
        .prop_map(
            |(value, metadata)| ConfigNode::String(ScalarNode { value, metadata }),
        )
}

fn number_node() -> impl Strategy<Value = ConfigNode> {
    (
        prop_oneof![
            Just("0".to_string()),
            "[1-9][0-9]{0,3}".prop_map(|s: String| s),
        ],
        scalar_meta_strategy(),
    )
        .prop_map(
            |(value, metadata)| ConfigNode::Number(ScalarNode { value, metadata }),
        )
}

fn float_node() -> impl Strategy<Value = ConfigNode> {
    (
        prop_oneof![
            Just("0.0".to_string()),
            "[0-9]+\\.[0-9]+".prop_map(|s: String| s),
        ],
        scalar_meta_strategy(),
    )
        .prop_map(
            |(value, metadata)| ConfigNode::Float(ScalarNode { value, metadata }),
        )
}

fn boolean_node() -> impl Strategy<Value = ConfigNode> {
    // Booleans only support description (range/duration are semantically invalid).
    (proptest::bool::ANY, description_strategy()).prop_map(
        |(value, description)| {
            let metadata = description.map(|d| ScalarMeta { description: Some(d), range: None });
            ConfigNode::Boolean(ScalarNode { value, metadata })
        },
    )
}

fn enum_node() -> impl Strategy<Value = ConfigNode> {
    // Enum must always carry metadata (options); otherwise the round-trip
    // collapses it into a plain String since there's no @ENUM annotation.
    // Use alphanumeric chars to avoid Lua escaping edge cases in annotations.
    (
        "[a-zA-Z0-9_]{0,8}",
        description_strategy(),
        proptest::collection::vec("[a-zA-Z0-9_]{1,8}", 1..5),
    )
        .prop_map(|(value, description, options)| {
            ConfigNode::Enum(EnumNode {
                value,
                metadata: Some(EnumMeta { description, options }),
            })
        })
}

fn float_value() -> impl Strategy<Value = f64> {
    // Use only f64 values with exact binary representations to avoid
    // precision loss through serde_json's float formatting (which does
    // not guarantee round-trip fidelity for all f64 values).
    prop_oneof![
        Just(0.0f64),
        Just(1.0f64),
        Just(-1.0f64),
        // Integers — always exact
        (-1000i64..1000).prop_map(|i| i as f64),
        // Halves — exact in IEEE 754
        (-1000i64..1000).prop_map(|i| i as f64 * 0.5),
    ]
}

const LUA_KW: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for",
    "function", "if", "in", "local", "nil", "not", "or", "repeat",
    "return", "then", "true", "until", "while",
];

fn vector2_node() -> impl Strategy<Value = ConfigNode> {
    (float_value(), float_value(), scalar_meta_strategy()).prop_map(
        |(x, y, metadata)| {
            ConfigNode::Vector2(Vector2Node {
                value: Vector2Value { x, y },
                metadata,
            })
        },
    )
}

fn vector3_node() -> impl Strategy<Value = ConfigNode> {
    (float_value(), float_value(), float_value(), scalar_meta_strategy()).prop_map(
        |(x, y, z, metadata)| {
            ConfigNode::Vector3(Vector3Node {
                value: Vector3Value { x, y, z },
                metadata,
            })
        },
    )
}

fn column_type() -> impl Strategy<Value = ColumnType> {
    prop_oneof![
        Just(ColumnType::Key),
        Just(ColumnType::String),
        Just(ColumnType::Number),
        Just(ColumnType::Enum),
        Just(ColumnType::Boolean),
        Just(ColumnType::Unknown),
    ]
}

fn column_def() -> impl Strategy<Value = ColumnDef> {
    (
        "[a-z_][a-z0-9_]{0,7}",
        column_type(),
        "[a-zA-Z0-9_ ]{0,16}",
        proptest::collection::vec("[a-zA-Z0-9_ ]{1,8}", 0..3),
    )
        .prop_map(
            |(field, col_type, label, values)| ColumnDef {
                field,
                col_type,
                label,
                values,
            },
        )
}

fn table_schema() -> impl Strategy<Value = TableSchema> {
    (
        proptest::bool::ANY,
        proptest::bool::ANY,
        proptest::bool::ANY,
        proptest::collection::vec(column_def(), 0..3),
    )
        .prop_map(|(allow_add, allow_delete, allow_edit, columns)| TableSchema {
            allow_add,
            allow_delete,
            allow_edit,
            columns,
        })
}

fn table_meta_strategy() -> impl Strategy<Value = Option<TableMeta>> {
    (description_strategy(), proptest::option::of(table_schema())).prop_map(
        |(description, schema)| {
            let m = TableMeta { description, schema };
            if m.description.is_none() && m.schema.is_none() {
                None
            } else {
                Some(m)
            }
        },
    )
}

fn arg_type() -> impl Strategy<Value = ColumnType> {
    prop_oneof![
        Just(ColumnType::String),
        Just(ColumnType::Number),
        Just(ColumnType::Boolean),
        Just(ColumnType::Enum),
        Just(ColumnType::Key),
        Just(ColumnType::Unknown),
    ]
}

fn arg_def() -> impl Strategy<Value = ArgDef> {
    (
        "[a-z_][a-z0-9_]{0,7}",
        arg_type(),
        "[a-zA-Z0-9_ ]{0,16}",
        proptest::bool::ANY,
    )
        .prop_map(|(name, arg_type, label, required)| ArgDef {
            name,
            arg_type,
            label,
            required,
        })
}

fn cfx_meta_strategy() -> impl Strategy<Value = CfxFunctionMeta> {
    (
        description_strategy(),
        proptest::collection::vec(arg_def(), 0..3),
    )
        .prop_map(|(description, args_schema)| CfxFunctionMeta {
            description,
            args_schema,
        })
}

fn cfx_node() -> impl Strategy<Value = ConfigNode> {
    cfx_meta_strategy().prop_map(|metadata| {
        ConfigNode::CfxFunction(CfxFunctionNode { metadata })
    })
}

fn leaf_node() -> impl Strategy<Value = ConfigNode> {
    prop_oneof![
        string_node(),
        number_node(),
        float_node(),
        boolean_node(),
        enum_node(),
        vector2_node(),
        vector3_node(),
        cfx_node(),
    ]
}

/// Shallow table (leaf children only) — avoids stack-overflow from deep
/// PartialEq recursion in the assertion.
fn flat_table_node() -> impl Strategy<Value = ConfigNode> {
    let key = "[a-z_][a-z0-9_]{0,7}".prop_filter("not keyword", |s| !LUA_KW.contains(&s.as_str()));
    (
        table_meta_strategy(),
        proptest::collection::vec(
            (key, leaf_node()),
            0..4,
        ),
    )
        .prop_map(|(metadata, entries)| {
            let rows = deduplicate_keys(entries);
            ConfigNode::Table(TableNode { rows, metadata })
        })
}

/// Deep table — children can themselves be tables (2 levels max).
fn deep_table_node(depth: u32) -> impl Strategy<Value = ConfigNode> {
    let child: BoxedStrategy<ConfigNode> = if depth == 0 {
        leaf_node().boxed()
    } else {
        prop_oneof![leaf_node(), deep_table_node(depth - 1)].boxed()
    };

    let key = "[a-z_][a-z0-9_]{0,3}".prop_filter("not keyword", |s| !LUA_KW.contains(&s.as_str()));
    (
        table_meta_strategy(),
        proptest::collection::vec(
            (key, child),
            0..3,
        ),
    )
        .prop_map(|(metadata, entries)| {
            let rows = deduplicate_keys(entries);
            ConfigNode::Table(TableNode { rows, metadata })
        })
        .boxed()
}

fn deduplicate_keys(entries: Vec<(String, ConfigNode)>) -> ConfigIR {
    let mut seen = HashMap::new();
    let mut out = ConfigIR::new();
    for (k, v) in entries {
        if seen.contains_key(&k) {
            continue;
        }
        seen.insert(k.clone(), ());
        out.insert(k, v);
    }
    out
}

fn top_level_ir() -> impl Strategy<Value = ConfigIR> {
    let k7 = "[a-z_][a-z0-9_]{0,7}".prop_filter("not keyword", |s| !LUA_KW.contains(&s.as_str()));
    let k3 = "[a-z_][a-z0-9_]{0,3}".prop_filter("not keyword", |s| !LUA_KW.contains(&s.as_str()));
    prop_oneof![
        // flat only
        proptest::collection::vec(
            (k7.clone(), leaf_node()),
            0..5,
        ).prop_map(deduplicate_keys),
        // flat + shallow tables
        proptest::collection::vec(
            (k7.clone(),
            prop_oneof![leaf_node(), flat_table_node()]),
            0..4,
        ).prop_map(deduplicate_keys),
        // deep tables (2 level nesting)
        proptest::collection::vec(
            (k3.clone(),
            prop_oneof![leaf_node(), deep_table_node(1)]),
            0..3,
        ).prop_map(deduplicate_keys),
    ]
}

fn meta_value() -> impl Strategy<Value = serde_json::Value> {
    // Only simple strings round-trip faithfully through Lua meta comments.
    // full_moon preserves escape sequences literally, so chars needing
    // Lua escaping (quotes, backslashes, newlines) change representation.
    "[a-zA-Z0-9_ ]{0,16}".prop_map(|s| serde_json::Value::String(s))
}

fn config_doc() -> impl Strategy<Value = ConfigDoc> {
    let meta_key = "[a-z_][a-z0-9_]{0,7}".prop_filter("not keyword", |s| !LUA_KW.contains(&s.as_str()));
    (
        proptest::option::of(
            proptest::collection::vec(
                (meta_key, meta_value()),
                0..3,
            )
            .prop_map(|v| {
                let mut m = indexmap::IndexMap::new();
                for (k, val) in v {
                    m.insert(k, val);
                }
                m
            }),
        ),
        top_level_ir(),
    )
        .prop_map(|(meta, fields)| ConfigDoc { meta, fields })
}

// ===========================================================================
// Round-trip property
// ===========================================================================

proptest! {
    /// Every ConfigDoc must survive Lua → JSON → Lua without
    /// meaningful structural change (up to normalization of empty metadata).
    #[test]
    fn roundtrip_lua_json_lua(mut doc in config_doc()) {
        normalize(&mut doc);

        let json1 = serde_json::to_string_pretty(&doc)
            .expect("serialize to JSON");

        let lua = json_to_lua(&json1)
            .expect("JSON -> Lua");

        let json2 = lua_to_json(&lua)
            .expect("Lua -> JSON");

        let mut doc2: ConfigDoc = serde_json::from_str(&json2)
            .expect("deserialize round-tripped JSON");

        normalize(&mut doc2);

        prop_assert_eq!(doc, doc2);
    }
}
