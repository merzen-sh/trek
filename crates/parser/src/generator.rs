use crate::models::*;
use std::fmt::Write;

pub fn generate_lua(ir: &ConfigIR) -> Result<String, String> {
    let mut out = String::from("config = {\n");
    for (i, (key, node)) in ir.iter().enumerate() {
        if i > 0 {
            out.push('\n');
        }
        emit_node(&mut out, key, node, 1);
    }
    out.push_str("}\n");

    let formatted = format_with_stylua(&out)?;

    full_moon::parse(&formatted).map_err(|errors| {
        let msgs: Vec<String> = errors
            .iter()
            .map(|e| e.error_message().to_string())
            .collect();
        format!("round-trip validation failed: {}", msgs.join("; "))
    })?;

    Ok(formatted)
}

fn format_with_stylua(source: &str) -> Result<String, String> {
    let config = stylua_lib::Config {
        indent_type: stylua_lib::IndentType::Spaces,
        indent_width: 4,
        ..stylua_lib::Config::default()
    };
    stylua_lib::format_code(source, config, None, stylua_lib::OutputVerification::None)
        .map_err(|e| format!("format error: {e}"))
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

    emit_meta_comments(out, meta.as_deref(), indent);
    emit_indent(out, indent);
    out.push_str(&escape_key(key));
    out.push_str(" = ");

    match node {
        ConfigNode::String(s) => {
            out.push('"');
            out.push_str(&escape_lua_string(&s.value));
            out.push('"');
        }
        ConfigNode::Number(n) => {
            if n.value.fract() == 0.0 {
                out.push_str(&format!("{}", n.value as i64));
            } else {
                out.push_str(&format!("{}", n.value));
            }
        }
        ConfigNode::Boolean(b) => {
            out.push_str(if b.value { "true" } else { "false" });
        }
        ConfigNode::Enum(e) => {
            out.push('"');
            out.push_str(&escape_lua_string(&e.value));
            out.push('"');
        }
        ConfigNode::Vector2(v) => {
            out.push_str(&format!("vector2({}, {})", v.x, v.y));
        }
        ConfigNode::Vector3(v) => {
            out.push_str(&format!("vector3({}, {}, {})", v.x, v.y, v.z));
        }
        ConfigNode::Table(t) => emit_table(out, &t.fields, indent),
        ConfigNode::DynamicTable(d) => emit_dynamic_table(out, &d.rows, indent),
        ConfigNode::Function(f) => {
            out.push_str(&f.value);
        }
        ConfigNode::Expression(e) => {
            out.push_str(&e.value);
        }
        ConfigNode::Nil(_) => {
            out.push_str("nil");
        }
        ConfigNode::Array(a) => emit_array(out, &a.items, indent),
    }
    out.push_str(",\n");
}

fn emit_table(out: &mut String, fields: &indexmap::IndexMap<String, ConfigNode>, indent: usize) {
    if fields.is_empty() {
        out.push_str("{}");
        return;
    }
    out.push_str("{\n");
    for (key, node) in fields {
        emit_node(out, key, node, indent + 1);
    }
    emit_indent(out, indent);
    out.push('}');
}

fn emit_dynamic_table(
    out: &mut String,
    rows: &[indexmap::IndexMap<String, serde_json::Value>],
    indent: usize,
) {
    if rows.is_empty() {
        out.push_str("{}");
        return;
    }
    out.push_str("{\n");
    for row in rows {
        let row_key = row
            .get("_key")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_default();
        emit_indent(out, indent + 1);
        out.push('[');
        out.push('"');
        out.push_str(&escape_lua_string(&row_key));
        out.push('"');
        out.push(']');
        out.push_str(" = {\n");
        for (rk, rv) in row {
            if rk == "_key" {
                continue;
            }
            emit_indent(out, indent + 2);
            out.push_str(&escape_key(rk));
            out.push_str(" = ");
            emit_json_value(out, rv);
            out.push_str(",\n");
        }
        emit_indent(out, indent + 1);
        out.push_str("},\n");
    }
    emit_indent(out, indent);
    out.push('}');
}

fn emit_array(out: &mut String, items: &[ConfigNode], indent: usize) {
    if items.is_empty() {
        out.push_str("{}");
        return;
    }
    out.push_str("{\n");
    for item in items {
        emit_indent(out, indent + 1);
        match item {
            ConfigNode::String(s) => {
                out.push('"');
                out.push_str(&escape_lua_string(&s.value));
                out.push('"');
            }
            ConfigNode::Number(n) => {
                if n.value.fract() == 0.0 {
                    out.push_str(&format!("{}", n.value as i64));
                } else {
                    out.push_str(&format!("{}", n.value));
                }
            }
            ConfigNode::Boolean(b) => {
                out.push_str(if b.value { "true" } else { "false" });
            }
            ConfigNode::Table(t) => emit_table(out, &t.fields, indent + 1),
            ConfigNode::Enum(e) => {
                out.push('"');
                out.push_str(&escape_lua_string(&e.value));
                out.push('"');
            }
            ConfigNode::Vector2(v) => {
                out.push_str(&format!("vector2({}, {})", v.x, v.y));
            }
            ConfigNode::Vector3(v) => {
                out.push_str(&format!("vector3({}, {}, {})", v.x, v.y, v.z));
            }
            ConfigNode::Function(f) => {
                out.push_str(&f.value);
            }
            ConfigNode::Expression(e) => {
                out.push_str(&e.value);
            }
            ConfigNode::Nil(_) => {
                out.push_str("nil");
            }
            ConfigNode::DynamicTable(d) => emit_dynamic_table(out, &d.rows, indent + 1),
            ConfigNode::Array(a) => emit_array(out, &a.items, indent + 1),
        }
        out.push_str(",\n");
    }
    emit_indent(out, indent);
    out.push('}');
}

