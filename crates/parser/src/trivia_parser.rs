use full_moon::{
    ast::{Expression, Field, Stmt, Var},
    parse,
    tokenizer::{TokenReference, TokenType},
};

use crate::models::{ArgDef, CfxFunctionMeta, ColumnDef, ColumnType, TableSchema};
use indexmap::IndexMap;

// ---------------------------------------------------------------------------
// Public annotation bag
// ---------------------------------------------------------------------------

/// All annotations collected from a single token's leading trivia.
#[derive(Default)]
pub struct Annotations {
    pub description:  Vec<String>,
    pub enum_options: Option<Vec<String>>,
    pub range:        Option<Vec<String>>,
    pub table_schema: Option<TableSchema>,
    pub cfx_function: Option<CfxFunctionMeta>,
}

impl Annotations {
    /// Move `description` out; returns `None` when empty.
    #[inline]
    pub fn take_description(&mut self) -> Option<Vec<String>> {
        if self.description.is_empty() { None } else { Some(std::mem::take(&mut self.description)) }
    }
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Collect every annotation from the leading trivia of `token_ref`.
///
/// | Syntax                    | Populates            |
/// |---------------------------|----------------------|
/// | `--! text`                | `description`        |
/// | `--@ENUM = { "a", "b" }`  | `enum_options`       |
/// | `--@RANGE = { 0, 100 }`   | `range`              |
/// | `--[[@TABLE … --]]`       | `table_schema`       |
/// | `--[[@CFX_FUNCTION … --]]`| `cfx_function`       |
/// | `--[[ !text … --]]`       | `description`        |
/// | `--[[ @ENUM … --]]`       | `enum_options`       |
/// | `--[[ @RANGE … --]]`      | `range`              |
pub fn parse_annotations(token_ref: &TokenReference) -> Annotations {
    let mut ann = Annotations::default();
    let trivia: Vec<_> = token_ref.leading_trivia().collect();

    for token in trivia.iter().rev() {
        match token.token_type() {
            TokenType::SingleLineComment { comment } => {
                if let Some(text) = comment.strip_prefix('!') {
                    ann.description.push(text.trim().to_string());
                } else if let Some(text) = comment.strip_prefix('@') {
                    if text.starts_with("ENUM") {
                        ann.enum_options = parse_table_values(text);
                    } else if text.starts_with("RANGE") {
                        ann.range = parse_table_values(text);
                    } else if text.starts_with("CFX_FUNCTION") {
                        ann.cfx_function = parse_cfx_function(text);
                    }
                }
            }
            TokenType::MultiLineComment { comment, .. } => {
                // full_moon emits `--[[` content as MultiLineComment.
                // Do NOT break here — the `--]]` closer appears as a separate
                // token *before* the opener when iterating in reverse, and we
                // must keep scanning past it to find preceding `--!` lines.
                let body = comment.trim_start();
                // Match @table / @TABLE / @CFX_FUNCTION (case-insensitive)
                if body.to_ascii_uppercase().starts_with("@TABLE") {
                    ann.table_schema = parse_table_schema(&body[1..]);
                } else if body.to_ascii_uppercase().starts_with("@CFX_FUNCTION") {
                    ann.cfx_function = parse_cfx_function(&body[1..]);
                } else {
                    // Parse each line for:
                    //   !text    → description
                    //   @ENUM…   → enum options
                    //   @RANGE…  → range constraint
                    let mut block_descs = Vec::new();
                    for line in body.lines() {
                        let line = line.trim();
                        if let Some(text) = line.strip_prefix('!') {
                            block_descs.push(text.trim().to_string());
                        } else if let Some(text) = line.strip_prefix('@') {
                            if text.starts_with("ENUM") {
                                ann.enum_options = parse_table_values(text);
                            } else if text.starts_with("RANGE") {
                                ann.range = parse_table_values(text);
                            }
                        }
                    }
                    // Lines within the block are in forward order, but the
                    // whole token is visited during reverse trivia iteration.
                    // Push in reverse so the final .reverse() restores order.
                    for d in block_descs.into_iter().rev() {
                        ann.description.push(d);
                    }
                }
            }
            TokenType::Whitespace { .. } => continue,
            // Only truly foreign tokens (identifiers, operators, etc.)
            // terminate the annotation window.
            _ => break,
        }
    }

    ann.description.reverse();
    ann
}

// ---------------------------------------------------------------------------
// `@TABLE` schema parser
// ---------------------------------------------------------------------------

fn parse_table_schema(body: &str) -> Option<TableSchema> {
    // `--[[@TABLE … --]]` leaves a trailing "--" in `body`; strip it.
    let body = body.trim_end_matches('-').trim_end();
    let src = format!("local {body}");
    let ast = parse(src.trim()).ok()?;
    let table = first_table_constructor(&ast)?;

    let mut schema = TableSchema::default();
    for field in table.fields().iter() {
        let Field::NameKey { key, value, .. } = field else { continue };
        match key.token().to_string().as_str() {
            "allow_add"    => schema.allow_add    = is_true(value),
            "allow_delete" => schema.allow_delete = is_true(value),
            "allow_edit"   => schema.allow_edit   = is_true(value),
            "schema" | "schemas" => schema.columns = parse_columns(value),
            _ => {}
        }
    }
    Some(schema)
}

fn parse_columns(expr: &Expression) -> Vec<ColumnDef> {
    let Expression::TableConstructor(outer) = expr else { return Vec::new() };
    outer
        .fields()
        .iter()
        .filter_map(|f| {
            let Field::NoKey(inner_expr) = f else { return None };
            let Expression::TableConstructor(inner) = inner_expr else { return None };
            parse_column_def(inner)
        })
        .collect()
}

fn parse_column_def(table: &full_moon::ast::TableConstructor) -> Option<ColumnDef> {
    let mut field    = None::<String>;
    let mut col_type = None::<String>;
    let mut label    = None::<String>;
    let mut values   = Vec::<String>::new();

    for f in table.fields().iter() {
        let Field::NameKey { key, value, .. } = f else { continue };
        match key.token().to_string().as_str() {
            "field" | "name"  => field    = Some(extract_string_value(value)),
            "type"            => col_type = Some(extract_string_value(value)),
            "label"           => label    = Some(extract_string_value(value)),
            "values"          => values   = collect_string_list(value),
            _ => {}
        }
    }

    Some(ColumnDef {
        field:    field?,
        col_type: ColumnType::from(col_type.as_deref().unwrap_or("")),
        label:    label.unwrap_or_default(),
        values,
    })
}

/// Meta block (`KEY = value` in a plain `--[[ … ]]` comment)
pub fn parse_lua_key_values(source: &str) -> IndexMap<String, serde_json::Value> {
    let Ok(ast) = parse(source) else { return IndexMap::new() };

    ast.nodes()
        .stmts()
        .filter_map(|stmt| {
            let Stmt::Assignment(assign) = stmt else { return None };
            Some(
                assign
                    .variables()
                    .iter()
                    .zip(assign.expressions().iter())
                    .filter_map(|(var, expr)| {
                        let Var::Name(token) = var else { return None };
                        Some((
                            token.token().to_string(),
                            serde_json::Value::String(extract_string_value(expr)),
                        ))
                    }),
            )
        })
        .flatten()
        .collect()
}

/// Extract scalar string representation from a Lua expression leaf.
#[inline]
pub fn extract_string_value(expr: &Expression) -> String {
    match expr {
        Expression::String(token) => {
            if let TokenType::StringLiteral { literal, .. } = token.token().token_type() {
                literal.to_string()
            } else {
                token.to_string()
            }
        }
        Expression::Number(token) => {
            if let TokenType::Number { text } = token.token().token_type() {
                text.to_string()
            } else {
                token.to_string()
            }
        }
        Expression::Symbol(token) => token.token().to_string(),
        other => other.to_string(),
    }
}

fn parse_table_values(ann: &str) -> Option<Vec<String>> {
    let src = format!("local {ann}");
    let ast = parse(src.trim()).ok()?;
    ast.nodes().stmts().find_map(|stmt| {
        let Stmt::LocalAssignment(local) = stmt else { return None };
        local.expressions().iter().find_map(|expr| {
            let Expression::TableConstructor(table) = expr else { return None };
            let v: Vec<String> = table
                .fields()
                .iter()
                .filter_map(|f| if let Field::NoKey(e) = f { Some(extract_string_value(e)) } else { None })
                .collect();
            if v.is_empty() { None } else { Some(v) }
        })
    })
}

fn collect_string_list(expr: &Expression) -> Vec<String> {
    let Expression::TableConstructor(table) = expr else { return Vec::new() };
    table
        .fields()
        .iter()
        .filter_map(|f| if let Field::NoKey(e) = f { Some(extract_string_value(e)) } else { None })
        .collect()
}

#[inline]
fn is_true(expr: &Expression) -> bool {
    matches!(expr, Expression::Symbol(t) if t.token().to_string() == "true")
}

// ---------------------------------------------------------------------------
// `@CFX_FUNCTION` parser
// ---------------------------------------------------------------------------

fn parse_cfx_function(ann: &str) -> Option<CfxFunctionMeta> {
    let src = format!("local {ann}");
    let ast = parse(src.trim()).ok()?;
    let table = first_table_constructor(&ast)?;

    let mut meta = CfxFunctionMeta::default();
    for field in table.fields().iter() {
        let Field::NameKey { key, value, .. } = field else { continue };
        if key.token().to_string() == "args_schema" {
            meta.args_schema = parse_arg_defs(value);
        }
    }
    Some(meta)
}

fn parse_arg_defs(expr: &Expression) -> Vec<ArgDef> {
    let Expression::TableConstructor(outer) = expr else { return Vec::new() };
    outer
        .fields()
        .iter()
        .filter_map(|f| {
            let Field::NoKey(inner_expr) = f else { return None };
            let Expression::TableConstructor(inner) = inner_expr else { return None };
            parse_arg_def(inner)
        })
        .collect()
}

fn parse_arg_def(table: &full_moon::ast::TableConstructor) -> Option<ArgDef> {
    let mut name     = None::<String>;
    let mut arg_type = None::<String>;
    let mut label    = None::<String>;
    let mut required = None::<bool>;

    for f in table.fields().iter() {
        let Field::NameKey { key, value, .. } = f else { continue };
        match key.token().to_string().as_str() {
            "name"     => name     = Some(extract_string_value(value)),
            "type"     => arg_type = Some(extract_string_value(value)),
            "label"    => label    = Some(extract_string_value(value)),
            "required" => required = Some(is_true(value)),
            _ => {}
        }
    }

    Some(ArgDef {
        name: name?,
        arg_type: ColumnType::from(arg_type.as_deref().unwrap_or("")),
        label: label.unwrap_or_default(),
        required: required.unwrap_or(false),
    })
}

fn first_table_constructor(ast: &full_moon::ast::Ast) -> Option<&full_moon::ast::TableConstructor> {
    ast.nodes().stmts().find_map(|stmt| {
        let Stmt::LocalAssignment(local) = stmt else { return None };
        local.expressions().iter().find_map(|expr| {
            if let Expression::TableConstructor(t) = expr { Some(t) } else { None }
        })
    })
}
