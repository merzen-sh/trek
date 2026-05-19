use crate::models::*;
use crate::trivia_parser;
use full_moon::ast::*;
use full_moon::tokenizer::{Token, TokenReference, TokenType};
use indexmap::IndexMap;

pub fn build_ir(source: &str) -> Result<ConfigIR, String> {
    let ast = full_moon::parse(source).map_err(|errors| {
        let msgs: Vec<String> = errors
            .iter()
            .map(|e| e.error_message().to_string())
            .collect();
        format!("parse error: {}", msgs.join("; "))
    })?;
    for stmt in ast.nodes().stmts() {
        match try_extract_config_assignment(stmt) {
            Ok(Some(ir)) => return Ok(ir),
            Ok(None) => {}
            Err(e) => return Err(e),
        }
    }
    Err("no config table assignment found".to_string())
}

pub fn validate_ir(ir: &ConfigIR) -> Result<(), String> {
    let mut errors = Vec::new();
    for (key, node) in ir {
        collect_validation_errors(key, node, "", &mut errors);
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("\n"))
    }
}

fn collect_validation_errors(
    field_name: &str,
    node: &ConfigNode,
    path: &str,
    errors: &mut Vec<String>,
) {
    let prefix = if path.is_empty() {
        field_name.to_string()
    } else {
        format!("{}.{}", path, field_name)
    };

    let meta = match node {
        ConfigNode::String(s) => s.metadata.as_deref(),
        ConfigNode::Number(n) => n.metadata.as_deref(),
        ConfigNode::Boolean(b) => b.metadata.as_deref(),
        ConfigNode::Enum(e) => e.metadata.as_deref(),
        ConfigNode::Vector2(v) => v.metadata.as_deref(),
        ConfigNode::Vector3(v) => v.metadata.as_deref(),
        ConfigNode::Table(t) => t.metadata.as_deref(),
        ConfigNode::DynamicTable(d) => d.metadata.as_deref(),
        ConfigNode::Function(f) => f.metadata.as_deref(),
        ConfigNode::Expression(e) => e.metadata.as_deref(),
        ConfigNode::Nil(n) => n.metadata.as_deref(),
        ConfigNode::Array(a) => a.metadata.as_deref(),
    };

    let Some(meta) = meta else { return };

    // RANGE can only apply to Number
    if meta.range.is_some() && !matches!(node, ConfigNode::Number(_)) {
        errors.push(format!(
            "Validation Error: Field '{}' has RANGE annotation but is not a number",
            prefix
        ));
    }

    // ENUM can only apply to String or Enum
    if let Some(options) = &meta.enum_options {
        if options.is_empty() {
            errors.push(format!(
                "Validation Error: Field '{}' has empty ENUM options list",
                prefix
            ));
        }
        match node {
            ConfigNode::Enum(ev) => {
                if !options.contains(&ev.value) {
                    errors.push(format!(
                        "Validation Error: Field '{}' value '{}' not in ENUM options [{}]",
                        prefix,
                        ev.value,
                        options.join(", ")
                    ));
                }
            }
            ConfigNode::String(sv) => {
                if !options.contains(&sv.value) {
                    errors.push(format!(
                        "Validation Error: Field '{}' value '{}' not in ENUM options [{}]",
                        prefix,
                        sv.value,
                        options.join(", ")
                    ));
                }
            }
            _ => {
                errors.push(format!(
                    "Validation Error: Field '{}' has ENUM annotation but value is not a string",
                    prefix
                ));
            }
        }
    }

    // MAP can only apply to Vector2 or Vector3
    if let Some(true) = meta.map {
        if !matches!(node, ConfigNode::Vector2(_) | ConfigNode::Vector3(_)) {
            errors.push(format!(
                "Validation Error: Field '{}' has MAP=true annotation but is not a vector2/vector3",
                prefix
            ));
        }
    }

    // RANGE bounds
    if let Some(range) = &meta.range {
        if let ConfigNode::Number(nv) = node {
            if nv.value < range.min || nv.value > range.max {
                errors.push(format!(
                    "Validation Error: Field '{}' value {} is outside RANGE [{}, {}]",
                    prefix, nv.value, range.min, range.max
                ));
            }
        }
    }

    // CFX_FUNCTION can only apply to Function or Expression
    if meta.function_info.is_some() {
        if !matches!(node, ConfigNode::Function(_) | ConfigNode::Expression(_)) {
            errors.push(format!(
                "Validation Error: Field '{}' has CFX_FUNCTION annotation but value is not a function",
                prefix
            ));
        }
    }

    // Recursively validate sub-tables
    match node {
        ConfigNode::Table(t) => {
            for (k, v) in &t.fields {
                collect_validation_errors(k, v, &prefix, errors);
            }
        }
        ConfigNode::Array(a) => {
            for (i, v) in a.items.iter().enumerate() {
                let idx = format!("{}[{}]", prefix, i);
                // Only recurse into table elements
                if let ConfigNode::Table(t) = v {
                    for (k, sub) in &t.fields {
                        collect_validation_errors(k, sub, &idx, errors);
                    }
                }
            }
        }
        _ => {}
    }
}

