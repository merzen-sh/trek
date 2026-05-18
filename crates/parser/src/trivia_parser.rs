use crate::models::*;
use full_moon::ast::Expression;
use full_moon::tokenizer::{Token, TokenType};

#[derive(Debug, Clone)]
pub enum Annotation {
    KeyValue { key: String, value: String },
    Block { key: String, raw_table: String },
}

pub fn parse_trivia_token(token: &Token) -> Option<Annotation> {
    match token.token_type() {
        TokenType::SingleLineComment { comment } => {
            let text = comment.to_string();
            if let Some(rest) = text.strip_prefix('!') {
                let rest = rest.trim();
                if let Some(eq_pos) = rest.find('=') {
                    let key = rest[..eq_pos].trim().to_string();
                    let value = rest[eq_pos + 1..].trim().to_string();
                    return Some(Annotation::KeyValue { key, value });
                }
            }
            None
        }
        TokenType::MultiLineComment { comment, .. } => {
            let text = comment.to_string();
            let text = text.trim();
            if let Some(eq_pos) = text.find('=') {
                let key = text[..eq_pos].trim().to_string();
                let raw_table = text[eq_pos + 1..].trim().to_string();
                return Some(Annotation::Block { key, raw_table });
            }
            None
        }
        _ => None,
    }
}

pub fn annotation_to_metadata(ann: &Annotation) -> Result<FieldMetadata, String> {
    match ann {
        Annotation::KeyValue { key, value } => parse_key_value_meta(key, value),
        Annotation::Block { key, raw_table } => parse_block_meta(key, raw_table),
    }
}

fn parse_key_value_meta(key: &str, value: &str) -> Result<FieldMetadata, String> {
    let mut meta = FieldMetadata::default();
    match key {
        "ENUM" => {
            let options = parse_lua_string_array(value)?;
            meta.enum_options = Some(options);
        }
        "MAP" => {
            let val = value.trim().to_lowercase();
            meta.map = Some(val == "true");
        }
        "RANGE" => {
            let range = parse_lua_range(value)?;
            meta.range = Some(range);
        }
        _ => {}
    }
    Ok(meta)
}

fn parse_block_meta(key: &str, raw_table: &str) -> Result<FieldMetadata, String> {
    let mut meta = FieldMetadata::default();
    match key {
        "TABLE" | "ITEMS" => {
            let schema = parse_table_schema(raw_table)?;
            meta.table_schema = Some(schema);
        }
        "CFX_FUNCTION" => {
            let info = parse_function_info(raw_table)?;
            meta.function_info = Some(info);
        }
        _ => {}
    }
    Ok(meta)
}

fn parse_lua_string_array(text: &str) -> Result<Vec<String>, String> {
    let wrapped = format!("local _ = {}", text);
    let ast = parse_virtual(&wrapped)?;
    let table = extract_table_from_block(&ast)?;
    let mut result = Vec::new();
    for field in table.fields().iter() {
        if let Ok(val) = field_string_value(field) {
            result.push(val);
        }
    }
    Ok(result)
}

fn parse_lua_range(text: &str) -> Result<RangeValue, String> {
    let wrapped = format!("local _ = {}", text);
    let ast = parse_virtual(&wrapped)?;
    let table = extract_table_from_block(&ast)?;
    let mut min = 0.0;
    let mut max = 0.0;
    for field in table.fields().iter() {
        let name = field_key_name(field)?;
        let num = field_number_value(field)?;
        match name.as_str() {
            "min" => min = num,
            "max" => max = num,
            _ => {}
        }
    }
    Ok(RangeValue { min, max })
}

fn parse_table_schema(raw_table: &str) -> Result<TableSchema, String> {
    let wrapped = format!("local _ = {}", raw_table);
    let ast = parse_virtual(&wrapped)?;
    let table = extract_table_from_block(&ast)?;
    let mut layout = String::from("static");
    let mut schema = Vec::new();
    for field in table.fields().iter() {
        let name = field_key_name(field)?;
        match name.as_str() {
            "layout" => {
                if let Ok(val) = field_string_value(field) {
                    layout = val;
                }
            }
            "schema" => {
                let tc = field_table_value(field)?;
                for row in tc.fields().iter() {
                    let col = parse_column_schema(row)?;
                    schema.push(col);
                }
            }
            _ => {}
        }
    }
    Ok(TableSchema { layout, schema })
}

