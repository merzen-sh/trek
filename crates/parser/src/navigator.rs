use full_moon::{
    ast::{
        Assignment, Block, Expression, Field, LocalAssignment, Stmt, TableConstructor,
        punctuated::{Pair, Punctuated},
    },
    ast::Ast,
    tokenizer::{TokenReference, TokenType},
};

use crate::ast_synth;
use crate::trivia_parser::extract_string_value;
use serde_json::Value;

/// Extract `(key, value)` from a table field, normalizing key tokens.
pub fn field_key_value<'a>(field: &'a Field) -> Option<(String, &'a Expression)> {
    match field {
        Field::NameKey { key, value, .. } => Some((normalize_key_token(key), value)),
        Field::ExpressionKey { key, value, .. } => {
            if let Expression::String(str_token) = key {
                let raw = str_token.token().to_string();
                let clean = raw.trim_matches(|c| c == '"' || c == '\'').to_string();
                Some((clean, value))
            } else {
                None
            }
        }
        Field::NoKey(_) => None,
        _ => None,
    }
}

fn normalize_key_token(key: &TokenReference) -> String {
    key.token().to_string().trim().to_string()
}

/// Locate the root `config = { ... }` table inside an AST.
pub fn find_root_table<'a>(ast: &'a Ast) -> Option<&'a TableConstructor> {
    for stmt in ast.nodes().stmts() {
        if let Some(table) = table_from_stmt(stmt) {
            return Some(table);
        }
    }
    if let Some(full_moon::ast::LastStmt::Return(r)) = ast.nodes().last_stmt() {
        for expr in r.returns().iter() {
            if let Expression::TableConstructor(table) = expr {
                return Some(table);
            }
        }
    }
    None
}

fn table_from_stmt(stmt: &Stmt) -> Option<&TableConstructor> {
    match stmt {
        Stmt::Assignment(a) => first_table_in_exprs(a.expressions()),
        Stmt::LocalAssignment(a) => first_table_in_exprs(a.expressions()),
        _ => None,
    }
}

fn first_table_in_exprs(exprs: &Punctuated<Expression>) -> Option<&TableConstructor> {
    exprs.iter().find_map(|expr| {
        if let Expression::TableConstructor(table) = expr {
            Some(table)
        } else {
            None
        }
    })
}

/// Walk `path` segments into a nested table constructor.
pub fn table_at_path<'a>(
    root: &'a TableConstructor,
    path: &[String],
) -> Result<&'a TableConstructor, String> {
    let mut current = root;
    for segment in path {
        let mut found = false;
        for field in current.fields().iter() {
            if let Some((key, Expression::TableConstructor(next))) = field_key_value(field) {
                if &key == segment {
                    current = next;
                    found = true;
                    break;
                }
            }
        }
        if !found {
            return Err(format!("table key not found: {segment}"));
        }
    }
    Ok(current)
}

/// Resolve the expression at `path` (path includes the field key as its last segment).
pub fn expression_at_path<'a>(
    root: &'a TableConstructor,
    path: &[String],
) -> Result<&'a Expression, String> {
    if path.is_empty() {
        return Err("empty ast_path".into());
    }
    let (parent_path, key) = path.split_at(path.len() - 1);
    let table = if parent_path.is_empty() {
        root
    } else {
        table_at_path(root, parent_path)?
    };
    for field in table.fields().iter() {
        if let Some((k, value)) = field_key_value(field) {
            if k == key[0] {
                return Ok(value);
            }
        }
    }
    Err(format!("field not found: {}", key[0]))
}

/// Read runtime data at `path` as JSON.
pub fn value_at_path(root: &TableConstructor, path: &[String]) -> Result<Value, String> {
    if path.is_empty() {
        return Err("empty ast_path".into());
    }
    let expr = expression_at_path(root, path)?;
    value_from_expression(expr)
}

fn value_from_expression(expr: &Expression) -> Result<Value, String> {
    match expr {
        Expression::TableConstructor(table) => table_to_json_map(table),
        Expression::Number(_) | Expression::String(_) | Expression::Symbol(_) => {
            Ok(Value::String(extract_string_value(expr)))
        }
        Expression::FunctionCall(_) | Expression::Var(_) => {
            if let Some((vec_type, nums)) = crate::visitor::try_parse_vector(expr) {
                let obj = match vec_type {
                    "vector2" => serde_json::json!({ "x": nums[0], "y": nums[1] }),
                    "vector3" => serde_json::json!({
                        "x": nums[0],
                        "y": nums[1],
                        "z": nums[2]
                    }),
                    _ => unreachable!(),
                };
                Ok(obj)
            } else {
                Ok(Value::Null)
            }
        }
        _ => Ok(Value::String(extract_string_value(expr))),
    }
}