fn try_extract_config_assignment(stmt: &Stmt) -> Result<Option<ConfigIR>, String> {
    let assignment = match stmt {
        Stmt::Assignment(a) => a,
        _ => return Ok(None),
    };
    for (var, expr) in assignment
        .variables()
        .iter()
        .zip(assignment.expressions().iter())
    {
        if var_name_str(var).is_none() {
            continue;
        }
        let tc = match expr {
            Expression::TableConstructor(tc) => tc,
            _ => continue,
        };
        return visit_table(tc).map(Some);
    }
    Ok(None)
}

fn var_name_str(var: &Var) -> Option<String> {
    match var {
        Var::Name(ref_token) => Some(token_text(ref_token)),
        Var::Expression(_) => None,
        _ => None,
    }
}

fn token_text(t: &TokenReference) -> String {
    t.token().to_string()
}

pub fn visit_table(tc: &TableConstructor) -> Result<ConfigIR, String> {
    let mut result = IndexMap::new();
    for field in tc.fields().iter() {
        let (key, expr) = match field_key_expr(field) {
            Some(kv) => kv,
            None => continue,
        };
        let meta = extract_metadata_from_trivia(field);
        let node = visit_expression(expr, meta)?;
        check_lua_key_ok(&key, &field)?;
        if result.contains_key(&key) {
            let line = field_line(field);
            return Err(format!(
                "line {}: Duplicate key '{}' in table constructor",
                line, key
            ));
        }
        result.insert(key, node);
    }
    Ok(result)
}

fn field_line(field: &Field) -> usize {
    match field {
        Field::NameKey { key, .. } => key.start_position().line(),
        Field::ExpressionKey { key, .. } => match key {
            Expression::String(token) => token.start_position().line(),
            _ => 0,
        },
        Field::NoKey(expr) => expr_line(expr),
        _ => 0,
    }
}

fn expr_line(expr: &Expression) -> usize {
    match expr {
        Expression::String(token) => token.start_position().line(),
        Expression::Number(token) => token.start_position().line(),
        Expression::Symbol(token) => token.start_position().line(),
        Expression::TableConstructor(tc) => {
            tc.fields().iter().next().map(|f| field_line(f)).unwrap_or(0)
        }
        _ => 0,
    }
}

fn check_lua_key_ok(key: &str, field: &Field) -> Result<(), String> {
    let line = field_line(field);
    if key.is_empty() {
        return Err(format!("line {}: Empty string used as table key", line));
    }
    if key.contains('\0') {
        return Err(format!(
            "line {}: Key contains null byte: '{}'",
            line, key
        ));
    }
    if key.len() > 200 {
        return Err(format!(
            "line {}: Key exceeds maximum length (200 chars): '{}'",
            line, &key[..40]
        ));
    }
    Ok(())
}

fn visit_field(field: &Field) -> Result<ConfigNode, String> {
    let meta = extract_metadata_from_trivia(field);
    let expr = match field {
        Field::NameKey { value, .. } | Field::ExpressionKey { value, .. } => value,
        Field::NoKey(expr) => expr,
        _ => return Ok(ConfigNode::Nil(NilValue { metadata: None })),
    };
    visit_expression(expr, meta)
}

