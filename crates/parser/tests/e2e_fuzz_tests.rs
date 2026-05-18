use indexmap::IndexMap;
use parser::models::*;
use parser::{generator, visitor};
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

// ── Baseline Fixtures ──────────────────────────────────────────────────

const FIXTURE_DEEP_NESTED: &str = r#"
config = {
    --//Top level description
    title = "main_config",
    --//Enable feature
    --!MAP = true
    feature_enabled = true,
    --//Nested shop config
    shop = {
        --//Position
        --!MAP = true
        pos = vector3(10.0, 95.0, 20.0),
        --//Price range
        --!RANGE = { min = 0, max = 100 }
        price = 50,
        --//Location type
        --!ENUM = { "urban", "rural", "suburban" }
        location = "urban",
        --//Items for sale
        --[[TABLE = {
            layout = "items",
            schema = {
                { name = "id", type = "string", is_key = true, label = "ID" },
                { name = "name", type = "string", label = "Name" },
                { name = "cost", type = "number", label = "Cost" }
            }
        }]]
        items = {
            item_001 = { id = "item_001", name = "Sword", cost = 100 },
            item_002 = { id = "item_002", name = "Shield", cost = 50 },
        },
        --//Nested category
        category = {
            --//Category name
            name = "weapons",
            --//Sub-category
            sub = {
                label = "melee",
            },
        },
    },
    --//Discord webhook
    --[[CFX_FUNCTION = { resource_name = "webhook", function_name = "send" }]]
    webhook = function(msg)
        exports.webhook:send(msg)
    end,
    --//Event expression
    call_events = trigger_event(),
    --//Numeric list
    counts = { 1, 2, 3 },
    --//Nil
    unused = nil,
}
"#;

const FIXTURE_EMPTY_TABLES: &str = r#"
config = {
    --//Empty table
    empty_tbl = {},
    --//Empty dynamic table with schema
    --[[TABLE = {
        layout = "items",
        schema = {}
    }]]
    empty_items = {},
    --//Single value
    single = "only_one",
    --//Vector edge cases
    origin = vector3(0.0, 0.0, 0.0),
    negative = vector2(-1.0, -2.5),
}
"#;

const FIXTURE_METADATA_HEAVY: &str = r#"
config = {
    --//Field with all annotation types
    --!ENUM = { "x", "y", "z" }
    --!RANGE = { min = 1, max = 10 }
    --!MAP = true
    complex = "x",
    --//Deeply nested metadata
    deep = {
        --//Level 1
        lvl2 = {
            --//Level 2
            lvl3 = {
                --//Level 3 - no annotations
                leaf = 42,
            },
        },
    },
}
"#;

const FIXTURE_EXPRESSIONS_AND_FUNCTIONS: &str = r#"
config = {
    --//Binary expression
    calculated = 10 + 20 * 3,
    --//Unary expression
    negated = -5,
    --//Parentheses
    grouped = (1 + 2) * 3,
    --//Anonymous function
    handler = function(a, b)
        return a + b
    end,
    --//String with special chars
    special = "hello \"world\"\nnext line",
}
"#;

// ── Helpers ────────────────────────────────────────────────────────────

fn parse_lua(source: &str) -> ConfigIR {
    visitor::build_ir(source).expect("parse should succeed on valid fixture")
}

fn round_trip_ir(ir: &ConfigIR) -> (String, ConfigIR) {
    let lua = generator::generate_lua(ir).expect("generation should succeed");
    let reparsed = visitor::build_ir(&lua).expect("re-parse should succeed");
    (lua, reparsed)
}

/// Deep structural compare ignoring metadata (which may shift during generation)
fn assert_structural_eq(original: &ConfigIR, reparsed: &ConfigIR, path: &str) {
    let orig_keys: Vec<&String> = original.keys().collect();
    let reparsed_keys: Vec<&String> = reparsed.keys().collect();
    assert_eq!(
        orig_keys, reparsed_keys,
        "key order mismatch at {path}: {:?} vs {:?}",
        orig_keys, reparsed_keys
    );

    for key in original.keys() {
        let sub_path = format!("{path}.{key}");
        let orig = original.get(key).unwrap();
        let repar = reparsed.get(key).expect("reparsed should have same key");
        assert_node_eq(orig, repar, &sub_path);
    }
}

