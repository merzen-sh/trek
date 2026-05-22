use std::fmt::Write;

use crate::models::{
    ConfigDoc, ConfigIR, ConfigNode, ScalarMeta, EnumMeta, TableMeta, TableSchema,
    ColumnDef, ColumnType, CfxFunctionMeta, ArgDef,
};
use indexmap::IndexMap;
use serde_json::Value;

pub fn generate(doc: &ConfigDoc) -> String {
    let mut out = String::new();
    write_meta_block(doc.meta.as_ref(), &mut out);
    out.push_str("config = {\n");
    write_fields(&doc.fields, &mut out, 1);
    out.push_str("}\n");
    out
}

fn indent(out: &mut String, depth: usize) {
    for _ in 0..depth {
        out.push_str("    ");
    }
}

fn is_lua_ident(s: &str) -> bool {
    // Lua reserved keywords cannot be used as bare field names.
    const KEYWORDS: &[&str] = &[
        "and", "break", "do", "else", "elseif", "end", "false", "for",
        "function", "if", "in", "local", "nil", "not", "or", "repeat",
        "return", "then", "true", "until", "while",
    ];
    if KEYWORDS.contains(&s) {
        return false;
    }
    if s.is_empty() {
        return false;
    }
    let mut chars = s.chars();
    let first = chars.next().unwrap();
    (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

fn write_key(key: &str, out: &mut String, depth: usize) {
    indent(out, depth);
    if is_lua_ident(key) {
        out.push_str(key);
    } else {
        write!(out, "[{}]", escape_lua(key)).unwrap();
    }
    out.push_str(" = ");
}

fn escape_lua(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if c.is_ascii_control() => {
                let _ = write!(out, "\\x{:02x}", c as u8);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn write_meta_block(meta: Option<&IndexMap<String, Value>>, out: &mut String) {
    let Some(meta) = meta else { return };
    if meta.is_empty() {
        return;
    }
    out.push_str("--[[\n");
    for (k, v) in meta {
        let val = match v {
            Value::String(s) => format!("\"{}\"", s.replace('\\', "\\\\").replace('"', "\\\"")),
            other => other.to_string(),
        };
        let _ = writeln!(out, "{k} = {val}");
    }
    out.push_str("]]\n");
}

fn write_fields(fields: &ConfigIR, out: &mut String, depth: usize) {
    for (key, node) in fields {
        write_node(key, node, out, depth);
    }
}

fn write_node(key: &str, node: &ConfigNode, out: &mut String, depth: usize) {
    match node {
        ConfigNode::String(n) => {
            write_scalar_meta(n.metadata.as_ref(), out, depth);
            write_key(key, out, depth);
            out.push_str(&escape_lua(&n.value));
            out.push_str(",\n");
        }
        ConfigNode::Number(n) => {
            write_scalar_meta(n.metadata.as_ref(), out, depth);
            write_key(key, out, depth);
            out.push_str(&n.value);
            out.push_str(",\n");
        }
        ConfigNode::Boolean(n) => {
            write_scalar_meta(n.metadata.as_ref(), out, depth);
            write_key(key, out, depth);
            out.push_str(if n.value { "true" } else { "false" });
            out.push_str(",\n");
        }
        ConfigNode::Enum(n) => {
            write_enum_meta(n.metadata.as_ref(), out, depth);
            write_key(key, out, depth);
            out.push_str(&escape_lua(&n.value));
            out.push_str(",\n");
        }
        ConfigNode::Vector2(n) => {
            write_scalar_meta(n.metadata.as_ref(), out, depth);
            write_key(key, out, depth);
            let _ = write!(out, "vector2({}, {})", fmt_float(n.value.x), fmt_float(n.value.y));
            out.push_str(",\n");
        }
        ConfigNode::Vector3(n) => {
            write_scalar_meta(n.metadata.as_ref(), out, depth);
            write_key(key, out, depth);
            let _ = write!(out, "vector3({}, {}, {})", fmt_float(n.value.x), fmt_float(n.value.y), fmt_float(n.value.z));
            out.push_str(",\n");
        }
        ConfigNode::Table(n) => {
            write_table_meta(n.metadata.as_ref(), out, depth);
            write_key(key, out, depth);
            out.push_str("{\n");
            write_fields(&n.rows, out, depth + 1);
            indent(out, depth);
            out.push_str("},\n");
        }
        ConfigNode::CfxFunction(n) => {
            write_cfx_meta(&n.metadata, out, depth);
            write_key(key, out, depth);
            out.push_str("function() end,\n");
        }
    }
}

fn write_descriptions(desc: &Option<Vec<String>>, out: &mut String, depth: usize) {
    let Some(lines) = desc else { return };
    for line in lines {
        indent(out, depth);
        out.push_str("--!");
        out.push_str(line);
        out.push('\n');
    }
}

fn write_scalar_meta(meta: Option<&ScalarMeta>, out: &mut String, depth: usize) {
    let Some(meta) = meta else { return };
    write_descriptions(&meta.description, out, depth);
    if let Some(range) = &meta.range {
        indent(out, depth);
        out.push_str("--@RANGE = { ");
        for (i, v) in range.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(v);
        }
        out.push_str(" }\n");
    }
}

fn write_enum_meta(meta: Option<&EnumMeta>, out: &mut String, depth: usize) {
    let Some(meta) = meta else { return };
    write_descriptions(&meta.description, out, depth);
    indent(out, depth);
    out.push_str("--@ENUM = { ");
    for (i, opt) in meta.options.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        out.push_str(&escape_lua(opt));
    }
    out.push_str(" }\n");
}

fn write_table_meta(meta: Option<&TableMeta>, out: &mut String, depth: usize) {
    let Some(meta) = meta else { return };
    write_descriptions(&meta.description, out, depth);
    if let Some(schema) = &meta.schema {
        write_table_schema(schema, out, depth);
    }
}

fn write_table_schema(schema: &TableSchema, out: &mut String, depth: usize) {
    indent(out, depth);
    out.push_str("--[[@TABLE = {\n");
    let inner = depth + 1;
    indent(out, inner);
    let _ = writeln!(out, "allow_add = {},", bool_str(schema.allow_add));
    indent(out, inner);
    let _ = writeln!(out, "allow_delete = {},", bool_str(schema.allow_delete));
    indent(out, inner);
    let _ = writeln!(out, "allow_edit = {},", bool_str(schema.allow_edit));
    if !schema.columns.is_empty() {
        indent(out, inner);
        out.push_str("schema = {\n");
        for col in &schema.columns {
            indent(out, inner + 1);
            write_column_def(col, out);
        }
        indent(out, inner);
        out.push_str("},\n");
    }
    indent(out, depth);
    out.push_str("}]]\n");
}

fn write_column_def(col: &ColumnDef, out: &mut String) {
    out.push('{');
    write!(out, " field = {}", escape_lua(&col.field)).unwrap();
    write!(out, ", type = {}", escape_lua(col_type_str(&col.col_type))).unwrap();
    write!(out, ", label = {}", escape_lua(&col.label)).unwrap();
    if !col.values.is_empty() {
        out.push_str(", values = { ");
        for (i, v) in col.values.iter().enumerate() {
            if i > 0 {
                out.push_str(", ");
            }
            out.push_str(&escape_lua(v));
        }
        out.push_str(" }");
    }
    out.push_str(" },\n");
}

fn col_type_str(t: &ColumnType) -> &'static str {
    match t {
        ColumnType::Key => "key",
        ColumnType::String => "string",
        ColumnType::Number => "number",
        ColumnType::Enum => "enum",
        ColumnType::Boolean => "boolean",
        ColumnType::Unknown => "unknown",
    }
}

fn write_cfx_meta(meta: &CfxFunctionMeta, out: &mut String, depth: usize) {
    write_descriptions(&meta.description, out, depth);
    indent(out, depth);
    out.push_str("--[[@CFX_FUNCTION = {\n");
    let inner = depth + 1;
    indent(out, inner);
    out.push_str("args_schema = {\n");
    for arg in &meta.args_schema {
        indent(out, inner + 1);
        write_arg_def(arg, out);
    }
    indent(out, inner);
    out.push_str("},\n");
    indent(out, depth);
    out.push_str("}]]\n");
}

fn write_arg_def(arg: &ArgDef, out: &mut String) {
    out.push('{');
    write!(out, " name = {}", escape_lua(&arg.name)).unwrap();
    write!(out, ", type = {}", escape_lua(col_type_str(&arg.arg_type))).unwrap();
    write!(out, ", label = {}", escape_lua(&arg.label)).unwrap();
    if arg.required {
        out.push_str(", required = true");
    } else {
        out.push_str(", required = false");
    }
    out.push_str(" },\n");
}

fn bool_str(b: bool) -> &'static str {
    if b {
        "true"
    } else {
        "false"
    }
}

fn fmt_float(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{n:.1}")
    } else {
        format!("{n}")
    }
}