fn visit_expression(expr: &Expression, meta: FieldMetadata) -> Result<ConfigNode, String> {
    match expr {
        Expression::String(token) => {
            let value = token_string_value(token);
            Ok(maybe_enum(value, meta))
        }
        Expression::Number(token) => {
            let raw = token.token().to_string();
            let value: f64 = raw.parse().unwrap_or_else(|_| {
                eprintln!(
                    "WARN: failed to parse number from token: '{raw}' (via token().to_string())"
                );
                eprintln!("  full token: '{}'", token.to_string());
                0.0
            });
            Ok(node_with_meta(
                ConfigNode::Number(NumberValue {
                    value,
                    metadata: None,
                }),
                meta,
            ))
        }
        Expression::Symbol(token) => {
            let s = token_text(token);
            match s.as_str() {
                "true" => Ok(node_with_meta(
                    ConfigNode::Boolean(BooleanValue {
                        value: true,
                        metadata: None,
                    }),
                    meta,
                )),
                "false" => Ok(node_with_meta(
                    ConfigNode::Boolean(BooleanValue {
                        value: false,
                        metadata: None,
                    }),
                    meta,
                )),
                "nil" => Ok(node_with_meta(ConfigNode::Nil(NilValue { metadata: None }), meta)),
                _ => Ok(node_with_meta(
                    ConfigNode::Expression(ExpressionValue {
                        value: s,
                        metadata: None,
                    }),
                    meta,
                )),
            }
        }
        Expression::TableConstructor(tc) => visit_table_value(tc, meta),
        Expression::FunctionCall(fc) => Ok(visit_function_call(fc, meta)),
        Expression::Function(anon_fn) => {
            let value = anon_fn.to_string();
            if value.starts_with("function") {
                Ok(node_with_meta(
                    ConfigNode::Function(FunctionValue {
                        value,
                        metadata: None,
                    }),
                    meta,
                ))
            } else {
                Ok(node_with_meta(
                    ConfigNode::Expression(ExpressionValue {
                        value,
                        metadata: None,
                    }),
                    meta,
                ))
            }
        }
        Expression::UnaryOperator { unop, expression } => {
            if let UnOp::Minus(_) = unop {
                if let Expression::Number(token) = expression.as_ref() {
                    if let Ok(val) = token.token().to_string().parse::<f64>() {
                        return Ok(node_with_meta(
                            ConfigNode::Number(NumberValue {
                                value: -val,
                                metadata: None,
                            }),
                            meta,
                        ));
                    }
                }
            }
            let inner = visit_expression(expression, FieldMetadata::default())?;
            let inner_str = expr_to_str(&inner);
            let op_str = match unop {
                UnOp::Minus(_) => "-",
                UnOp::Not(_) => "not ",
                UnOp::Hash(_) => "#",
                _ => "",
            };
            Ok(node_with_meta(
                ConfigNode::Expression(ExpressionValue {
                    value: format!("{op_str}{inner_str}"),
                    metadata: None,
                }),
                meta,
            ))
        }
        Expression::BinaryOperator { lhs, binop, rhs } => {
            let lhs_str = expr_to_str(&visit_expression(lhs, FieldMetadata::default())?);
            let rhs_str = expr_to_str(&visit_expression(rhs, FieldMetadata::default())?);
            let op_str = binop_operator_str(binop);
            Ok(node_with_meta(
                ConfigNode::Expression(ExpressionValue {
                    value: format!("{lhs_str} {op_str} {rhs_str}"),
                    metadata: None,
                }),
                meta,
            ))
        }
        Expression::Parentheses { expression, .. } => {
            let inner = visit_expression(expression, FieldMetadata::default())?;
            let inner_str = expr_to_str(&inner);
            Ok(node_with_meta(
                ConfigNode::Expression(ExpressionValue {
                    value: format!("({inner_str})"),
                    metadata: None,
                }),
                meta,
            ))
        }
        _ => {
            let value = expr.to_string();
            if value.starts_with("function") || value.starts_with("function(") {
                Ok(node_with_meta(
                    ConfigNode::Function(FunctionValue {
                        value,
                        metadata: None,
                    }),
                    meta,
                ))
            } else {
                Ok(node_with_meta(
                    ConfigNode::Expression(ExpressionValue {
                        value,
                        metadata: None,
                    }),
                    meta,
                ))
            }
        }
    }
}

