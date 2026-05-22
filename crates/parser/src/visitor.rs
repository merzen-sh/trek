use full_moon::{
    ast::{
        Ast, Call, Expression, Field, FunctionArgs, Prefix, Stmt, Suffix, Var,
        punctuated::Punctuated,
    },
    node::Node,
    tokenizer::{TokenReference, TokenType},
};
use std::cell::Cell;

use crate::models::{
    CfxFunctionNode, ConfigDoc, ConfigIR, ConfigNode,
    EnumMeta, EnumNode,
    ScalarMeta, ScalarNode,
    TableMeta, TableNode,
    Vector2Node, Vector2Value, Vector3Node, Vector3Value,
};
use crate::trivia_parser::{extract_string_value, parse_annotations, parse_lua_key_values};
use indexmap::IndexMap;

const MAX_RECURSION: u32 = 128;

struct DepthGuard<'a> {
    depth: &'a Cell<u32>,
}

impl Drop for DepthGuard<'_> {
    fn drop(&mut self) {
        self.depth.set(self.depth.get() - 1);
    }
}

pub struct LuaAstVisitor<'s> {
    ast: &'s Ast,
    depth: Cell<u32>,
}

impl<'s> LuaAstVisitor<'s> {
    pub fn new(ast: &'s Ast) -> Self {
        Self { ast, depth: Cell::new(0) }
    }