fn assert_node_eq(orig: &ConfigNode, repar: &ConfigNode, path: &str) {
    match (orig, repar) {
        (ConfigNode::String(a), ConfigNode::String(b)) => {
            assert_eq!(a.value, b.value, "String value mismatch at {path}")
        }
        (ConfigNode::Number(a), ConfigNode::Number(b)) => {
            assert!(
                (a.value - b.value).abs() < f64::EPSILON || (a.value.is_nan() && b.value.is_nan()),
                "Number value mismatch at {path}: {} vs {}",
                a.value,
                b.value
            )
        }
        (ConfigNode::Boolean(a), ConfigNode::Boolean(b)) => {
            assert_eq!(a.value, b.value, "Boolean mismatch at {path}")
        }
        (ConfigNode::Enum(a), ConfigNode::Enum(b)) => {
            assert_eq!(a.value, b.value, "Enum value mismatch at {path}")
        }
        (ConfigNode::Vector2(a), ConfigNode::Vector2(b)) => {
            assert!(
                (a.x - b.x).abs() < f64::EPSILON,
                "Vector2.x mismatch at {path}"
            );
            assert!(
                (a.y - b.y).abs() < f64::EPSILON,
                "Vector2.y mismatch at {path}"
            );
        }
        (ConfigNode::Vector3(a), ConfigNode::Vector3(b)) => {
            assert!(
                (a.x - b.x).abs() < f64::EPSILON,
                "Vector3.x mismatch at {path}"
            );
            assert!(
                (a.y - b.y).abs() < f64::EPSILON,
                "Vector3.y mismatch at {path}"
            );
            assert!(
                (a.z - b.z).abs() < f64::EPSILON,
                "Vector3.z mismatch at {path}"
            );
        }
        (ConfigNode::Table(a), ConfigNode::Table(b)) => {
            assert_eq!(
                a.fields.len(),
                b.fields.len(),
                "Table field count mismatch at {path}"
            );
            assert_structural_eq(&a.fields, &b.fields, path);
        }
        (ConfigNode::DynamicTable(a), ConfigNode::DynamicTable(b)) => {
            assert_eq!(
                a.rows.len(),
                b.rows.len(),
                "DynamicTable row count mismatch at {path}"
            );
            for (i, (ar, br)) in a.rows.iter().zip(b.rows.iter()).enumerate() {
                let row_path = format!("{path}[{i}]");
                let ak: Vec<&String> = ar.keys().collect();
                let bk: Vec<&String> = br.keys().collect();
                assert_eq!(ak, bk, "DynamicTable row key order mismatch at {row_path}");
                for k in ar.keys() {
                    let av = ar.get(k).unwrap();
                    let bv = br.get(k).unwrap();
                    // Compare numbers by f64 value to handle 0.0 vs 0 in JSON
                    match (av, bv) {
                        (serde_json::Value::Number(na), serde_json::Value::Number(nb)) => {
                            assert!(
                                na.as_f64() == nb.as_f64(),
                                "DynamicTable cell mismatch at {row_path}.{k}: {av} vs {bv}"
                            );
                        }
                        _ => {
                            assert_eq!(av, bv, "DynamicTable cell mismatch at {row_path}.{k}");
                        }
                    }
                }
            }
        }
        (ConfigNode::Function(a), ConfigNode::Function(b)) => {
            // Normalize whitespace: StyLua may reformat single-line function
            // bodies into multi-line, so we collapse all whitespace runs
            let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
            assert_eq!(
                norm(&a.value),
                norm(&b.value),
                "Function mismatch at {path}"
            )
        }
        (ConfigNode::Expression(a), ConfigNode::Expression(b)) => {
            let norm = |s: &str| s.split_whitespace().collect::<Vec<_>>().join(" ");
            assert_eq!(
                norm(&a.value),
                norm(&b.value),
                "Expression mismatch at {path}"
            )
        }
        (ConfigNode::Nil(_), ConfigNode::Nil(_)) => {}
        (ConfigNode::Array(a), ConfigNode::Array(b)) => {
            assert_eq!(
                a.items.len(),
                b.items.len(),
                "Array length mismatch at {path}"
            );
            for (i, (ai, bi)) in a.items.iter().zip(b.items.iter()).enumerate() {
                assert_node_eq(ai, bi, &format!("{path}[{i}]"));
            }
        }
        _ => panic!(
            "Type mismatch at {path}: {:?} vs {:?}",
            kind(orig),
            kind(repar)
        ),
    }
}

fn kind(node: &ConfigNode) -> &'static str {
    match node {
        ConfigNode::String(_) => "string",
        ConfigNode::Number(_) => "number",
        ConfigNode::Boolean(_) => "boolean",
        ConfigNode::Enum(_) => "enum",
        ConfigNode::Vector2(_) => "vector2",
        ConfigNode::Vector3(_) => "vector3",
        ConfigNode::Table(_) => "table",
        ConfigNode::DynamicTable(_) => "dynamic_table",
        ConfigNode::Function(_) => "function",
        ConfigNode::Expression(_) => "expression",
        ConfigNode::Nil(_) => "nil",
        ConfigNode::Array(_) => "array",
    }
}

// ── Node kind tag for type-aware metadata fuzzing ─────────────────────

#[derive(Clone, Copy)]
enum NodeKind {
    String,
    Number,
    Boolean,
    Enum,
    Vector2,
    Vector3,
    Table,
    DynamicTable,
    Function,
    Expression,
    Nil,
    Array,
}

// ── Fuzzer / Mutation Harness ──────────────────────────────────────────

fn run_fuzz_cycle(initial_lua: &str, seed: u64) {
    let ir = parse_lua(initial_lua);
    let mutated = fuzz_ir(ir, seed);
    let lua = generator::generate_lua(&mutated).unwrap_or_else(|e| {
        // Print the partially generated Lua for debugging
        panic!("seed {seed} generation failed: {e}");
    });
    let reparsed = visitor::build_ir(&lua).expect("re-parse should succeed");
    assert_structural_eq(&mutated, &reparsed, "root");
}