fn visit_table_value(tc: &TableConstructor, meta: FieldMetadata) -> Result<ConfigNode, String> {
    if let Some(schema) = &meta.table_schema {
        if schema.layout == "items" {
            let mut rows = Vec::new();
            for field in tc.fields().iter() {
                let row_key = match field_key_str(field) {
                    Some(k) => k,
                    None => continue,
                };
                check_lua_key_ok(&row_key, &field)?;
                let mut row = IndexMap::new();
                row.insert("_key".to_string(), serde_json::Value::String(row_key));
                populate_dynamic_row(field, &mut row);
                rows.push(row);
            }
            return Ok(ConfigNode::DynamicTable(DynamicTableValue {
                rows,
                metadata: Some(Box::new(meta)),
            }));
        }
    }

    let has_fields = tc.fields().iter().next().is_some();
    let all_nokey = has_fields && tc.fields().iter().all(|f| matches!(f, Field::NoKey(_)));
    if all_nokey {
        let mut items = Vec::new();
        for field in tc.fields().iter() {
            let node = visit_field(field)?;
            items.push(node);
        }
        return Ok(ConfigNode::Array(ArrayValue {
            items,
            metadata: Some(Box::new(meta)),
        }));
    }

    let mut fields = IndexMap::new();
    for field in tc.fields().iter() {
        let (key, _) = match field_key_expr(field) {
            Some(kv) => kv,
            None => continue,
        };
        check_lua_key_ok(&key, &field)?;
        let node = visit_field(field)?;
        if fields.contains_key(&key) {
            let line = field_line(field);
            return Err(format!(
                "line {}: Duplicate key '{}' in table constructor",
                line, key
            ));
        }
        fields.insert(key, node);
    }
    Ok(ConfigNode::Table(TableValue {
        fields,
        metadata: Some(Box::new(meta)),
    }))
}

fn populate_dynamic_row(field: &Field, row: &mut IndexMap<String, serde_json::Value>) {
    let tc = match field {
        Field::NameKey {
            value: Expression::TableConstructor(tc),
            ..
        }
        | Field::ExpressionKey {
            value: Expression::TableConstructor(tc),
            ..
        } => tc,
        _ => return,
    };
    for subfield in tc.fields().iter() {
        let key = match field_key_str(subfield) {
            Some(k) => k,
            None => continue,
        };
        let val = match field_value_expr(subfield) {
            Some(expr) => expr_to_json_value(expr),
            None => continue,
        };
        row.insert(key, val);
    }
}

fn expr_to_json_value(expr: &Expression) -> serde_json::Value {
    match expr {
        Expression::String(token) => serde_json::Value::String(token_string_value(token)),
        Expression::Number(token) => {
            let raw = token.token().to_string();
            if let Ok(v) = raw.parse::<f64>() {
                if v.fract() == 0.0 {
                    serde_json::Value::Number(serde_json::Number::from(v as i64))
                } else {
                    serde_json::Value::Number(
                        serde_json::Number::from_f64(v).unwrap_or(serde_json::Number::from(0)),
                    )
                }
            } else {
                serde_json::Value::Null
            }
        }
        // Handle negative number literals: -100, -3.14, etc.
        Expression::UnaryOperator { unop, expression } if matches!(unop, UnOp::Minus(_)) => {
            if let Expression::Number(token) = expression.as_ref() {
                let raw = token.token().to_string();
                if let Ok(v) = raw.parse::<f64>() {
                    let neg = -v;
                    if neg.fract() == 0.0 {
                        return serde_json::Value::Number(serde_json::Number::from(neg as i64));
                    } else {
                        return serde_json::Value::Number(
                            serde_json::Number::from_f64(neg)
                                .unwrap_or(serde_json::Number::from(0)),
                        );
                    }
                }
            }
            serde_json::Value::String(expr.to_string())
        }
        Expression::Symbol(token) => match token_text(token).as_str() {
            "true" => serde_json::Value::Bool(true),
            "false" => serde_json::Value::Bool(false),
            _ => serde_json::Value::Null,
        },
        Expression::TableConstructor(tc) => {
            let mut map = serde_json::Map::new();
            for field in tc.fields().iter() {
                if let Some(key) = field_key_str(field) {
                    if let Some(expr) = field_value_expr(field) {
                        map.insert(key, expr_to_json_value(expr));
                    }
                }
            }
            serde_json::Value::Object(map)
        }
        Expression::FunctionCall(fc) => serde_json::Value::String(fc.to_string()),
        _ => serde_json::Value::String(expr.to_string()),
    }
}