    fn guard_depth(&self) -> Result<DepthGuard<'_>, ()> {
        let d = self.depth.get();
        if d >= MAX_RECURSION {
            return Err(());
        }
        self.depth.set(d + 1);
        Ok(DepthGuard { depth: &self.depth })
    }

    pub fn extract(&self) -> ConfigDoc {
        let mut ir = self.collect_stmts(self.ast.nodes().stmts());
        if let Some(last) = self.ast.nodes().last_stmt() {
            if let full_moon::ast::LastStmt::Return(r) = last {
                self.collect_exprs(r.returns(), &mut ir);
            }
        }

        ConfigDoc {
            meta:   self.extract_meta(),
            fields: ir,
        }
    }

    fn extract_meta(&self) -> Option<IndexMap<String, serde_json::Value>> {
        use full_moon::tokenizer::TokenType;
        let first = self.ast.nodes().stmts().next();
        let token = if let Some(stmt) = first {
            stmt.tokens().next()
        } else {
            self.ast.nodes().last_stmt()?.tokens().next()
        }?;
        token.leading_trivia().find_map(|t| {
            if let TokenType::MultiLineComment { comment, .. } = t.token_type() {
                if comment.trim_start().starts_with("@TABLE") { return None; }
                let map = parse_lua_key_values(comment);
                if !map.is_empty() { Some(map) } else { None }
            } else {
                None
            }
        })
    }

    fn collect_stmts<'a>(&self, stmts: impl Iterator<Item = &'a Stmt>) -> ConfigIR {
        let mut ir = ConfigIR::new();
        for stmt in stmts {
            match stmt {
                Stmt::LocalAssignment(a) => self.collect_exprs(a.expressions(), &mut ir),
                Stmt::Assignment(a)      => self.collect_exprs(a.expressions(), &mut ir),
                _ => {}
            }
        }
        ir
    }

    fn collect_exprs(&self, exprs: &Punctuated<Expression>, ir: &mut ConfigIR) {
        for expr in exprs.iter() {
            if let Expression::TableConstructor(table) = expr {
                self.collect_fields(table.fields(), ir);
            }
        }
    }

    // Recursive field collection
    fn collect_fields(&self, fields: &Punctuated<Field>, ir: &mut ConfigIR) {
        for field in fields.iter() {
            if let Field::NameKey { key, value, .. } = field {
                ir.insert(key.token().to_string(), self.build_node(key, value));
            }
        }
    }

    fn build_node(&self, key: &TokenReference, value: &Expression) -> ConfigNode {
        let mut ann = parse_annotations(key);

        if let Expression::TableConstructor(table) = value {
            let mut rows = ConfigIR::new();
            // Guard against stack overflow from deeply nested tables.
            if self.guard_depth().is_ok() {
                self.collect_fields(table.fields(), &mut rows);
            }
            // DepthGuard drops here, decrementing depth.

            let description = ann.take_description();
            let schema = ann.table_schema.take();
                let metadata = if description.is_some() || schema.is_some() {
                    Some(TableMeta { description, schema })
                } else {
                    None
                };
            return ConfigNode::Table(TableNode { rows, metadata });
        }

        // CFX_FUNCTION (annotation-driven, overrides type inference for function values)
        if let Some(mut cfx_meta) = ann.cfx_function.take() {
            cfx_meta.description = ann.take_description().or(cfx_meta.description);
            return ConfigNode::CfxFunction(CfxFunctionNode { metadata: cfx_meta });
        }

        // Vector2 / Vector3 (detect vector2() / vector3() function calls)
        if let Some((vec_type, nums)) = try_parse_vector(value) {
            let description = ann.take_description();
            let range = ann.range.take();
            let metadata = (description.is_some() || range.is_some())
                .then(|| ScalarMeta { description, range });
            return match vec_type {
                "vector2" => ConfigNode::Vector2(Vector2Node {
                    value: Vector2Value { x: nums[0], y: nums[1] },
                    metadata,
                }),
                "vector3" => ConfigNode::Vector3(Vector3Node {
                    value: Vector3Value { x: nums[0], y: nums[1], z: nums[2] },
                    metadata,
                }),
                _ => unreachable!(),
            };
        }

        // Scalars
        let raw = extract_string_value(value);

        // Enum (annotation-driven, overrides type inference)
        if let Some(options) = ann.enum_options.take() {
            let description = ann.take_description();
            return ConfigNode::Enum(EnumNode {
                value:    raw,
                metadata: Some(EnumMeta { description, options }),
            });
        }

        let description = ann.take_description();
        let range       = ann.range.take();

        match value {
            Expression::Number(_) => {
                let metadata = (description.is_some() || range.is_some())
                    .then(|| ScalarMeta { description, range });
                ConfigNode::Number(ScalarNode { value: raw, metadata })
            }
            Expression::Symbol(_) if raw == "true" || raw == "false" => {
                let metadata = description
                    .map(|d| ScalarMeta { description: Some(d), range: None });
                ConfigNode::Boolean(ScalarNode { value: raw == "true", metadata })
            }
            _ => {
                let metadata = (description.is_some() || range.is_some())
        .then(|| ScalarMeta { description, range });
    ConfigNode::String(ScalarNode { value: raw, metadata })
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Vector2 / Vector3 detection
// ---------------------------------------------------------------------------

pub(crate) fn try_parse_vector(expr: &Expression) -> Option<(&'static str, Vec<f64>)> {
    let (prefix, suffixes) = match expr {
        Expression::FunctionCall(fc) => (fc.prefix(), fc.suffixes().collect::<Vec<_>>()),
        Expression::Var(Var::Expression(ve)) => (ve.prefix(), ve.suffixes().collect::<Vec<_>>()),
        _ => return None,
    };

    let name = match prefix {
        Prefix::Name(token) => token.token().to_string(),
        _ => return None,
    };

    let dim = match name.as_str() {
        "vector2" => 2usize,
        "vector3" => 3usize,
        _ => return None,
    };

    let call = suffixes.first()?;
    let Suffix::Call(Call::AnonymousCall(FunctionArgs::Parentheses { arguments, .. })) = call else {
        return None;
    };

    fn extract_number(expr: &Expression) -> Option<f64> {
        match expr {
            Expression::Number(t) => {
                if let TokenType::Number { text } = t.token().token_type() {
                    text.parse::<f64>().ok()
                } else {
                    None
                }
            }
            Expression::UnaryOperator { unop, expression } => {
                if unop.token().to_string() == "-" {
                    extract_number(expression).map(|n| -n)
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    let nums: Vec<f64> = arguments.iter().filter_map(extract_number).collect();

    if nums.len() != dim {
        return None;
    }

    Some((if dim == 2 { "vector2" } else { "vector3" }, nums))
}