fn table_to_json_map(table: &TableConstructor) -> Result<Value, String> {
    let mut map = serde_json::Map::new();
    for field in table.fields().iter() {
        if let Some((key, value)) = field_key_value(field) {
            map.insert(key, value_from_expression(value)?);
        }
    }
    Ok(Value::Object(map))
}

/// Apply a scalar patch (replace the value expression, preserve key trivia).
pub fn patch_scalar_at_path(
    ast: Ast,
    path: &[String],
    new_value: Value,
) -> Result<Ast, String> {
    if path.is_empty() {
        return Err("empty ast_path".into());
    }
    let new_expr = ast_synth::expression_from_json(&new_value)?;
    map_root_table(ast, |root| replace_field_value(root, path, new_expr.clone()))
}

/// Remove a row (`NameKey` / `ExpressionKey`) from the table at `path`.
pub fn patch_table_remove_row_at_path(
    ast: Ast,
    table_path: &[String],
    row_key: &str,
) -> Result<Ast, String> {
    map_root_table(ast, |root| remove_row_at_table_path(root, table_path, row_key))
}

fn remove_row_at_table_path(
    table: TableConstructor,
    path: &[String],
    row_key: &str,
) -> Result<TableConstructor, String> {
    if path.is_empty() {
        return Ok(remove_field_from_table(table, row_key));
    }
    let segment = &path[0];
    let rest = &path[1..];
    let mut found = false;
    let fields: Vec<Pair<Field>> = table
        .fields()
        .pairs()
        .map(|pair| {
            let mut p = pair.clone();
            match p.value_mut() {
                Field::NameKey { key, value, .. } => {
                    if normalize_key_token(key) == *segment {
                        if let Expression::TableConstructor(inner) = value {
                            *value = Expression::TableConstructor(remove_row_at_table_path(
                                inner.clone(),
                                rest,
                                row_key,
                            )?);
                            found = true;
                        } else {
                            return Err(format!("key is not a table: {segment}"));
                        }
                    }
                }
                Field::ExpressionKey { key: ek, value, .. } => {
                    if let Expression::String(str_token) = ek {
                        let raw = str_token.token().to_string();
                        let clean = raw.trim_matches(|c| c == '"' || c == '\'');
                        if clean == segment {
                            if let Expression::TableConstructor(inner) = value {
                                *value = Expression::TableConstructor(remove_row_at_table_path(
                                    inner.clone(),
                                    rest,
                                    row_key,
                                )?);
                                found = true;
                            } else {
                                return Err(format!("key is not a table: {segment}"));
                            }
                        }
                    }
                }
                _ => {}
            }
            Ok(p)
        })
        .collect::<Result<_, _>>()?;
    if !found {
        return Err(format!("table key not found: {segment}"));
    }
    Ok(table.with_fields(Punctuated::from_iter(fields)))
}

fn remove_field_from_table(table: TableConstructor, key: &str) -> TableConstructor {
    let mut fields: Vec<Pair<Field>> = table
        .fields()
        .pairs()
        .filter(|pair| !field_matches_key(pair.value(), key))
        .cloned()
        .collect();
    if let Some(last) = fields.last_mut() {
        if last.punctuation().is_some() {
            *last = Pair::new(last.clone().into_value(), None);
        }
    }
    table.with_fields(Punctuated::from_iter(fields))
}