fn visit_function_call(fc: &FunctionCall, meta: FieldMetadata) -> ConfigNode {
    let name = prefix_name_str(fc.prefix());
    let first_suffix = fc.suffixes().next();
    let call_args = first_suffix.and_then(|s| match s {
        Suffix::Call(call) => match call {
            Call::AnonymousCall(args) => Some(args),
            _ => None,
        },
        _ => None,
    });

    match name.as_deref() {
        Some("vector2") => {
            let vals: Vec<f64> = extract_number_args(call_args);
            if vals.len() >= 2 {
                return ConfigNode::Vector2(Vector2Value {
                    x: vals[0],
                    y: vals[1],
                    metadata: Some(Box::new(meta)),
                });
            }
            fallback_expr(fc, meta)
        }
        Some("vector3") => {
            let vals: Vec<f64> = extract_number_args(call_args);
            if vals.len() >= 3 {
                return ConfigNode::Vector3(Vector3Value {
                    x: vals[0],
                    y: vals[1],
                    z: vals[2],
                    metadata: Some(Box::new(meta)),
                });
            }
            fallback_expr(fc, meta)
        }
        _ => {
            let call_str = fc.to_string();
            if call_str.starts_with("function") {
                ConfigNode::Function(FunctionValue {
                    value: call_str,
                    metadata: Some(Box::new(meta)),
                })
            } else {
                ConfigNode::Expression(ExpressionValue {
                    value: call_str,
                    metadata: Some(Box::new(meta)),
                })
            }
        }
    }
}