fn fuzz_ir(ir: ConfigIR, seed: u64) -> ConfigIR {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut out = IndexMap::new();
    for (key, node) in ir {
        let fuzzed = fuzz_node(node, &mut rng);
        out.insert(key, fuzzed);
    }
    out
}

fn fuzz_node(node: ConfigNode, rng: &mut StdRng) -> ConfigNode {
    match node {
        ConfigNode::String(v) => ConfigNode::String(StringValue {
            value: fuzz_string(&v.value, rng),
            metadata: fuzz_meta_for(v.metadata, rng, NodeKind::String),
        }),
        ConfigNode::Number(v) => ConfigNode::Number(NumberValue {
            value: fuzz_number(v.value, rng),
            metadata: fuzz_meta_for(v.metadata, rng, NodeKind::Number),
        }),
        ConfigNode::Boolean(v) => ConfigNode::Boolean(BooleanValue {
            value: rng.gen_bool(0.5) ^ v.value,
            metadata: fuzz_meta_for(v.metadata, rng, NodeKind::Boolean),
        }),
        ConfigNode::Enum(v) => ConfigNode::Enum(EnumValue {
            value: fuzz_enum(&v.value, &v.metadata, rng),
            metadata: fuzz_meta_for(v.metadata, rng, NodeKind::Enum),
        }),
        ConfigNode::Vector2(v) => ConfigNode::Vector2(Vector2Value {
            x: fuzz_coord(v.x, rng),
            y: fuzz_coord(v.y, rng),
            metadata: fuzz_meta_for(v.metadata, rng, NodeKind::Vector2),
        }),
        ConfigNode::Vector3(v) => ConfigNode::Vector3(Vector3Value {
            x: fuzz_coord(v.x, rng),
            y: fuzz_coord(v.y, rng),
            z: fuzz_coord(v.z, rng),
            metadata: fuzz_meta_for(v.metadata, rng, NodeKind::Vector3),
        }),
        ConfigNode::Table(v) => ConfigNode::Table(TableValue {
            fields: fuzz_table_fields(v.fields, rng),
            metadata: fuzz_meta_for(v.metadata, rng, NodeKind::Table),
        }),
        ConfigNode::DynamicTable(v) => ConfigNode::DynamicTable(DynamicTableValue {
            rows: fuzz_rows(v.rows, rng),
            metadata: fuzz_meta_for(v.metadata, rng, NodeKind::DynamicTable),
        }),
        ConfigNode::Function(v) => ConfigNode::Function(FunctionValue {
            value: fuzz_function_body(&v.value, rng),
            metadata: fuzz_meta_for(v.metadata, rng, NodeKind::Function),
        }),
        ConfigNode::Expression(v) => ConfigNode::Expression(ExpressionValue {
            value: fuzz_expression(&v.value, rng),
            metadata: fuzz_meta_for(v.metadata, rng, NodeKind::Expression),
        }),
        ConfigNode::Nil(v) => ConfigNode::Nil(NilValue {
            metadata: fuzz_meta_for(v.metadata, rng, NodeKind::Nil),
        }),
        ConfigNode::Array(v) => ConfigNode::Array(ArrayValue {
            items: fuzz_array_items(v.items, rng),
            metadata: fuzz_meta_for(v.metadata, rng, NodeKind::Array),
        }),
    }
}