fn parse_column_schema(field: &full_moon::ast::Field) -> Result<ColumnSchema, String> {
    let tc = match field {
        full_moon::ast::Field::NoKey(Expression::TableConstructor(tc))
        | full_moon::ast::Field::ExpressionKey {
            value: Expression::TableConstructor(tc),
            ..
        }
        | full_moon::ast::Field::NameKey {
            value: Expression::TableConstructor(tc),
            ..
        } => tc,
        _ => return Err("expected table constructor for column".to_string()),
    };
    let mut name = String::new();
    let mut column_type = String::new();
    let mut is_key = None;
    let mut label = None;
    let mut description = None;
    for col_field in tc.fields().iter() {
        let field_name = field_key_name(col_field)?;
        match field_name.as_str() {
            "name" => name = field_string_value(col_field)?,
            "type" => column_type = field_string_value(col_field)?,
            "is_key" => {
                if let Some(b) = field_bool_value(col_field) {
                    is_key = Some(b);
                }
            }
            "label" => {
                if let Ok(v) = field_string_value(col_field) {
                    label = Some(v);
                }
            }
            "description" => {
                if let Ok(v) = field_string_value(col_field) {
                    description = Some(v);
                }
            }
            _ => {}
        }
    }
    Ok(ColumnSchema {
        name,
        column_type,
        is_key,
        label,
        description,
    })
}

fn parse_function_info(raw_table: &str) -> Result<FunctionInfo, String> {
    let wrapped = format!("local _ = {}", raw_table);
    let ast = parse_virtual(&wrapped)?;
    let table = extract_table_from_block(&ast)?;
    let mut resource_name = String::new();
    let mut function_name = String::new();
    for field in table.fields().iter() {
        let name = field_key_name(field)?;
        match name.as_str() {
            "resource_name" => resource_name = field_string_value(field)?,
            "function_name" => function_name = field_string_value(field)?,
            _ => {}
        }
    }
    Ok(FunctionInfo {
        resource_name,
        function_name,
    })
}

fn parse_virtual(code: &str) -> Result<full_moon::ast::Ast, String> {
    full_moon::parse(code).map_err(|errors| {
        let msgs: Vec<String> = errors
            .iter()
            .map(|e| e.error_message().to_string())
            .collect();
        format!("parse error: {}", msgs.join("; "))
    })
}

fn extract_table_from_block(
    ast: &full_moon::ast::Ast,
) -> Result<&full_moon::ast::TableConstructor, String> {
    for stmt in ast.nodes().stmts() {
        if let full_moon::ast::Stmt::LocalAssignment(la) = stmt {
            for expr in la.expressions().iter() {
                if let Expression::TableConstructor(tc) = expr {
                    return Ok(tc);
                }
            }
        }
    }
    Err("expected local assignment with table constructor".to_string())
}

fn field_key_name(field: &full_moon::ast::Field) -> Result<String, String> {
    match field {
        full_moon::ast::Field::NameKey { key, .. } => Ok(key.token().to_string()),
        full_moon::ast::Field::ExpressionKey { key, .. } => {
            if let Expression::String(token) = key {
                if let TokenType::StringLiteral { literal, .. } = token.token().token_type() {
                    return Ok(literal.to_string());
                }
            }
            Err("unsupported expression key".to_string())
        }
        full_moon::ast::Field::NoKey(_) => Err("field has no key".to_string()),
        _ => Err("unknown field type".to_string()),
    }
}

fn field_number_value(field: &full_moon::ast::Field) -> Result<f64, String> {
    let expr = field_value_expr(field)?;
    match expr {
        Expression::Number(token) => {
            let s = token.to_string();
            s.parse::<f64>().map_err(|e| format!("parse number: {e}"))
        }
        _ => Err("expected number".to_string()),
    }
}

fn field_string_value(field: &full_moon::ast::Field) -> Result<String, String> {
    let expr = field_value_expr(field)?;
    match expr {
        Expression::String(token) => {
            if let TokenType::StringLiteral { literal, .. } = token.token().token_type() {
                Ok(literal.to_string())
            } else {
                Ok(token.to_string().trim_matches('"').to_string())
            }
        }
        _ => Err("expected string".to_string()),
    }
}

fn field_bool_value(field: &full_moon::ast::Field) -> Option<bool> {
    let expr = field_value_expr(field).ok()?;
    match expr {
        Expression::Symbol(token) => match token.to_string().as_str() {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn field_table_value(
    field: &full_moon::ast::Field,
) -> Result<&full_moon::ast::TableConstructor, String> {
    let expr = field_value_expr(field)?;
    match expr {
        Expression::TableConstructor(tc) => Ok(tc),
        _ => Err("expected table constructor".to_string()),
    }
}

fn field_value_expr(field: &full_moon::ast::Field) -> Result<&Expression, String> {
    match field {
        full_moon::ast::Field::NameKey { value, .. }
        | full_moon::ast::Field::ExpressionKey { value, .. } => Ok(value),
        full_moon::ast::Field::NoKey(expr) => Ok(expr),
        _ => Err("unsupported field type".to_string()),
    }
}