fn extract_number_args(args: Option<&full_moon::ast::FunctionArgs>) -> Vec<f64> {
    let args = match args {
        Some(fa) => fa,
        None => return Vec::new(),
    };
    match args {
        full_moon::ast::FunctionArgs::Parentheses { arguments, .. } => arguments
            .iter()
            .filter_map(|e| match e {
                Expression::Number(token) => token.token().to_string().parse::<f64>().ok(),
                // Handle negative numbers: UnaryOperator { Minus, Number }
                Expression::UnaryOperator { unop, expression }
                    if matches!(unop, UnOp::Minus(_)) =>
                {
                    if let Expression::Number(token) = expression.as_ref() {
                        token.token().to_string().parse::<f64>().ok().map(|v| -v)
                    } else {
                        None
                    }
                }
                _ => None,
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn prefix_name_str(prefix: &Prefix) -> Option<String> {
    match prefix {
        Prefix::Name(token) => Some(token_text(token)),
        Prefix::Expression(expr) => Some(expr.to_string()),
        _ => None,
    }
}

fn fallback_expr(fc: &FunctionCall, meta: FieldMetadata) -> ConfigNode {
    ConfigNode::Expression(ExpressionValue {
        value: fc.to_string(),
        metadata: Some(Box::new(meta)),
    })
}

// ---- Metadata extraction from trivia ----

fn extract_metadata_from_trivia(field: &Field) -> FieldMetadata {
    let mut meta = FieldMetadata::default();
    let trivia_list: Vec<&Token> = field_key_trivia(field);
    for token in &trivia_list {
        match token.token_type() {
            TokenType::SingleLineComment { comment } => {
                let text = comment.to_string();
                if text.starts_with('/') && text.get(1..2) == Some("/") {
                    meta.description = Some(text[2..].trim().to_string());
                    continue;
                }
                if let Some(ann) = trivia_parser::parse_trivia_token(token) {
                    if let Ok(parsed) = trivia_parser::annotation_to_metadata(&ann) {
                        meta.merge(parsed);
                    }
                }
            }
            TokenType::MultiLineComment { .. } => {
                if let Some(ann) = trivia_parser::parse_trivia_token(token) {
                    if let Ok(parsed) = trivia_parser::annotation_to_metadata(&ann) {
                        meta.merge(parsed);
                    }
                }
            }
            _ => {}
        }
    }
    meta
}

fn field_key_trivia(field: &Field) -> Vec<&Token> {
    match field {
        Field::NameKey { key, .. } => key.leading_trivia().collect(),
        Field::ExpressionKey { key, .. } => match key {
            Expression::String(token) => token.leading_trivia().collect(),
            _ => Vec::new(),
        },
        Field::NoKey(expr) => match expr {
            Expression::String(token) => token.leading_trivia().collect(),
            Expression::Number(token) => token.leading_trivia().collect(),
            Expression::Symbol(token) => token.leading_trivia().collect(),
            _ => Vec::new(),
        },
        _ => Vec::new(),
    }
}

fn field_key_expr(field: &Field) -> Option<(String, &Expression)> {
    match field {
        Field::NameKey { key, value, .. } => Some((token_text(key), value)),
        Field::ExpressionKey { key, value, .. } => {
            let name = match key {
                Expression::String(token) => token_string_value(token),
                _ => return None,
            };
            Some((name, value))
        }
        Field::NoKey(_) => None,
        _ => None,
    }
}

fn field_key_str(field: &Field) -> Option<String> {
    match field {
        Field::NameKey { key, .. } => Some(token_text(key)),
        Field::ExpressionKey { key, .. } => {
            if let Expression::String(token) = key {
                Some(token_string_value(token))
            } else {
                None
            }
        }
        Field::NoKey(_) => None,
        _ => None,
    }
}

fn field_value_expr(field: &Field) -> Option<&Expression> {
    match field {
        Field::NameKey { value, .. } => Some(value),
        Field::ExpressionKey { value, .. } => Some(value),
        Field::NoKey(expr) => Some(expr),
        _ => None,
    }
}

fn token_string_value(token: &TokenReference) -> String {
    let raw = if let TokenType::StringLiteral { literal, .. } = token.token().token_type() {
        literal.to_string()
    } else {
        token.token().to_string().trim_matches('"').to_string()
    };
    unescape_lua_string(&raw)
}

fn unescape_lua_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            None => out.push('\\'),
            Some('\\') => out.push('\\'),
            Some('"') => out.push('"'),
            Some('\'') => out.push('\''),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('b') => out.push('\u{0008}'),
            Some('f') => out.push('\u{000C}'),
            Some('a') => out.push('\u{0007}'),
            Some('v') => out.push('\u{000B}'),
            Some('z') => {
                while matches!(chars.peek(), Some(' ' | '\t' | '\n' | '\r')) {
                    chars.next();
                }
            }
            Some('x') => {
                let hex: String = chars
                    .by_ref()
                    .take(2)
                    .take_while(|c| c.is_ascii_hexdigit())
                    .collect();
                out.push(u8::from_str_radix(&hex, 16).unwrap_or(0) as char);
            }
            Some('u') => {
                if chars.next() != Some('{') {
                    out.push('u');
                    continue;
                }
                let hex: String = chars.by_ref().take_while(|c| *c != '}').collect();
                if let Ok(codepoint) = u32::from_str_radix(&hex, 16) {
                    if let Some(ch) = char::from_u32(codepoint) {
                        out.push(ch);
                    }
                }
                let _ = chars.next(); // skip '}'
            }
            Some(d @ '0'..='9') => {
                let digits = {
                    let mut s = String::from(d);
                    for _ in 0..2 {
                        match chars.peek() {
                            Some(c @ '0'..='9') => {
                                s.push(*c);
                                chars.next();
                            }
                            _ => break,
                        }
                    }
                    s
                };
                if let Ok(byte) = u8::from_str_radix(&digits, 10) {
                    out.push(byte as char);
                }
            }
            Some(other) => {
                out.push(other);
            }
        }
    }
    out
}

fn binop_operator_str(binop: &BinOp) -> &'static str {
    match binop {
        BinOp::Plus(_) => "+",
        BinOp::Minus(_) => "-",
        BinOp::Star(_) => "*",
        BinOp::Slash(_) => "/",
        BinOp::Caret(_) => "^",
        BinOp::Percent(_) => "%",
        BinOp::TwoDots(_) => "..",
        BinOp::TwoEqual(_) => "==",
        BinOp::TildeEqual(_) => "~=",
        BinOp::LessThan(_) => "<",
        BinOp::LessThanEqual(_) => "<=",
        BinOp::GreaterThan(_) => ">",
        BinOp::GreaterThanEqual(_) => ">=",
        BinOp::And(_) => "and",
        BinOp::Or(_) => "or",
        _ => "?",
    }
}

