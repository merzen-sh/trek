use crate::models::*;
use crate::trivia_parser::{self, Annotation};
use crate::visitor;
use full_moon::ast::{Expression, Field};
use full_moon::node::Node;
use full_moon::tokenizer::{Token, TokenType};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Diagnostic {
    pub severity: String, // "Error" | "Warning"
    pub line: usize,
    pub character: usize,
    pub message: String,
    pub source: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LintResult {
    pub success: bool,
    pub diagnostics: Vec<Diagnostic>,
    pub data: Option<serde_json::Value>,
}

pub fn lint(source: &str) -> String {
    let result = run_lint(source);
    serde_json::to_string_pretty(&result).unwrap_or_else(|e| {
        format!(
            "{{\"success\": false, \"diagnostics\": [{{\"severity\": \"Error\", \"line\": 1, \"character\": 1, \"message\": \"Failed to serialize lint result: {}\", \"source\": \"configir-linter\"}}], \"data\": null}}",
            e
        )
    })
}

fn run_lint(source: &str) -> LintResult {
    let mut diagnostics = Vec::new();

    // --- Category A: Syntax Validation (Lua / Parser Level) ---
    let ast = match full_moon::parse(source) {
        Ok(ast) => ast,
        Err(errors) => {
            for err in errors {
                let msg = err.to_string();
                let (line, col) = match &err {
                    full_moon::Error::AstError(ast_err) => {
                        let start = ast_err.token().start_position();
                        (start.line(), start.character())
                    }
                    full_moon::Error::TokenizerError(tok_err) => {
                        let start = tok_err.range().0;
                        (start.line(), start.character())
                    }
                };
                diagnostics.push(Diagnostic {
                    severity: "Error".to_string(),
                    line,
                    character: col,
                    message: format!("Syntax Error: {msg}"),
                    source: "configir-linter".to_string(),
                });
            }
            return LintResult {
                success: false,
                diagnostics,
                data: None,
            };
        }
    };

    // Extract all annotations in the document to validate syntax and semantics
    let mut annotations = Vec::new();
    for token in ast.tokens() {
        if let Some(ann) = trivia_parser::parse_trivia_token(token.token()) {
            annotations.push((token.clone(), ann));
        }
    }

    // --- Category A & B: Annotation-level Syntax and Semantic Checks ---
    for (token, ann) in &annotations {
        let start = token.start_position().unwrap();
        let token_line = start.line();
        let token_col = start.character();

        match ann {
            Annotation::KeyValue { key, value } => {
                // Category A: Syntax Error Check
                let wrapped = format!("local _ = {}", value);
                if let Err(errors) = full_moon::parse(&wrapped) {
                    let first_err = errors.iter().next();
                    let msg = if let Some(err) = first_err {
                        let err_msg = err.to_string();
                        if err_msg.contains("close") || err_msg.contains("expected '}'") {
                            "Syntax Error: Unclosed table constructor in annotation expression."
                                .to_string()
                        } else {
                            format!("Syntax Error: {err_msg} in annotation expression.")
                        }
                    } else {
                        "Syntax Error in annotation expression.".to_string()
                    };

                    diagnostics.push(Diagnostic {
                        severity: "Error".to_string(),
                        line: token_line,
                        character: token_col,
                        message: msg,
                        source: "configir-linter".to_string(),
                    });
                    continue;
                }

                // Category B: Semantic Rules for KeyValue
                match key.as_str() {
                    "ENUM" => {
                        if let Err(e) = validate_enum_semantic(value) {
                            diagnostics.push(Diagnostic {
                                severity: "Error".to_string(),
                                line: token_line,
                                character: token_col,
                                message: format!("Semantic Error: {e}"),
                                source: "configir-linter".to_string(),
                            });
                        }
                    }
                    "RANGE" => {
                        if let Err(e) = validate_range_semantic(value) {
                            diagnostics.push(Diagnostic {
                                severity: "Error".to_string(),
                                line: token_line,
                                character: token_col,
                                message: format!("Semantic Error: {e}"),
                                source: "configir-linter".to_string(),
                            });
                        }
                    }
                    "MAP" => {
                        let val = value.trim();
                        if val != "true" && val != "false" {
                            diagnostics.push(Diagnostic {
                                severity: "Error".to_string(),
                                line: token_line,
                                character: token_col,
                                message: "Semantic Error: MAP annotation must be a boolean literal (true or false).".to_string(),
                                source: "configir-linter".to_string(),
                            });
                        }
                    }
                    other => {
                        diagnostics.push(Diagnostic {
                            severity: "Error".to_string(),
                            line: token_line,
                            character: token_col,
                            message: format!(
                                "Semantic Error: Unrecognized annotation keyword '{other}'."
                            ),
                            source: "configir-linter".to_string(),
                        });
                    }
                }
            }
            Annotation::Block { key, raw_table } => {
                // Category A: Syntax Error Check
                let wrapped = format!("local _ = {}", raw_table);
                if let Err(errors) = full_moon::parse(&wrapped) {
                    let first_err = errors.iter().next();
                    let msg = if let Some(err) = first_err {
                        let err_msg = err.to_string();
                        if err_msg.contains("close") || err_msg.contains("expected '}'") {
                            "Syntax Error: Unclosed table constructor in annotation expression."
                                .to_string()
                        } else {
                            format!("Syntax Error: {err_msg} in annotation expression.")
                        }
                    } else {
                        "Syntax Error in annotation expression.".to_string()
                    };

                    diagnostics.push(Diagnostic {
                        severity: "Error".to_string(),
                        line: token_line,
                        character: token_col,
                        message: msg,
                        source: "configir-linter".to_string(),
                    });
                    continue;
                }

                // Category B: Semantic Rules for Block
                match key.as_str() {
                    "TABLE" | "ITEMS" => {
                        if let Err(e) = validate_table_schema_semantic(raw_table) {
                            diagnostics.push(Diagnostic {
                                severity: "Error".to_string(),
                                line: token_line,
                                character: token_col,
                                message: format!("Semantic Error: {e}"),
                                source: "configir-linter".to_string(),
                            });
                        }
                    }
                    "CFX_FUNCTION" => {
                        if let Err(e) = validate_cfx_function_semantic(raw_table) {
                            diagnostics.push(Diagnostic {
                                severity: "Error".to_string(),
                                line: token_line,
                                character: token_col,
                                message: format!("Semantic Error: {e}"),
                                source: "configir-linter".to_string(),
                            });
                        }
                    }
                    other => {
                        diagnostics.push(Diagnostic {
                            severity: "Error".to_string(),
                            line: token_line,
                            character: token_col,
                            message: format!(
                                "Semantic Error: Unrecognized annotation keyword '{other}'."
                            ),
                            source: "configir-linter".to_string(),
                        });
                    }
                }
            }
        }
    }

    // If there were Category A / B semantic errors in the annotations, we can still run category C.
    // Let's build the IR first, to get all fields and their attached metadata.
    let ir = match visitor::build_ir(source) {
        Ok(ir) => ir,
        Err(_) => {
            // If visitor fails but we didn't add any diagnostics, let's report a generic config error.
            if diagnostics.is_empty() {
                diagnostics.push(Diagnostic {
                    severity: "Error".to_string(),
                    line: 1,
                    character: 1,
                    message: "no config table assignment found".to_string(),
                    source: "configir-linter".to_string(),
                });
            }
            return LintResult {
                success: false,
                diagnostics,
                data: None,
            };
        }
    };

    // Find the config table constructor in the AST to perform structural checks (Category C)
    let mut config_tc = None;
    for stmt in ast.nodes().stmts() {
        if let full_moon::ast::Stmt::Assignment(assignment) = stmt {
            for (var, expr) in assignment
                .variables()
                .iter()
                .zip(assignment.expressions().iter())
            {
                if let full_moon::ast::Var::Name(ref_token) = var {
                    if ref_token.token().to_string() == "config" {
                        if let Expression::TableConstructor(tc) = expr {
                            config_tc = Some(tc);
                        }
                    }
                }
            }
        }
    }

    if let Some(tc) = config_tc {
        // Collect all valid annotations that are correctly attached to fields
        let mut attached_tokens = Vec::new();

        // Recursively validate table fields and match types (Category C)
        validate_fields_recursive(tc, &ir, &mut diagnostics, &mut attached_tokens);

        // Category C Dangling Annotation check
        // An annotation is dangling if it is NOT in `attached_tokens`, or if it is inside but failed immediate attachment checks.
        for (token, _ann) in &annotations {
            let start = token.start_position().unwrap();
            let token_line = start.line();
            let token_col = start.character();

            let is_attached = attached_tokens.iter().any(|t: &Token| {
                t.start_position().bytes() == token.start_position().unwrap().bytes()
            });

            if !is_attached {
                diagnostics.push(Diagnostic {
                    severity: "Error".to_string(),
                    line: token_line,
                    character: token_col,
                    message: "Structural Error: Dangling annotation. Annotations must be immediately followed by a configuration field or variable assignment.".to_string(),
                    source: "configir-linter".to_string(),
                });
            }
        }
    }

    // Sort diagnostics by line, then character
    diagnostics.sort_by(|a, b| a.line.cmp(&b.line).then(a.character.cmp(&b.character)));

    let success = diagnostics.iter().all(|d| d.severity != "Error");
    let data = if success {
        serde_json::to_value(&ir).ok()
    } else {
        None
    };

    LintResult {
        success,
        diagnostics,
        data,
    }
}

// --- Semantic Validation Helper Functions (Category B) ---

fn validate_enum_semantic(value: &str) -> Result<(), String> {
    let wrapped = format!("local _ = {}", value);
    let ast = full_moon::parse(&wrapped).map_err(|errors| {
        let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        msgs.join("; ")
    })?;
    let tc = extract_table_from_ast(&ast)?;
    if tc.fields().iter().next().is_none() {
        return Err(
            "ENUM annotation must be a non-empty array table containing strings.".to_string(),
        );
    }
    for field in tc.fields().iter() {
        match field {
            Field::NoKey(expr) => match expr {
                Expression::String(_) => {}
                _ => return Err("ENUM options must be string literals.".to_string()),
            },
            _ => return Err("ENUM annotation must be an array table constructor defining string options with no keys.".to_string()),
        }
    }
    Ok(())
}

fn validate_range_semantic(value: &str) -> Result<(), String> {
    let wrapped = format!("local _ = {}", value);
    let ast = full_moon::parse(&wrapped).map_err(|errors| {
        let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        msgs.join("; ")
    })?;
    let tc = extract_table_from_ast(&ast)?;

    let mut min = None;
    let mut max = None;

    for field in tc.fields().iter() {
        let key = field_key_name(field)?;
        let val = field_number_value(field)?;
        match key.as_str() {
            "min" => min = Some(val),
            "max" => max = Some(val),
            other => {
                return Err(format!(
                    "RANGE annotation contains unrecognized field '{other}'. Only 'min' and 'max' are allowed."
                ));
            }
        }
    }

    match (min, max) {
        (Some(min_v), Some(max_v)) => {
            if min_v > max_v {
                Err(format!(
                    "RANGE 'min' ({}) cannot be greater than 'max' ({}).",
                    min_v, max_v
                ))
            } else {
                Ok(())
            }
        }
        _ => {
            Err("RANGE annotation must explicitly define both 'min' and 'max' fields.".to_string())
        }
    }
}

fn validate_table_schema_semantic(value: &str) -> Result<(), String> {
    let wrapped = format!("local _ = {}", value);
    let ast = full_moon::parse(&wrapped).map_err(|errors| {
        let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        msgs.join("; ")
    })?;
    let tc = extract_table_from_ast(&ast)?;

    let mut layout = None;
    let mut schema = None;

    for field in tc.fields().iter() {
        let key = field_key_name(field)?;
        match key.as_str() {
            "layout" => {
                layout = Some(field_string_value(field)?);
            }
            "schema" => {
                schema = Some(field_table_value(field)?);
            }
            other => {
                return Err(format!(
                    "TABLE/ITEMS contains unrecognized field '{other}'. Only 'layout' and 'schema' are allowed."
                ));
            }
        }
    }

    if layout.is_none() {
        return Err("TABLE/ITEMS annotation must contain a 'layout' string.".to_string());
    }
    let schema_tc = match schema {
        Some(s) => s,
        None => return Err("TABLE/ITEMS annotation must contain a 'schema' array.".to_string()),
    };

    for row in schema_tc.fields().iter() {
        let col_tc = match row {
            Field::NoKey(Expression::TableConstructor(c)) => c,
            Field::NameKey {
                value: Expression::TableConstructor(c),
                ..
            } => c,
            Field::ExpressionKey {
                value: Expression::TableConstructor(c),
                ..
            } => c,
            _ => return Err("Schema column must be a table constructor.".to_string()),
        };

        let mut name = None;
        let mut column_type = None;

        for col_field in col_tc.fields().iter() {
            let col_key = field_key_name(col_field)?;
            match col_key.as_str() {
                "name" => name = Some(field_string_value(col_field)?),
                "type" => column_type = Some(field_string_value(col_field)?),
                "is_key" | "label" | "description" => {}
                other => {
                    return Err(format!(
                        "Schema column contains unrecognized field '{other}'."
                    ));
                }
            }
        }

        match (name, column_type) {
            (Some(_), Some(t)) => {
                let valid_types = ["string", "number", "boolean", "vector2", "vector3"];
                if !valid_types.contains(&t.as_str()) {
                    return Err(format!(
                        "Schema column has unrecognized type '{t}'. Only 'string', 'number', 'boolean', 'vector2', 'vector3' are allowed."
                    ));
                }
            }
            _ => {
                return Err(
                    "Schema column must contain a 'name' and a primitive 'type'.".to_string(),
                );
            }
        }
    }

    Ok(())
}

fn validate_cfx_function_semantic(value: &str) -> Result<(), String> {
    let wrapped = format!("local _ = {}", value);
    let ast = full_moon::parse(&wrapped).map_err(|errors| {
        let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        msgs.join("; ")
    })?;
    let tc = extract_table_from_ast(&ast)?;

    let mut resource_name = None;
    let mut function_name = None;

    for field in tc.fields().iter() {
        let key = field_key_name(field)?;
        let val = field_string_value(field)?;
        match key.as_str() {
            "resource_name" => resource_name = Some(val),
            "function_name" => function_name = Some(val),
            other => {
                return Err(format!(
                    "CFX_FUNCTION contains unrecognized field '{other}'. Only 'resource_name' and 'function_name' are allowed."
                ));
            }
        }
    }

    if resource_name.is_none() || function_name.is_none() {
        Err(
            "CFX_FUNCTION annotation must contain 'resource_name' and 'function_name' strings."
                .to_string(),
        )
    } else {
        Ok(())
    }
}

// --- Recursive Category C Validation ---

fn validate_fields_recursive(
    tc: &full_moon::ast::TableConstructor,
    ir: &IndexMap<String, ConfigNode>,
    diagnostics: &mut Vec<Diagnostic>,
    attached_tokens: &mut Vec<Token>,
) {
    for field in tc.fields().iter() {
        let key = match field_key_str(field) {
            Some(k) => k,
            None => continue,
        };

        let node = match ir.get(&key) {
            Some(n) => n,
            None => continue,
        };

        // Validate any annotations in the leading trivia of this field
        let trivia_list = field_key_trivia(field);
        let mut has_intervening_comment = false;
        let mut annotations_in_trivia = Vec::new();

        for token in &trivia_list {
            match token.token_type() {
                TokenType::SingleLineComment { .. } | TokenType::MultiLineComment { .. } => {
                    if let Some(ann) = trivia_parser::parse_trivia_token(token) {
                        annotations_in_trivia.push(((*token).clone(), ann));
                        has_intervening_comment = false;
                    } else {
                        // A generic comment
                        has_intervening_comment = true;
                    }
                }
                _ => {}
            }
        }

        // If we found annotations, the last one in the trivia list should be verified for dangling constraints
        if let Some((token, _ann)) = annotations_in_trivia.last() {
            let start = token.start_position();
            let token_line = start.line();
            let token_col = start.character();

            // Count newlines after the annotation token
            let mut newlines_after = 0;
            let mut other_comments_after = false;
            let mut found_ann = false;

            for t in &trivia_list {
                if t.start_position().bytes() == token.start_position().bytes() {
                    found_ann = true;
                    continue;
                }
                if found_ann {
                    match t.token_type() {
                        TokenType::Whitespace { characters } => {
                            newlines_after += characters.chars().filter(|&c| c == '\n').count();
                        }
                        TokenType::SingleLineComment { .. }
                        | TokenType::MultiLineComment { .. } => {
                            other_comments_after = true;
                        }
                        _ => {}
                    }
                }
            }

            let is_dangling = newlines_after > 1 || other_comments_after || has_intervening_comment;

            if is_dangling {
                diagnostics.push(Diagnostic {
                    severity: "Error".to_string(),
                    line: token_line,
                    character: token_col,
                    message: "Structural Error: Dangling annotation. Annotations must be immediately followed by a configuration field or variable assignment.".to_string(),
                    source: "configir-linter".to_string(),
                });
            } else {
                // Record it as successfully attached!
                attached_tokens.push((*token).clone());

                // Now run Type Mismatch checks (Category C)
                if let Some(meta) = get_node_metadata(node) {
                    validate_type_mismatch(&key, node, meta, token_line, token_col, diagnostics);
                }
            }
        }

        // If the value is a nested table constructor, recurse
        if let ConfigNode::Table(t) = node {
            if let Some(expr) = field_value_expr(field) {
                if let Expression::TableConstructor(sub_tc) = expr {
                    validate_fields_recursive(sub_tc, &t.fields, diagnostics, attached_tokens);
                }
            }
        }
    }
}

fn validate_type_mismatch(
    field_name: &str,
    node: &ConfigNode,
    meta: &FieldMetadata,
    line: usize,
    col: usize,
    diagnostics: &mut Vec<Diagnostic>,
) {
    // 1. MAP Mismatch
    if let Some(true) = meta.map {
        match node {
            ConfigNode::Vector2(_) | ConfigNode::Vector3(_) => {}
            _ => {
                diagnostics.push(Diagnostic {
                    severity: "Error".to_string(),
                    line,
                    character: col,
                    message: format!("Type Mismatch: Field '{field_name}' is annotated with MAP = true but its value is not a vector2 or vector3."),
                    source: "configir-linter".to_string(),
                });
            }
        }
    }

    // 2. ENUM Mismatch
    if let Some(options) = &meta.enum_options {
        match node {
            ConfigNode::Enum(ev) => {
                if !options.contains(&ev.value) {
                    diagnostics.push(Diagnostic {
                        severity: "Error".to_string(),
                        line,
                        character: col,
                        message: format!(
                            "Type Mismatch: Field '{field_name}' default value '{}' does not exist in the specified ENUM options [{}].",
                            ev.value,
                            options.join(", ")
                        ),
                        source: "configir-linter".to_string(),
                    });
                }
            }
            ConfigNode::String(sv) => {
                if !options.contains(&sv.value) {
                    diagnostics.push(Diagnostic {
                        severity: "Error".to_string(),
                        line,
                        character: col,
                        message: format!(
                            "Type Mismatch: Field '{field_name}' default value '{}' does not exist in the specified ENUM options [{}].",
                            sv.value,
                            options.join(", ")
                        ),
                        source: "configir-linter".to_string(),
                    });
                }
            }
            _ => {
                diagnostics.push(Diagnostic {
                    severity: "Error".to_string(),
                    line,
                    character: col,
                    message: format!(
                        "Type Mismatch: Field '{field_name}' must be a string literal matching the ENUM options [{}].",
                        options.join(", ")
                    ),
                    source: "configir-linter".to_string(),
                });
            }
        }
    }

    // 3. RANGE Mismatch
    if let Some(range) = &meta.range {
        match node {
            ConfigNode::Number(nv) => {
                if nv.value < range.min || nv.value > range.max {
                    diagnostics.push(Diagnostic {
                        severity: "Error".to_string(),
                        line,
                        character: col,
                        message: format!(
                            "Type Mismatch: Field '{field_name}' default value {} is out of the defined RANGE bounds [{}, {}].",
                            nv.value, range.min, range.max
                        ),
                        source: "configir-linter".to_string(),
                    });
                }
            }
            _ => {
                diagnostics.push(Diagnostic {
                    severity: "Error".to_string(),
                    line,
                    character: col,
                    message: format!(
                        "Type Mismatch: Field '{field_name}' must be a number literal falling within RANGE bounds [{}, {}].",
                        range.min, range.max
                    ),
                    source: "configir-linter".to_string(),
                });
            }
        }
    }

    // 4. CFX_FUNCTION Mismatch
    if meta.function_info.is_some() {
        match node {
            ConfigNode::Function(_) | ConfigNode::Expression(_) => {}
            _ => {
                diagnostics.push(Diagnostic {
                    severity: "Error".to_string(),
                    line,
                    character: col,
                    message: format!(
                        "Type Mismatch: Field '{field_name}' is annotated with CFX_FUNCTION but its value is not a function."
                    ),
                    source: "configir-linter".to_string(),
                });
            }
        }
    }

    // 5. TABLE / ITEMS Schema Mismatch
    if let Some(schema) = &meta.table_schema {
        match node {
            ConfigNode::Table(t) => {
                // Static table layout: check each column schema
                for col_schema in &schema.schema {
                    if let Some(sub_node) = t.fields.get(&col_schema.name) {
                        if let Err(expected_t) =
                            match_primitive_type(sub_node, &col_schema.column_type)
                        {
                            diagnostics.push(Diagnostic {
                                severity: "Error".to_string(),
                                line,
                                character: col,
                                message: format!(
                                    "Type Mismatch: Field '{}' in table '{}' expects type '{}' but got '{}'.",
                                    col_schema.name, field_name, col_schema.column_type, expected_t
                                ),
                                source: "configir-linter".to_string(),
                            });
                        }
                    }
                }
            }
            ConfigNode::DynamicTable(dt) => {
                // Dynamic table list/rows layout: check columns of each row
                for (row_idx, row) in dt.rows.iter().enumerate() {
                    let row_key = row
                        .get("_key")
                        .and_then(|v| v.as_str())
                        .unwrap_or_else(|| "row");
                    for col_schema in &schema.schema {
                        if col_schema.is_key.unwrap_or(false) {
                            continue;
                        }
                        if let Some(val) = row.get(&col_schema.name) {
                            if let Err(actual_t) =
                                match_json_primitive_type(val, &col_schema.column_type)
                            {
                                diagnostics.push(Diagnostic {
                                    severity: "Error".to_string(),
                                    line,
                                    character: col,
                                    message: format!(
                                        "Type Mismatch: Field '{}' in row '{}' (index {}) expects type '{}' but got '{}'.",
                                        col_schema.name, row_key, row_idx, col_schema.column_type, actual_t
                                    ),
                                    source: "configir-linter".to_string(),
                                });
                            }
                        }
                    }
                }
            }
            _ => {
                diagnostics.push(Diagnostic {
                    severity: "Error".to_string(),
                    line,
                    character: col,
                    message: format!(
                        "Type Mismatch: Field '{field_name}' is annotated with TABLE/ITEMS but its value is not a table."
                    ),
                    source: "configir-linter".to_string(),
                });
            }
        }
    }
}

fn match_primitive_type(node: &ConfigNode, expected: &str) -> Result<(), String> {
    match expected {
        "string" => match node {
            ConfigNode::String(_) | ConfigNode::Enum(_) => Ok(()),
            other => Err(get_node_type_name(other)),
        },
        "number" => match node {
            ConfigNode::Number(_) => Ok(()),
            other => Err(get_node_type_name(other)),
        },
        "boolean" => match node {
            ConfigNode::Boolean(_) => Ok(()),
            other => Err(get_node_type_name(other)),
        },
        "vector2" => match node {
            ConfigNode::Vector2(_) => Ok(()),
            other => Err(get_node_type_name(other)),
        },
        "vector3" => match node {
            ConfigNode::Vector3(_) => Ok(()),
            other => Err(get_node_type_name(other)),
        },
        _ => Ok(()),
    }
}

fn match_json_primitive_type(val: &serde_json::Value, expected: &str) -> Result<(), String> {
    match expected {
        "string" => {
            if val.is_string() {
                Ok(())
            } else {
                Err(get_json_type_name(val))
            }
        }
        "number" => {
            if val.is_number() {
                Ok(())
            } else {
                Err(get_json_type_name(val))
            }
        }
        "boolean" => {
            if val.is_boolean() {
                Ok(())
            } else {
                Err(get_json_type_name(val))
            }
        }
        _ => Ok(()),
    }
}

fn get_node_type_name(node: &ConfigNode) -> String {
    match node {
        ConfigNode::String(_) => "string".to_string(),
        ConfigNode::Number(_) => "number".to_string(),
        ConfigNode::Boolean(_) => "boolean".to_string(),
        ConfigNode::Enum(_) => "string".to_string(),
        ConfigNode::Vector2(_) => "vector2".to_string(),
        ConfigNode::Vector3(_) => "vector3".to_string(),
        ConfigNode::Table(_) => "table".to_string(),
        ConfigNode::DynamicTable(_) => "table".to_string(),
        ConfigNode::Function(_) => "function".to_string(),
        ConfigNode::Expression(_) => "expression".to_string(),
        ConfigNode::Nil(_) => "nil".to_string(),
        ConfigNode::Array(_) => "array".to_string(),
    }
}

fn get_json_type_name(val: &serde_json::Value) -> String {
    match val {
        serde_json::Value::Null => "null".to_string(),
        serde_json::Value::Bool(_) => "boolean".to_string(),
        serde_json::Value::Number(_) => "number".to_string(),
        serde_json::Value::String(_) => "string".to_string(),
        serde_json::Value::Array(_) => "array".to_string(),
        serde_json::Value::Object(_) => "object".to_string(),
    }
}

fn get_node_metadata(node: &ConfigNode) -> Option<&FieldMetadata> {
    match node {
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
    }
}

// --- AST Traversal & Parsing Helpers ---

fn extract_table_from_ast(
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
    Err("expected table constructor definition".to_string())
}

fn field_key_name(field: &Field) -> Result<String, String> {
    match field {
        Field::NameKey { key, .. } => Ok(key.token().to_string()),
        Field::ExpressionKey { key, .. } => {
            if let Expression::String(token) = key {
                if let TokenType::StringLiteral { literal, .. } = token.token().token_type() {
                    return Ok(literal.to_string());
                }
            }
            Err("expected expression key to be a string".to_string())
        }
        Field::NoKey(_) => Err("field must have a key".to_string()),
        _ => Err("unknown field type".to_string()),
    }
}

fn field_number_value(field: &Field) -> Result<f64, String> {
    let expr = field_value_expr(field).ok_or_else(|| "field value not found".to_string())?;
    match expr {
        Expression::Number(token) => {
            let s = token.to_string();
            s.parse::<f64>().map_err(|e| format!("parse number: {e}"))
        }
        // Handle negative numbers
        Expression::UnaryOperator { unop, expression } => {
            if let full_moon::ast::UnOp::Minus(_) = unop {
                if let Expression::Number(token) = expression.as_ref() {
                    let s = token.to_string();
                    let val = s.parse::<f64>().map_err(|e| format!("parse number: {e}"))?;
                    return Ok(-val);
                }
            }
            Err("expected unary minus on number".to_string())
        }
        _ => Err("expected number literal".to_string()),
    }
}

fn field_string_value(field: &Field) -> Result<String, String> {
    let expr = field_value_expr(field).ok_or_else(|| "field value not found".to_string())?;
    match expr {
        Expression::String(token) => {
            if let TokenType::StringLiteral { literal, .. } = token.token().token_type() {
                Ok(literal.to_string())
            } else {
                Ok(token.to_string().trim_matches('"').to_string())
            }
        }
        _ => Err("expected string literal".to_string()),
    }
}

fn field_table_value(field: &Field) -> Result<&full_moon::ast::TableConstructor, String> {
    let expr = field_value_expr(field).ok_or_else(|| "field value not found".to_string())?;
    match expr {
        Expression::TableConstructor(tc) => Ok(tc),
        _ => Err("expected table constructor".to_string()),
    }
}

fn field_value_expr(field: &Field) -> Option<&Expression> {
    match field {
        Field::NameKey { value, .. } | Field::ExpressionKey { value, .. } => Some(value),
        Field::NoKey(expr) => Some(expr),
        _ => None,
    }
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

fn field_key_str(field: &Field) -> Option<String> {
    match field {
        Field::NameKey { key, .. } => Some(key.token().to_string()),
        Field::ExpressionKey { key, .. } => {
            if let Expression::String(token) = key {
                if let TokenType::StringLiteral { literal, .. } = token.token().token_type() {
                    Some(literal.to_string())
                } else {
                    Some(token.to_string().trim_matches('"').to_string())
                }
            } else {
                None
            }
        }
        Field::NoKey(_) => None,
        _ => None,
    }
}
