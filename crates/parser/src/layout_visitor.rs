use full_moon::{
    ast::{
        Ast, Expression, Field, Stmt,
        punctuated::Punctuated,
    },
    node::Node,
};
use std::cell::Cell;

use crate::models::{
    LayoutCfxFunctionNode, LayoutDoc, LayoutEnumNode, LayoutIR, LayoutNode, LayoutScalarNode,
    LayoutTableNode, LayoutVectorNode, EnumMeta, ScalarMeta, TableMeta,
};
use crate::trivia_parser::extract_string_value;
use crate::navigator::field_key_value;
use crate::trivia_parser::{parse_annotations, parse_lua_key_values};
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

pub struct LayoutVisitor<'s> {
    ast: &'s Ast,
    depth: Cell<u32>,
}

impl<'s> LayoutVisitor<'s> {
    pub fn new(ast: &'s Ast) -> Self {
        Self {
            ast,
            depth: Cell::new(0),
        }
    }

    fn guard_depth(&self) -> Result<DepthGuard<'_>, ()> {
        let d = self.depth.get();
        if d >= MAX_RECURSION {
            return Err(());
        }
        self.depth.set(d + 1);
        Ok(DepthGuard { depth: &self.depth })
    }

    pub fn extract(&self) -> LayoutDoc {
        let mut fields = self.collect_stmts(self.ast.nodes().stmts());
        if let Some(last) = self.ast.nodes().last_stmt() {
            if let full_moon::ast::LastStmt::Return(r) = last {
                self.collect_exprs(r.returns(), &mut fields);
            }
        }
        LayoutDoc {
            meta: self.extract_meta(),
            fields,
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
                if comment.trim_start().starts_with("@TABLE") {
                    return None;
                }
                let map = parse_lua_key_values(comment);
                if !map.is_empty() {
                    Some(map)
                } else {
                    None
                }
            } else {
                None
            }
        })
    }

    fn collect_stmts<'a>(&self, stmts: impl Iterator<Item = &'a Stmt>) -> LayoutIR {
        let mut ir = LayoutIR::new();
        for stmt in stmts {
            match stmt {
                Stmt::LocalAssignment(a) => self.collect_exprs(a.expressions(), &mut ir),
                Stmt::Assignment(a) => self.collect_exprs(a.expressions(), &mut ir),
                _ => {}
            }
        }
        ir
    }

    fn collect_exprs(&self, exprs: &Punctuated<Expression>, ir: &mut LayoutIR) {
        for expr in exprs.iter() {
            if let Expression::TableConstructor(table) = expr {
                self.collect_fields(table.fields(), ir, Vec::new());
            }
        }
    }

    fn collect_fields(
        &self,
        fields: &Punctuated<Field>,
        ir: &mut LayoutIR,
        parent_path: Vec<String>,
    ) {
        for field in fields.iter() {
            if let Some((key, value)) = field_key_value(field) {
                let mut path = parent_path.clone();
                path.push(key.clone());
                ir.insert(key, self.build_layout_node(&path, field, value));
            }
        }
    }

    fn build_layout_node(
        &self,
        path: &[String],
        field: &Field,
        value: &Expression,
    ) -> LayoutNode {
        let key_token = match field {
            Field::NameKey { key, .. } => key,
            Field::ExpressionKey { key, .. } => {
                if let Expression::String(t) = key {
                    t
                } else {
                    return LayoutNode::String(LayoutScalarNode {
                        ast_path: path.to_vec(),
                        metadata: None,
                    });
                }
            }
            _ => {
                return LayoutNode::String(LayoutScalarNode {
                    ast_path: path.to_vec(),
                    metadata: None,
                });
            }
        };
        let mut ann = parse_annotations(key_token);

        if let Expression::TableConstructor(table) = value {
            let description = ann.take_description();
            let schema = ann.table_schema.take();
            let is_row_table = schema
                .as_ref()
                .is_some_and(|s| !s.columns.is_empty() || s.allow_add || s.allow_delete);
            let metadata = (description.is_some() || schema.is_some())
                .then(|| TableMeta { description, schema });
            let mut child_fields = LayoutIR::new();
            // `@TABLE` schema tables use dynamic row keys — omit rows from the layout tree.
            if !is_row_table && self.guard_depth().is_ok() {
                self.collect_fields(table.fields(), &mut child_fields, path.to_vec());
            }
            return LayoutNode::Table(LayoutTableNode {
                ast_path: path.to_vec(),
                metadata,
                fields: child_fields,
            });
        }

        if let Some(mut cfx_meta) = ann.cfx_function.take() {
            cfx_meta.description = ann.take_description().or(cfx_meta.description);
            return LayoutNode::CfxFunction(LayoutCfxFunctionNode {
                ast_path: path.to_vec(),
                metadata: cfx_meta,
            });
        }

        if let Some((vec_type, _)) = crate::visitor::try_parse_vector(value) {
            let description = ann.take_description();
            let range = ann.range.take();
            let metadata = (description.is_some() || range.is_some())
                .then(|| ScalarMeta { description, range });
            return match vec_type {
                "vector2" => LayoutNode::Vector2(LayoutVectorNode {
                    ast_path: path.to_vec(),
                    metadata,
                }),
                "vector3" => LayoutNode::Vector3(LayoutVectorNode {
                    ast_path: path.to_vec(),
                    metadata,
                }),
                _ => unreachable!(),
            };
        }

        if let Some(options) = ann.enum_options.take() {
            let description = ann.take_description();
            return LayoutNode::Enum(LayoutEnumNode {
                ast_path: path.to_vec(),
                metadata: Some(EnumMeta { description, options }),
            });
        }

        let description = ann.take_description();
        let range = ann.range.take();
        let metadata = (description.is_some() || range.is_some())
            .then(|| ScalarMeta { description, range });

        match value {
            Expression::Number(_) => {
                let raw = extract_string_value(value);
                if raw.contains('.') {
                    LayoutNode::Float(LayoutScalarNode {
                        ast_path: path.to_vec(),
                        metadata,
                    })
                } else {
                    LayoutNode::Number(LayoutScalarNode {
                        ast_path: path.to_vec(),
                        metadata,
                    })
                }
            }
            Expression::Symbol(_) => LayoutNode::Boolean(LayoutScalarNode {
                ast_path: path.to_vec(),
                metadata,
            }),
            _ => LayoutNode::String(LayoutScalarNode {
                ast_path: path.to_vec(),
                metadata,
            }),
        }
    }
}