fn fuzz_string(orig: &str, rng: &mut StdRng) -> String {
    match rng.gen_range(0..5) {
        0 => orig.to_string(),
        1 => String::new(),
        2 => "a".repeat(rng.gen_range(1..50)),
        3 => format!("{}__{}", orig, rng.r#gen::<u64>()),
        _ => {
            let special = ["\\\"", "\n", "\t", "\r", "\\", "`", "@", "#"];
            let pick = special[rng.gen_range(0..special.len())];
            format!("{orig}{pick}")
        }
    }
}

fn fuzz_number(orig: f64, rng: &mut StdRng) -> f64 {
    match rng.gen_range(0..6) {
        0 => orig,
        1 => -orig,
        2 => rng.gen_range(-1e6..1e6),
        3 => 0.0,
        4 => orig * rng.gen_range(0.5..2.0),
        _ => (rng.r#gen::<i64>() % 1000) as f64,
    }
}

fn fuzz_coord(orig: f64, rng: &mut StdRng) -> f64 {
    match rng.gen_range(0..5) {
        0 => orig,
        1 => -orig,
        2 => rng.gen_range(-1e4..1e4),
        3 => 0.0,
        _ => orig * rng.gen_range(0.1..10.0),
    }
}

fn fuzz_enum(orig: &str, meta: &Option<Box<FieldMetadata>>, rng: &mut StdRng) -> String {
    if let Some(meta) = meta {
        if let Some(options) = &meta.enum_options {
            if !options.is_empty() && rng.gen_bool(0.5) {
                let idx = rng.gen_range(0..options.len());
                return options[idx].clone();
            }
        }
    }
    fuzz_string(orig, rng)
}

fn fuzz_function_body(orig: &str, rng: &mut StdRng) -> String {
    match rng.gen_range(0..3) {
        0 => orig.to_string(),
        1 => format!("function() return {} end", rng.r#gen::<u64>()),
        _ => "function() end".to_string(),
    }
}

fn fuzz_expression(orig: &str, rng: &mut StdRng) -> String {
    // Only produce values that stay as Expression after round-trip.
    // Bare numbers become Number, bare strings become String, so avoid those.
    match rng.gen_range(0..4) {
        0 => orig.to_string(),
        1 => format!("{} + {}", rng.r#gen::<i32>(), rng.r#gen::<i32>()),
        2 => format!("({} + {})", rng.r#gen::<i32>(), rng.r#gen::<i32>()),
        _ => format!("call_{}()", rng.gen_range(0u32..1000)),
    }
}

fn fuzz_table_fields(
    fields: IndexMap<String, ConfigNode>,
    rng: &mut StdRng,
) -> IndexMap<String, ConfigNode> {
    let mut out = IndexMap::new();
    let names = ["a", "b", "c", "x", "y", "z", "foo", "bar", "baz"];
    for (key, node) in fields {
        if rng.gen_bool(0.15) {
            continue; // remove field
        }
        let new_key = if rng.gen_bool(0.2) {
            names[rng.gen_range(0..names.len())].to_string()
        } else {
            key
        };
        if rng.gen_bool(0.1) {
            out.insert(new_key.clone(), fuzz_node(node, rng));
            out.insert(format!("{new_key}_extra"), gen_random_node(rng));
        } else {
            out.insert(new_key, fuzz_node(node, rng));
        }
    }
    if rng.gen_bool(0.1) && !out.is_empty() {
        let mut keys: Vec<String> = out.keys().cloned().collect();
        let idx = rng.gen_range(0..keys.len());
        keys.swap(0, idx);
        let mut reordered = IndexMap::new();
        for k in keys {
            if let Some(v) = out.swap_remove(&k) {
                reordered.insert(k, v);
            }
        }
        reordered
    } else {
        out
    }
}

fn fuzz_rows(
    rows: Vec<IndexMap<String, serde_json::Value>>,
    rng: &mut StdRng,
) -> Vec<IndexMap<String, serde_json::Value>> {
    if rows.is_empty() {
        return rows;
    }
    let mut out: Vec<IndexMap<String, serde_json::Value>> = rows;
    if rng.gen_bool(0.2) {
        let idx = rng.gen_range(0..out.len());
        let new_row = fuzz_single_row(&out[idx], rng);
        out.push(new_row);
    }
    if rng.gen_bool(0.2) && out.len() > 1 {
        let idx = rng.gen_range(0..out.len());
        out.remove(idx);
    }
    if rng.gen_bool(0.3) {
        out.shuffle(rng);
    }
    for row in &mut out {
        *row = fuzz_single_row(row, rng);
    }
    out
}

fn fuzz_single_row(
    row: &IndexMap<String, serde_json::Value>,
    rng: &mut StdRng,
) -> IndexMap<String, serde_json::Value> {
    let mut out = row.clone();
    if let Some(_key) = out.get("_key") {
        if rng.gen_bool(0.4) {
            let uuid = format!("fuzz_{:016x}", rng.r#gen::<u64>());
            out.insert("_key".to_string(), serde_json::Value::String(uuid));
        }
    }
    for (k, v) in out.clone().iter() {
        if k == "_key" {
            continue;
        }
        if rng.gen_bool(0.25) {
            out.insert(k.clone(), fuzz_json_value(v, rng));
        }
    }
    if rng.gen_bool(0.15) {
        let new_key = format!("extra_{}", rng.r#gen::<u32>());
        out.insert(
            new_key,
            serde_json::Value::String(format!("val_{}", rng.r#gen::<u64>())),
        );
    }
    out
}

fn fuzz_json_value(val: &serde_json::Value, rng: &mut StdRng) -> serde_json::Value {
    match val {
        serde_json::Value::String(s) => serde_json::Value::String(fuzz_string(s, rng)),
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                serde_json::json!(fuzz_number(f, rng))
            } else {
                serde_json::json!(rng.r#gen::<i64>())
            }
        }
        serde_json::Value::Bool(b) => serde_json::Value::Bool(rng.gen_bool(0.5) ^ b),
        _ => val.clone(),
    }
}

fn fuzz_array_items(items: Vec<ConfigNode>, rng: &mut StdRng) -> Vec<ConfigNode> {
    let mut out = items;
    if rng.gen_bool(0.2) {
        out.push(gen_random_node(rng));
    }
    if rng.gen_bool(0.2) && !out.is_empty() {
        let idx = rng.gen_range(0..out.len());
        out.remove(idx);
    }
    for item in &mut out {
        *item = fuzz_node(
            std::mem::replace(item, ConfigNode::Nil(NilValue { metadata: None })),
            rng,
        );
    }
    out
}

fn gen_random_node(rng: &mut StdRng) -> ConfigNode {
    match rng.gen_range(0..8) {
        0 => ConfigNode::String(StringValue {
            value: format!("rnd_{}", rng.r#gen::<u64>()),
            metadata: None,
        }),
        1 => ConfigNode::Number(NumberValue {
            value: rng.gen_range(-1000.0..1000.0),
            metadata: None,
        }),
        2 => ConfigNode::Boolean(BooleanValue {
            value: rng.gen_bool(0.5),
            metadata: None,
        }),
        3 => ConfigNode::Nil(NilValue { metadata: None }),
        4 => ConfigNode::Expression(ExpressionValue {
            value: format!("{} + {}", rng.r#gen::<i32>(), rng.r#gen::<i32>()),
            metadata: None,
        }),
        5 => ConfigNode::Vector2(Vector2Value {
            x: rng.gen_range(-10.0..10.0),
            y: rng.gen_range(-10.0..10.0),
            metadata: None,
        }),
        6 => ConfigNode::Vector3(Vector3Value {
            x: rng.gen_range(-10.0..10.0),
            y: rng.gen_range(-10.0..10.0),
            z: rng.gen_range(-10.0..10.0),
            metadata: None,
        }),
        _ => ConfigNode::String(StringValue {
            value: String::new(),
            metadata: None,
        }),
    }
}

// ── Metadata fuzzing ───────────────────────────────────────────────────

fn fuzz_meta(meta: Option<Box<FieldMetadata>>, rng: &mut StdRng) -> Option<Box<FieldMetadata>> {
    let mut m = match meta {
        Some(m) => *m,
        None => {
            if rng.gen_bool(0.3) {
                return None;
            }
            FieldMetadata::default()
        }
    };

    if rng.gen_bool(0.3) {
        m.description = Some(fuzz_string(m.description.as_deref().unwrap_or("desc"), rng));
    }
    if rng.gen_bool(0.3) {
        m.range = Some(fuzz_range(m.range, rng));
    }
    if rng.gen_bool(0.3) {
        m.enum_options = Some(fuzz_enum_options(m.enum_options, rng));
    }
    if rng.gen_bool(0.2) {
        m.map = Some(rng.gen_bool(0.5));
    }
    if rng.gen_bool(0.2) {
        m.nil_marker = Some(fuzz_string(m.nil_marker.as_deref().unwrap_or("nil"), rng));
    }
    if rng.gen_bool(0.2) {
        m.function_info = Some(fuzz_function_info(m.function_info, rng));
    }
    if rng.gen_bool(0.3) {
        m.table_schema = Some(fuzz_table_schema(m.table_schema, rng));
    }

    let has_any = m.description.is_some()
        || m.range.is_some()
        || m.enum_options.is_some()
        || m.map.is_some()
        || m.nil_marker.is_some()
        || m.function_info.is_some()
        || m.table_schema.is_some();

    if has_any { Some(Box::new(m)) } else { None }
}

/// Type-aware metadata fuzzer: strips metadata that would change the node's
/// type when the generated Lua is re-parsed.
fn fuzz_meta_for(
    meta: Option<Box<FieldMetadata>>,
    rng: &mut StdRng,
    kind: NodeKind,
) -> Option<Box<FieldMetadata>> {
    let mut result = fuzz_meta(meta, rng);

    // DynamicTable MUST always have table_schema with layout="items",
    // even if fuzz_meta returned None — otherwise empty {} re-parses as Table.
    if matches!(kind, NodeKind::DynamicTable) && result.is_none() {
        result = Some(Box::new(FieldMetadata {
            table_schema: Some(TableSchema {
                layout: "items".into(),
                schema: Vec::new(),
            }),
            ..Default::default()
        }));
    }

    // Enum MUST always have enum_options
    if matches!(kind, NodeKind::Enum) && result.is_none() {
        result = Some(Box::new(FieldMetadata {
            enum_options: Some(vec!["a".into(), "b".into(), "c".into()]),
            ..Default::default()
        }));
    }

    if let Some(ref mut m) = result {
        match kind {
            // enum_options on a String makes the parser return Enum
            NodeKind::String => {
                m.enum_options = None;
            }
            // Enum MUST have enum_options to remain an Enum
            NodeKind::Enum => {
                if m.enum_options.is_none()
                    || m.enum_options
                        .as_ref()
                        .map(|o| o.is_empty())
                        .unwrap_or(true)
                {
                    m.enum_options = Some(vec!["a".into(), "b".into(), "c".into()]);
                }
            }
            // DynamicTable must always have table_schema with layout = "items"
            NodeKind::DynamicTable => match m.table_schema {
                Some(ref mut ts) => {
                    ts.layout = "items".into();
                }
                None => {
                    m.table_schema = Some(TableSchema {
                        layout: "items".into(),
                        schema: Vec::new(),
                    });
                }
            },
            // Table must NOT have layout = "items" (would become DynamicTable)
            NodeKind::Table => {
                if let Some(ref mut ts) = m.table_schema {
                    if ts.layout == "items" {
                        ts.layout = "static".into();
                    }
                }
            }
            // Array must not have layout = "items" (would become DynamicTable)
            NodeKind::Array => {
                if let Some(ref ts) = m.table_schema {
                    if ts.layout == "items" {
                        m.table_schema = None;
                    }
                }
            }
            // Other node types: metadata doesn't affect type parsing
            _ => {}
        }
    }
    result
}

fn fuzz_range(range: Option<RangeValue>, rng: &mut StdRng) -> RangeValue {
    let mut r = range.unwrap_or(RangeValue {
        min: 0.0,
        max: 100.0,
    });
    match rng.gen_range(0..4) {
        0 => r.min = fuzz_number(r.min, rng),
        1 => r.max = fuzz_number(r.max, rng),
        2 => std::mem::swap(&mut r.min, &mut r.max),
        _ => {}
    }
    r
}

fn fuzz_enum_options(options: Option<Vec<String>>, rng: &mut StdRng) -> Vec<String> {
    let mut opts = options.unwrap_or_else(|| vec!["a".into(), "b".into(), "c".into()]);
    if rng.gen_bool(0.3) && !opts.is_empty() {
        let idx = rng.gen_range(0..opts.len());
        opts[idx] = fuzz_string(&opts[idx], rng);
    }
    if rng.gen_bool(0.2) {
        opts.push(fuzz_string("opt", rng));
    }
    if rng.gen_bool(0.2) && opts.len() > 1 {
        opts.remove(rng.gen_range(0..opts.len()));
    }
    if rng.gen_bool(0.3) {
        opts.shuffle(rng);
    }
    opts
}

fn fuzz_function_info(info: Option<FunctionInfo>, rng: &mut StdRng) -> FunctionInfo {
    let mut i = info.unwrap_or(FunctionInfo {
        resource_name: "res".into(),
        function_name: "func".into(),
    });
    if rng.gen_bool(0.5) {
        i.resource_name = fuzz_string(&i.resource_name, rng);
    }
    if rng.gen_bool(0.5) {
        i.function_name = fuzz_string(&i.function_name, rng);
    }
    i
}

fn fuzz_table_schema(schema: Option<TableSchema>, rng: &mut StdRng) -> TableSchema {
    let mut s = schema.unwrap_or(TableSchema {
        layout: "static".into(),
        schema: Vec::new(),
    });
    if rng.gen_bool(0.4) {
        s.layout = match rng.gen_range(0..3) {
            0 => "static".into(),
            1 => "items".into(),
            _ => fuzz_string(&s.layout, rng),
        };
    }
    if rng.gen_bool(0.3) {
        let col = ColumnSchema {
            name: fuzz_string("col", rng),
            column_type: ["string", "number", "boolean"][rng.gen_range(0..3)].into(),
            is_key: Some(rng.gen_bool(0.3)),
            label: Some(fuzz_string("label", rng)),
            description: Some(fuzz_string("desc", rng)),
        };
        s.schema.push(col);
    }
    if rng.gen_bool(0.3) && !s.schema.is_empty() {
        s.schema.remove(rng.gen_range(0..s.schema.len()));
    }
    for col in &mut s.schema {
        if rng.gen_bool(0.3) {
            col.name = fuzz_string(&col.name, rng);
        }
        if rng.gen_bool(0.3) {
            col.column_type = ["string", "number", "boolean"][rng.gen_range(0..3)].into();
        }
        if rng.gen_bool(0.2) {
            col.is_key = Some(rng.gen_bool(0.5));
        }
    }
    if rng.gen_bool(0.3) {
        s.schema.shuffle(rng);
    }
    s
}

// ── Tests ──────────────────────────────────────────────────────────────

#[test]
fn baseline_round_trip_deep_nested() {
    let ir = parse_lua(FIXTURE_DEEP_NESTED);
    let (lua, reparsed) = round_trip_ir(&ir);
    assert_structural_eq(&ir, &reparsed, "root");
    // validate Lua is syntactically valid
    full_moon::parse(&lua).expect("generated Lua must be valid");
}

#[test]
fn baseline_round_trip_empty_tables() {
    let ir = parse_lua(FIXTURE_EMPTY_TABLES);
    let (_lua, reparsed) = round_trip_ir(&ir);
    assert_structural_eq(&ir, &reparsed, "root");
}

#[test]
fn baseline_round_trip_metadata_heavy() {
    let ir = parse_lua(FIXTURE_METADATA_HEAVY);
    let (_lua, reparsed) = round_trip_ir(&ir);
    assert_structural_eq(&ir, &reparsed, "root");
}

#[test]
fn baseline_round_trip_expressions_and_functions() {
    let ir = parse_lua(FIXTURE_EXPRESSIONS_AND_FUNCTIONS);
    let (_lua, reparsed) = round_trip_ir(&ir);
    assert_structural_eq(&ir, &reparsed, "root");
}

#[test]
fn fuzz_deep_nested_50_cycles() {
    for seed in 0..50 {
        run_fuzz_cycle(FIXTURE_DEEP_NESTED, seed);
    }
}

#[test]
fn fuzz_empty_tables_50_cycles() {
    for seed in 0..50 {
        run_fuzz_cycle(FIXTURE_EMPTY_TABLES, seed);
    }
}

#[test]
fn fuzz_metadata_heavy_50_cycles() {
    for seed in 0..50 {
        run_fuzz_cycle(FIXTURE_METADATA_HEAVY, seed);
    }
}

#[test]
fn fuzz_expressions_and_functions_50_cycles() {
    for seed in 0..50 {
        run_fuzz_cycle(FIXTURE_EXPRESSIONS_AND_FUNCTIONS, seed);
    }
}

#[test]
fn fuzz_all_fixtures_200_iterations() {
    let fixtures = [
        FIXTURE_DEEP_NESTED,
        FIXTURE_EMPTY_TABLES,
        FIXTURE_METADATA_HEAVY,
        FIXTURE_EXPRESSIONS_AND_FUNCTIONS,
    ];
    for batch in 0..5 {
        for (fi, &fixture) in fixtures.iter().enumerate() {
            let seed = (batch * fixtures.len() + fi) as u64;
            run_fuzz_cycle(fixture, seed);
        }
    }
}

// ── Property-based assertions ──────────────────────────────────────────

#[test]
fn generated_lua_is_always_valid_syntax() {
    let fixtures = [
        FIXTURE_DEEP_NESTED,
        FIXTURE_EMPTY_TABLES,
        FIXTURE_METADATA_HEAVY,
        FIXTURE_EXPRESSIONS_AND_FUNCTIONS,
    ];
    for &src in &fixtures {
        let ir = parse_lua(src);
        let lua = generator::generate_lua(&ir).expect("generation must not fail");
        match full_moon::parse(&lua) {
            Ok(_) => {}
            Err(errors) => {
                panic!(
                    "generated Lua has syntax errors:\n--- lua ---\n{lua}\n--- errors ---\n{}",
                    errors
                        .iter()
                        .map(|e| e.error_message().to_string())
                        .collect::<Vec<_>>()
                        .join("; ")
                )
            }
        }
    }
}

#[test]
fn key_order_preserved_through_round_trip() {
    let ir = parse_lua(FIXTURE_DEEP_NESTED);
    let (_lua, reparsed) = round_trip_ir(&ir);
    let orig_keys: Vec<&String> = ir.keys().collect();
    let reparsed_keys: Vec<&String> = reparsed.keys().collect();
    assert_eq!(
        orig_keys, reparsed_keys,
        "top-level key order must be preserved"
    );
}

#[test]
fn dynamic_table_rows_key_order_preserved() {
    let ir = parse_lua(FIXTURE_DEEP_NESTED);
    let shop = match ir.get("shop").unwrap() {
        ConfigNode::Table(t) => &t.fields,
        _ => panic!("expected shop table"),
    };
    let items = match shop.get("items").unwrap() {
        ConfigNode::DynamicTable(dt) => dt,
        _ => panic!("expected dynamic table"),
    };
    let (_lua, reparsed) = round_trip_ir(&ir);
    let reparsed_shop = match reparsed.get("shop").unwrap() {
        ConfigNode::Table(t) => &t.fields,
        _ => panic!("expected shop table in reparsed"),
    };
    let reparsed_items = match reparsed_shop.get("items").unwrap() {
        ConfigNode::DynamicTable(dt) => dt,
        _ => panic!("expected dynamic table in reparsed"),
    };
    for (i, (orig_row, repar_row)) in items
        .rows
        .iter()
        .zip(reparsed_items.rows.iter())
        .enumerate()
    {
        let orig_keys: Vec<&String> = orig_row.keys().collect();
        let repar_keys: Vec<&String> = repar_row.keys().collect();
        assert_eq!(
            orig_keys, repar_keys,
            "dynamic table row {i} key order mismatch"
        );
    }
}

#[test]
fn zero_panic_on_corrupted_metadata() {
    let ir = parse_lua(FIXTURE_DEEP_NESTED);
    let mut corrupted = ir.clone();
    if let Some(ConfigNode::DynamicTable(dt)) = corrupted.get_mut("shop").and_then(|n| {
        if let ConfigNode::Table(t) = n {
            t.fields.get_mut("items")
        } else {
            None
        }
    }) {
        dt.metadata = Some(Box::new(FieldMetadata {
            description: Some("\n\n\n".into()),
            range: Some(RangeValue {
                min: f64::NAN,
                max: f64::NAN,
            }),
            enum_options: Some(Vec::new()),
            map: None,
            nil_marker: Some("\0null\0".into()),
            function_info: Some(FunctionInfo {
                resource_name: "\n".into(),
                function_name: "\t".into(),
            }),
            table_schema: Some(TableSchema {
                layout: "\n\n".into(),
                schema: vec![ColumnSchema {
                    name: "\0".into(),
                    column_type: "".into(),
                    is_key: None,
                    label: Some("\r".into()),
                    description: Some("\0".into()),
                }],
            }),
        }));
    }
    // must not panic, can return Err but not panic
    let _ = generator::generate_lua(&corrupted);
}

#[test]
fn zero_panic_on_deeply_nested_corruption() {
    let ir = parse_lua(FIXTURE_DEEP_NESTED);
    // Replace every node with random corruption recursively
    fn corrupt_all(node: &mut ConfigNode, depth: usize) {
        if depth > 10 {
            return;
        }
        match node {
            ConfigNode::Table(t) => {
                for (_, v) in t.fields.iter_mut() {
                    corrupt_all(v, depth + 1);
                }
                if let Some(meta) = &mut t.metadata {
                    meta.description = Some("\n\n\n\n\n".into());
                }
            }
            ConfigNode::DynamicTable(dt) => {
                for row in &mut dt.rows {
                    for (_, v) in row.iter_mut() {
                        if let serde_json::Value::String(s) = v {
                            *s = "\0\n\t\r".into();
                        }
                    }
                }
            }
            ConfigNode::Function(f) => {
                f.value = "function()\n\n\n\nend".into();
            }
            ConfigNode::Expression(e) => {
                e.value = "(((((".into();
            }
            _ => {}
        }
    }
    let mut ir = ir;
    for (_, node) in ir.iter_mut() {
        corrupt_all(node, 0);
    }
    let _ = generator::generate_lua(&ir);
}

#[test]
fn empty_ir_round_trip() {
    let ir = ConfigIR::new();
    let lua = generator::generate_lua(&ir).expect("empty IR should generate");
    let reparsed = visitor::build_ir(&lua).expect("empty generated Lua should re-parse");
    assert!(reparsed.is_empty(), "re-parsed empty IR should be empty");
}

#[test]
fn single_key_round_trip() {
    let mut ir = ConfigIR::new();
    ir.insert(
        "key".into(),
        ConfigNode::String(StringValue {
            value: "value".into(),
            metadata: None,
        }),
    );
    let (_lua, reparsed) = round_trip_ir(&ir);
    assert_structural_eq(&ir, &reparsed, "root");
}

#[test]
fn all_node_types_round_trip() {
    let mut ir = ConfigIR::new();
    ir.insert(
        "s".into(),
        ConfigNode::String(StringValue {
            value: "hello".into(),
            metadata: None,
        }),
    );
    ir.insert(
        "n".into(),
        ConfigNode::Number(NumberValue {
            value: 42.5,
            metadata: None,
        }),
    );
    ir.insert(
        "b".into(),
        ConfigNode::Boolean(BooleanValue {
            value: true,
            metadata: None,
        }),
    );
    ir.insert(
        "e".into(),
        ConfigNode::Enum(EnumValue {
            value: "a".into(),
            metadata: Some(Box::new(FieldMetadata {
                enum_options: Some(vec!["a".into(), "b".into()]),
                ..Default::default()
            })),
        }),
    );
    ir.insert(
        "v2".into(),
        ConfigNode::Vector2(Vector2Value {
            x: 1.0,
            y: 2.0,
            metadata: None,
        }),
    );
    ir.insert(
        "v3".into(),
        ConfigNode::Vector3(Vector3Value {
            x: 1.0,
            y: 2.0,
            z: 3.0,
            metadata: None,
        }),
    );
    ir.insert(
        "t".into(),
        ConfigNode::Table(TableValue {
            fields: {
                let mut f = IndexMap::new();
                f.insert(
                    "inner".into(),
                    ConfigNode::Number(NumberValue {
                        value: 99.0,
                        metadata: None,
                    }),
                );
                f
            },
            metadata: None,
        }),
    );
    ir.insert(
        "dt".into(),
        ConfigNode::DynamicTable(DynamicTableValue {
            rows: vec![{
                let mut r = IndexMap::new();
                r.insert("_key".into(), serde_json::Value::String("id_1".into()));
                r.insert(
                    "val".into(),
                    serde_json::Value::Number(serde_json::Number::from(10)),
                );
                r
            }],
            metadata: Some(Box::new(FieldMetadata {
                table_schema: Some(TableSchema {
                    layout: "items".into(),
                    schema: vec![ColumnSchema {
                        name: "val".into(),
                        column_type: "number".into(),
                        is_key: None,
                        label: None,
                        description: None,
                    }],
                }),
                ..Default::default()
            })),
        }),
    );
    ir.insert(
        "fn".into(),
        ConfigNode::Function(FunctionValue {
            value: "function() end".into(),
            metadata: None,
        }),
    );
    ir.insert(
        "expr".into(),
        ConfigNode::Expression(ExpressionValue {
            value: "1 + 2".into(),
            metadata: None,
        }),
    );
    ir.insert("nil".into(), ConfigNode::Nil(NilValue { metadata: None }));
    ir.insert(
        "arr".into(),
        ConfigNode::Array(ArrayValue {
            items: vec![
                ConfigNode::Number(NumberValue {
                    value: 1.0,
                    metadata: None,
                }),
                ConfigNode::Number(NumberValue {
                    value: 2.0,
                    metadata: None,
                }),
            ],
            metadata: None,
        }),
    );

    let (_lua, reparsed) = round_trip_ir(&ir);
    assert_structural_eq(&ir, &reparsed, "root");
}

// ── Determinism Test ──────────────────────────────────────────────────

#[test]
fn deterministic_output_same_seed_same_result() {
    const SEED: u64 = 42;
    let ir = parse_lua(FIXTURE_DEEP_NESTED);
    let mutated_a = fuzz_ir(ir.clone(), SEED);
    let mutated_b = fuzz_ir(ir, SEED);
    let (lua_a, _) = round_trip_ir(&mutated_a);
    let (lua_b, _) = round_trip_ir(&mutated_b);
    assert_eq!(lua_a, lua_b, "same seed must produce identical output");
}