fn field_matches_key(field: &Field, key: &str) -> bool {
    match field {
        Field::NameKey { key: field_key, .. } => normalize_key_token(field_key) == key,
        Field::ExpressionKey { key: ek, .. } => {
            if let Expression::String(str_token) = ek {
                let raw = str_token.token().to_string();
                let clean = raw.trim_matches(|c| c == '"' || c == '\'');
                clean == key
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Append a new `NameKey` row to the table at `path` (empty path = root config table).
pub fn patch_table_row_at_path(
    ast: Ast,
    path: &[String],
    row_key: &str,
    row_payload: &Value,
) -> Result<Ast, String> {
    let new_field = ast_synth::table_row_field(row_key, row_payload)?;
    map_root_table(ast, |root| append_field_at_table_path(root, path, new_field.clone()))
}

fn replace_field_value(
    table: TableConstructor,
    path: &[String],
    new_expr: Expression,
) -> Result<TableConstructor, String> {
    if path.is_empty() {
        return Err("empty ast_path".into());
    }
    let (parent_path, key) = path.split_at(path.len() - 1);
    let key = &key[0];

    if parent_path.is_empty() {
        return Ok(replace_in_table(table, key, new_expr));
    }

    let mut updated = table;
    updated = patch_nested_table(updated, parent_path, key, new_expr)?;
    Ok(updated)
}

fn patch_nested_table(
    table: TableConstructor,
    parent_path: &[String],
    key: &str,
    new_expr: Expression,
) -> Result<TableConstructor, String> {
    if parent_path.is_empty() {
        return Ok(replace_in_table(table, key, new_expr));
    }
    let segment = &parent_path[0];
    let rest = &parent_path[1..];
    let mut fields: Vec<Pair<Field>> = table.fields().pairs().cloned().collect();
    let mut found = false;
    for pair in fields.iter_mut() {
        let field = pair.value_mut();
        if let Field::NameKey { key: field_key, value, .. } = field {
            if normalize_key_token(field_key) == *segment {
                if let Expression::TableConstructor(inner) = value {
                    *value = Expression::TableConstructor(patch_nested_table(
                        inner.clone(),
                        rest,
                        key,
                        new_expr.clone(),
                    )?);
                    found = true;
                    break;
                }
            }
        }
        if let Field::ExpressionKey { key: ek, value, .. } = field {
            if let Expression::String(str_token) = ek {
                let raw = str_token.token().to_string();
                let clean = raw.trim_matches(|c| c == '"' || c == '\'');
                if clean == segment {
                    if let Expression::TableConstructor(inner) = value {
                        *value = Expression::TableConstructor(patch_nested_table(
                            inner.clone(),
                            rest,
                            key,
                            new_expr.clone(),
                        )?);
                        found = true;
                        break;
                    }
                }
            }
        }
    }
    if !found {
        return Err(format!("table key not found: {segment}"));
    }
    Ok(table.with_fields(Punctuated::from_iter(fields)))
}

fn replace_in_table(
    table: TableConstructor,
    key: &str,
    new_expr: Expression,
) -> TableConstructor {
    let fields: Vec<Pair<Field>> = table
        .fields()
        .pairs()
        .map(|pair| {
            let mut p = pair.clone();
            match p.value_mut() {
                Field::NameKey {
                    key: field_key,
                    value,
                    equal: _,
                } => {
                    if normalize_key_token(field_key) == key {
                        *value = new_expr.clone();
                    }
                }
                Field::ExpressionKey {
                    key: ek,
                    value,
                    ..
                } => {
                    if let Expression::String(str_token) = ek {
                        let raw = str_token.token().to_string();
                        let clean = raw.trim_matches(|c| c == '"' || c == '\'');
                        if clean == key {
                            *value = new_expr.clone();
                        }
                    }
                }
                _ => {}
            }
            p
        })
        .collect();
    table.with_fields(Punctuated::from_iter(fields))
}

fn append_field_at_table_path(
    table: TableConstructor,
    path: &[String],
    new_field: Field,
) -> Result<TableConstructor, String> {
    if path.is_empty() {
        return Ok(append_field_to_table(table, new_field));
    }
    let segment = &path[0];
    let rest = &path[1..];
    let fields: Vec<Pair<Field>> = table
        .fields()
        .pairs()
        .map(|pair| {
            let mut p = pair.clone();
            let field = p.value_mut();
            match field {
                Field::NameKey { key, value, .. } => {
                    if normalize_key_token(key) == *segment {
                        if let Expression::TableConstructor(inner) = value {
                            *value = Expression::TableConstructor(append_field_at_table_path(
                                inner.clone(),
                                rest,
                                new_field.clone(),
                            )?);
                        } else {
                            return Err(format!("key is not a table: {segment}"));
                        }
                    }
                }
                Field::ExpressionKey { key: ek, value, .. } => {
                    if let Expression::String(str_token) = ek {
                        let raw = str_token.token().to_string();
                        let clean = raw.trim_matches(|c| c == '"' || c == '\'');
                        if clean == segment {
                            if let Expression::TableConstructor(inner) = value {
                                *value = Expression::TableConstructor(append_field_at_table_path(
                                    inner.clone(),
                                    rest,
                                    new_field.clone(),
                                )?);
                            } else {
                                return Err(format!("key is not a table: {segment}"));
                            }
                        }
                    }
                }
                _ => {}
            }
            Ok(p)
        })
        .collect::<Result<_, _>>()?;
    Ok(table.with_fields(Punctuated::from_iter(fields)))
}

pub fn append_field_to_table(table: TableConstructor, mut new_field: Field) -> TableConstructor {
    let indent = detect_existing_indent(&table);
    if let Field::NameKey { key, .. } = &mut new_field {
        *key = ast_synth::key_token_with_indent(
            &normalize_key_token(key),
            &indent,
        );
    }
    let mut fields: Vec<Pair<Field>> = table.fields().pairs().cloned().collect();
    if let Some(last) = fields.last_mut() {
        if last.punctuation().is_none() {
            let punct = ast_synth::comma_token();
            *last = Pair::Punctuated(last.clone().into_value(), punct);
        }
    }
    fields.push(Pair::new(new_field, None));
    table.with_fields(Punctuated::from_iter(fields))
}

pub fn map_root_table(
    ast: Ast,
    f: impl Fn(TableConstructor) -> Result<TableConstructor, String>,
) -> Result<Ast, String> {
    let block = ast.nodes().clone();
    let new_block = map_block_root_table(block, &f)?;
    Ok(ast.with_nodes(new_block))
}

fn map_block_root_table(
    block: Block,
    f: &impl Fn(TableConstructor) -> Result<TableConstructor, String>,
) -> Result<Block, String> {
    let stmts: Vec<(Stmt, Option<TokenReference>)> = block
        .stmts_with_semicolon()
        .map(|(stmt, semi)| map_stmt_root_table(stmt.clone(), &f).map(|s| (s, semi.clone())))
        .collect::<Result<_, _>>()?;
    Ok(block.with_stmts(stmts))
}

fn map_stmt_root_table(
    stmt: Stmt,
    f: &impl Fn(TableConstructor) -> Result<TableConstructor, String>,
) -> Result<Stmt, String> {
    match stmt {
        Stmt::Assignment(a) => Ok(Stmt::Assignment(map_assignment_table(a, f)?)),
        Stmt::LocalAssignment(a) => Ok(Stmt::LocalAssignment(map_local_assignment_table(a, f)?)),
        other => Ok(other),
    }
}

fn map_assignment_table(
    assignment: Assignment,
    f: &impl Fn(TableConstructor) -> Result<TableConstructor, String>,
) -> Result<Assignment, String> {
    let exprs: Vec<Pair<Expression>> = assignment
        .expressions()
        .pairs()
        .map(|pair| -> Result<Pair<Expression>, String> {
            let mut p = pair.clone();
            if let Expression::TableConstructor(table) = p.value_mut() {
                *p.value_mut() = Expression::TableConstructor(f(table.clone())?);
            }
            Ok(p)
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(assignment.with_expressions(Punctuated::from_iter(exprs)))
}

fn map_local_assignment_table(
    assignment: LocalAssignment,
    f: &impl Fn(TableConstructor) -> Result<TableConstructor, String>,
) -> Result<LocalAssignment, String> {
    let exprs: Vec<Pair<Expression>> = assignment
        .expressions()
        .pairs()
        .map(|pair| -> Result<Pair<Expression>, String> {
            let mut p = pair.clone();
            if let Expression::TableConstructor(table) = p.value_mut() {
                *p.value_mut() = Expression::TableConstructor(f(table.clone())?);
            }
            Ok(p)
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(assignment.with_expressions(Punctuated::from_iter(exprs)))
}

/// Detect indentation used by existing rows in a table (for lossless injection).
pub fn detect_existing_indent(table: &TableConstructor) -> String {
    if let Some(first) = table.fields().iter().next() {
        if let Field::NameKey { key, .. } = first {
            for trivia in key.leading_trivia() {
                if let TokenType::Whitespace { characters } = trivia.token_type() {
                    let s = characters.to_string();
                    if let Some(last_nl) = s.rfind('\n') {
                        return s[last_nl + 1..].to_string();
                    }
                    return s;
                }
            }
        }
    }
    "    ".to_string()
}