fn emit_json_value(out: &mut String, val: &serde_json::Value) {
    match val {
        serde_json::Value::String(s) => {
            out.push('"');
            out.push_str(&escape_lua_string(s));
            out.push('"');
        }
        serde_json::Value::Number(n) => {
            if let Some(f) = n.as_f64() {
                if f.fract() == 0.0 {
                    out.push_str(&format!("{}", f as i64));
                } else {
                    out.push_str(&format!("{f}"));
                }
            } else {
                out.push_str(&n.to_string());
            }
        }
        serde_json::Value::Bool(b) => {
            out.push_str(if *b { "true" } else { "false" });
        }
        serde_json::Value::Object(obj) => {
            out.push_str("{\n");
            for (k, v) in obj {
                emit_indent(out, 3);
                out.push_str(&escape_key(k));
                out.push_str(" = ");
                emit_json_value(out, v);
                out.push_str(",\n");
            }
            emit_indent(out, 2);
            out.push('}');
        }
        serde_json::Value::Array(arr) => {
            out.push_str("{\n");
            for v in arr {
                emit_indent(out, 3);
                emit_json_value(out, v);
                out.push_str(",\n");
            }
            emit_indent(out, 2);
            out.push('}');
        }
        serde_json::Value::Null => {
            out.push_str("nil");
        }
    }
}

fn emit_meta_comments(out: &mut String, meta: Option<&FieldMetadata>, indent: usize) {
    let meta = match meta {
        Some(m) => m,
        None => return,
    };

    // Sanitize helpers for comment content
    let sanitize_line = |s: &str| -> String { s.replace('\n', " ").replace('\r', " ") };
    let sanitize_block =
        |s: &str| -> String { s.replace("]]", "] ]").replace('\n', " ").replace('\r', " ") };

    if let Some(desc) = &meta.description {
        emit_indent(out, indent);
        out.push_str("--//");
        out.push_str(&sanitize_line(desc));
        out.push('\n');
    }

    if let Some(range) = &meta.range {
        emit_indent(out, indent);
        out.push_str("--!RANGE = { min = ");
        emit_smart_number(out, range.min);
        out.push_str(", max = ");
        emit_smart_number(out, range.max);
        out.push_str(" }\n");
    }

    if let Some(options) = &meta.enum_options {
        emit_indent(out, indent);
        out.push_str("--!ENUM = { ");
        for (i, opt) in options.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push('"');
            out.push_str(&escape_lua_string(opt));
            out.push('"');
        }
        out.push_str(" }\n");
    }

    if let Some(map) = &meta.map {
        emit_indent(out, indent);
        out.push_str("--!MAP = ");
        out.push_str(if *map { "true" } else { "false" });
        out.push('\n');
    }

    if let Some(func_info) = &meta.function_info {
        emit_indent(out, indent);
        out.push_str("--[[CFX_FUNCTION = { resource_name = \"");
        out.push_str(&sanitize_block(&func_info.resource_name));
        out.push_str("\", function_name = \"");
        out.push_str(&sanitize_block(&func_info.function_name));
        out.push_str("\" }]]\n");
    }

    if let Some(schema) = &meta.table_schema {
        emit_indent(out, indent);
        out.push_str("--[[TABLE = {\n");
        emit_indent(out, indent + 1);
        out.push_str("layout = \"");
        out.push_str(&sanitize_block(&schema.layout));
        out.push_str("\",\n");
        emit_indent(out, indent + 1);
        out.push_str("schema = {\n");
        for col in &schema.schema {
            emit_indent(out, indent + 2);
            out.push_str("{ ");
            write!(out, "name = \"{}\"", sanitize_block(&col.name)).ok();
            write!(out, ", type = \"{}\"", sanitize_block(&col.column_type)).ok();
            if let Some(true) = col.is_key {
                out.push_str(", is_key = true");
            }
            if let Some(label) = &col.label {
                write!(out, ", label = \"{}\"", sanitize_block(label)).ok();
            }
            if let Some(desc) = &col.description {
                write!(out, ", description = \"{}\"", sanitize_block(desc)).ok();
            }
            out.push_str(" },\n");
        }
        emit_indent(out, indent + 1);
        out.push_str("},\n");
        emit_indent(out, indent);
        out.push_str("}]]\n");
    }
}

fn emit_indent(out: &mut String, indent: usize) {
    for _ in 0..indent {
        out.push_str("    ");
    }
}

fn emit_smart_number(out: &mut String, val: f64) {
    if val.fract() == 0.0 {
        write!(out, "{}", val as i64).ok();
    } else {
        write!(out, "{val}").ok();
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
    if key.contains(|c: char| !c.is_alphanumeric() && c != '_')
        || key.is_empty()
        || key.starts_with(|c: char| c.is_ascii_digit())
        || LUA_KEYWORDS.contains(&key)
    {
        format!("[\"{}\"]", key.replace('\\', "\\\\").replace('"', "\\\""))
    } else {
        key.to_string()
    }
}

const LUA_KEYWORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];