fn expr_to_str(node: &ConfigNode) -> String {
    match node {
        ConfigNode::String(s) => format!("\"{}\"", s.value),
        ConfigNode::Number(n) => {
            if n.value.fract() == 0.0 {
                format!("{}", n.value as i64)
            } else {
                format!("{}", n.value)
            }
        }
        ConfigNode::Boolean(b) => b.value.to_string(),
        ConfigNode::Expression(e) => e.value.clone(),
        ConfigNode::Function(f) => f.value.clone(),
        ConfigNode::Nil(_) => "nil".to_string(),
        _ => format!("{node:?}"),
    }
}

fn maybe_enum(value: String, meta: FieldMetadata) -> ConfigNode {
    if meta.enum_options.is_some() {
        ConfigNode::Enum(EnumValue {
            value,
            metadata: Some(Box::new(meta)),
        })
    } else {
        ConfigNode::String(StringValue {
            value,
            metadata: Some(Box::new(meta)),
        })
    }
}

fn node_with_meta(node: ConfigNode, meta: FieldMetadata) -> ConfigNode {
    if meta_is_empty(&meta) {
        return node;
    }
    let b = Box::new(meta);
    match node {
        ConfigNode::String(mut v) => {
            v.metadata = Some(b);
            ConfigNode::String(v)
        }
        ConfigNode::Number(mut v) => {
            v.metadata = Some(b);
            ConfigNode::Number(v)
        }
        ConfigNode::Boolean(mut v) => {
            v.metadata = Some(b);
            ConfigNode::Boolean(v)
        }
        ConfigNode::Enum(mut v) => {
            v.metadata = Some(b);
            ConfigNode::Enum(v)
        }
        ConfigNode::Vector2(mut v) => {
            v.metadata = Some(b);
            ConfigNode::Vector2(v)
        }
        ConfigNode::Vector3(mut v) => {
            v.metadata = Some(b);
            ConfigNode::Vector3(v)
        }
        ConfigNode::Table(mut v) => {
            v.metadata = Some(b);
            ConfigNode::Table(v)
        }
        ConfigNode::DynamicTable(mut v) => {
            v.metadata = Some(b);
            ConfigNode::DynamicTable(v)
        }
        ConfigNode::Function(mut v) => {
            v.metadata = Some(b);
            ConfigNode::Function(v)
        }
        ConfigNode::Expression(mut v) => {
            v.metadata = Some(b);
            ConfigNode::Expression(v)
        }
        ConfigNode::Nil(mut v) => {
            v.metadata = Some(b);
            ConfigNode::Nil(v)
        }
        ConfigNode::Array(mut v) => {
            v.metadata = Some(b);
            ConfigNode::Array(v)
        }
    }
}

pub(crate) fn meta_is_empty(meta: &FieldMetadata) -> bool {
    meta.description.is_none()
        && meta.range.is_none()
        && meta.enum_options.is_none()
        && meta.map.is_none()
        && meta.nil_marker.is_none()
        && meta.function_info.is_none()
        && meta.table_schema.is_none()
}
