use indexmap::IndexMap;
use parser::models::*;
use parser::visitor;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::{Rng, SeedableRng};

const FIXTURE: &str = r#"
config = {
    --//Top level description
    title = "main_config",
    --//Enable feature
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
    match rng.gen_range(0..4) {
        0 => orig.to_string(),
        1 => format!("{}", rng.r#gen::<i32>()),
        2 => format!("\"{}\"", fuzz_string("val", rng)),
        _ => format!("{} + {}", rng.r#gen::<i32>(), rng.r#gen::<i32>()),
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
            continue;
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
    let mut out = rows;
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
            value: format!("{}", rng.r#gen::<i32>()),
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
fn fuzz_node(node: ConfigNode, rng: &mut StdRng) -> ConfigNode {
    match node {
        ConfigNode::String(v) => ConfigNode::String(StringValue {
            value: fuzz_string(&v.value, rng),
            metadata: fuzz_meta(v.metadata, rng),
        }),
        ConfigNode::Number(v) => ConfigNode::Number(NumberValue {
            value: fuzz_number(v.value, rng),
            metadata: fuzz_meta(v.metadata, rng),
        }),
        ConfigNode::Boolean(v) => ConfigNode::Boolean(BooleanValue {
            value: rng.gen_bool(0.5) ^ v.value,
            metadata: fuzz_meta(v.metadata, rng),
        }),
        ConfigNode::Enum(v) => ConfigNode::Enum(EnumValue {
            value: fuzz_enum(&v.value, &v.metadata, rng),
            metadata: fuzz_meta(v.metadata, rng),
        }),
        ConfigNode::Vector2(v) => ConfigNode::Vector2(Vector2Value {
            x: fuzz_coord(v.x, rng),
            y: fuzz_coord(v.y, rng),
            metadata: fuzz_meta(v.metadata, rng),
        }),
        ConfigNode::Vector3(v) => ConfigNode::Vector3(Vector3Value {
            x: fuzz_coord(v.x, rng),
            y: fuzz_coord(v.y, rng),
            z: fuzz_coord(v.z, rng),
            metadata: fuzz_meta(v.metadata, rng),
        }),
        ConfigNode::Table(v) => ConfigNode::Table(TableValue {
            fields: fuzz_table_fields(v.fields, rng),
            metadata: fuzz_meta(v.metadata, rng),
        }),
        ConfigNode::DynamicTable(v) => ConfigNode::DynamicTable(DynamicTableValue {
            rows: fuzz_rows(v.rows, rng),
            metadata: fuzz_meta(v.metadata, rng),
        }),
        ConfigNode::Function(v) => ConfigNode::Function(FunctionValue {
            value: fuzz_function_body(&v.value, rng),
            metadata: fuzz_meta(v.metadata, rng),
        }),
        ConfigNode::Expression(v) => ConfigNode::Expression(ExpressionValue {
            value: fuzz_expression(&v.value, rng),
            metadata: fuzz_meta(v.metadata, rng),
        }),
        ConfigNode::Nil(v) => ConfigNode::Nil(NilValue {
            metadata: fuzz_meta(v.metadata, rng),
        }),
        ConfigNode::Array(v) => ConfigNode::Array(ArrayValue {
            items: fuzz_array_items(v.items, rng),
            metadata: fuzz_meta(v.metadata, rng),
        }),
    }
}
fn fuzz_ir(ir: IndexMap<String, ConfigNode>, seed: u64) -> IndexMap<String, ConfigNode> {
    let mut rng = StdRng::seed_from_u64(seed);
    let mut out = IndexMap::new();
    for (key, node) in ir {
        let fuzzed = fuzz_node(node, &mut rng);
        out.insert(key, fuzzed);
    }
    out
}

#[test]
fn debug_seed0() {
    let ir = visitor::build_ir(FIXTURE).unwrap();
    let mutated = fuzz_ir(ir, 0);

    // Manually replicate the generator but WITHOUT stylua to see the raw output
    let mut out = String::from("config = {\n");
    // Use the generator's internal functions by just calling generate_lua
    // but catch the error and print the raw output

    // Actually let's just generate manually
    fn emit_indent(out: &mut String, indent: usize) {
        for _ in 0..indent {
            out.push_str("    ");
        }
    }
    fn escape_lua_string(s: &str) -> String {
        s.replace('\\', "\\\\")
            .replace('"', "\\\"")
            .replace('\n', "\\n")
            .replace('\r', "\\r")
            .replace('\t', "\\t")
    }
    fn escape_key(key: &str) -> String {
        let lua_kw = [
            "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto",
            "if", "in", "local", "nil", "not", "or", "repeat", "return", "then", "true", "until",
            "while",
        ];
        if key.contains(|c: char| !c.is_alphanumeric() && c != '_')
            || key.is_empty()
            || lua_kw.contains(&key)
        {
            format!("[\"{}\"]", key.replace('\\', "\\\\").replace('"', "\\\""))
        } else {
            key.to_string()
        }
    }

    fn emit_meta(out: &mut String, meta: Option<&FieldMetadata>, indent: usize) {
        let meta = match meta {
            Some(m) => m,
            None => return,
        };
        if let Some(desc) = &meta.description {
            emit_indent(out, indent);
            let sanitized = desc.replace('\n', " ").replace('\r', " ");
            out.push_str(&format!("--//{sanitized}\n"));
        }
        if let Some(range) = &meta.range {
            emit_indent(out, indent);
            out.push_str(&format!(
                "--!RANGE = {{ min = {}, max = {} }}\n",
                range.min, range.max
            ));
        }
        if let Some(options) = &meta.enum_options {
            emit_indent(out, indent);
            out.push_str("--!ENUM = { ");
            for (i, opt) in options.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(&format!("\"{opt}\""));
            }
            out.push_str(" }\n");
        }
        if let Some(map) = &meta.map {
            emit_indent(out, indent);
            out.push_str(&format!("--!MAP = {map}\n"));
        }
        if let Some(fi) = &meta.function_info {
            emit_indent(out, indent);
            out.push_str(&format!(
                "--[[CFX_FUNCTION = {{ resource_name = \"{}\", function_name = \"{}\" }}]]\n",
                fi.resource_name, fi.function_name
            ));
        }
        if let Some(schema) = &meta.table_schema {
            emit_indent(out, indent);
            out.push_str("--[[TABLE = {\n");
            emit_indent(out, indent + 1);
            out.push_str(&format!("layout = \"{}\",\n", schema.layout));
            emit_indent(out, indent + 1);
            out.push_str("schema = {\n");
            for col in &schema.schema {
                emit_indent(out, indent + 2);
                out.push_str(&format!(
                    "{{ name = \"{}\", type = \"{}\"",
                    col.name, col.column_type
                ));
                if let Some(true) = col.is_key {
                    out.push_str(", is_key = true");
                }
                if let Some(label) = &col.label {
                    out.push_str(&format!(", label = \"{label}\""));
                }
                if let Some(desc) = &col.description {
                    out.push_str(&format!(", description = \"{desc}\""));
                }
                out.push_str(" },\n");
            }
            emit_indent(out, indent + 1);
            out.push_str("},\n");
            emit_indent(out, indent);
            out.push_str("}]]\n");
        }
    }

    fn emit_node(out: &mut String, key: &str, node: &ConfigNode, indent: usize) {
        let meta = match node {
            ConfigNode::String(s) => &s.metadata,
            ConfigNode::Number(n) => &n.metadata,
            ConfigNode::Boolean(b) => &b.metadata,
            ConfigNode::Enum(e) => &e.metadata,
            ConfigNode::Vector2(v) => &v.metadata,
            ConfigNode::Vector3(v) => &v.metadata,
            ConfigNode::Table(t) => &t.metadata,
            ConfigNode::DynamicTable(d) => &d.metadata,
            ConfigNode::Function(f) => &f.metadata,
            ConfigNode::Expression(e) => &e.metadata,
            ConfigNode::Nil(n) => &n.metadata,
            ConfigNode::Array(a) => &a.metadata,
        };
        emit_meta(out, meta.as_deref(), indent);
        emit_indent(out, indent);
        out.push_str(&escape_key(key));
        out.push_str(" = ");
        match node {
            ConfigNode::String(s) => out.push_str(&format!("\"{}\"", escape_lua_string(&s.value))),
            ConfigNode::Number(n) => {
                if n.value.fract() == 0.0 {
                    out.push_str(&format!("{}", n.value as i64));
                } else {
                    out.push_str(&format!("{}", n.value));
                }
            }
            ConfigNode::Boolean(b) => out.push_str(if b.value { "true" } else { "false" }),
            ConfigNode::Enum(e) => out.push_str(&format!("\"{}\"", escape_lua_string(&e.value))),
            ConfigNode::Vector2(v) => out.push_str(&format!("vector2({}, {})", v.x, v.y)),
            ConfigNode::Vector3(v) => out.push_str(&format!("vector3({}, {}, {})", v.x, v.y, v.z)),
            ConfigNode::Table(t) => {
                if t.fields.is_empty() {
                    out.push_str("{}");
                } else {
                    out.push_str("{\n");
                    for (k, n) in &t.fields {
                        emit_node(out, k, n, indent + 1);
                    }
                    emit_indent(out, indent);
                    out.push('}');
                }
            }
            ConfigNode::DynamicTable(d) => {
                if d.rows.is_empty() {
                    out.push_str("{}");
                } else {
                    out.push_str("{\n");
                    for row in &d.rows {
                        let rk = row.get("_key").and_then(|v| v.as_str()).unwrap_or("");
                        emit_indent(out, indent + 1);
                        out.push_str(&format!("[\"{}\"] = {{\n", escape_lua_string(rk)));
                        for (k, v) in row {
                            if k == "_key" {
                                continue;
                            }
                            emit_indent(out, indent + 2);
                            out.push_str(&escape_key(k));
                            out.push_str(&format!(" = {},\n", v));
                        }
                        emit_indent(out, indent + 1);
                        out.push_str("},\n");
                    }
                    emit_indent(out, indent);
                    out.push('}');
                }
            }
            ConfigNode::Function(f) => out.push_str(&f.value),
            ConfigNode::Expression(e) => out.push_str(&e.value),
            ConfigNode::Nil(_) => out.push_str("nil"),
            ConfigNode::Array(a) => {
                if a.items.is_empty() {
                    out.push_str("{}");
                } else {
                    out.push_str("{\n");
                    for item in &a.items {
                        emit_indent(out, indent + 1);
                        match item {
                            ConfigNode::Number(n) => {
                                if n.value.fract() == 0.0 {
                                    out.push_str(&format!("{}", n.value as i64));
                                } else {
                                    out.push_str(&format!("{}", n.value));
                                }
                            }
                            ConfigNode::String(s) => {
                                out.push_str(&format!("\"{}\"", escape_lua_string(&s.value)))
                            }
                            ConfigNode::Boolean(b) => {
                                out.push_str(if b.value { "true" } else { "false" })
                            }
                            _ => {}
                        }
                        out.push_str(",\n");
                    }
                    emit_indent(out, indent);
                    out.push('}');
                }
            }
        }
        out.push_str(",\n");
    }

    for (i, (key, node)) in mutated.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        emit_node(&mut out, key, node, 1);
    }
    out.push_str("}\n");

    eprintln!("=== RAW LUA (line-numbered) ===");
    for (i, line) in out.lines().enumerate() {
        eprintln!("{:>4}: {}", i + 1, line);
    }
}
