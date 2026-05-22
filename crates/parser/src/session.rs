use full_moon::ast::Ast;

use crate::layout_visitor::LayoutVisitor;
use crate::models::LayoutDoc;
use crate::navigator::{
    find_root_table, patch_scalar_at_path, patch_table_remove_row_at_path, patch_table_row_at_path,
    value_at_path,
};
use crate::guarded_parse;
use serde_json::Value;

/// In-memory AST session — single source of truth for lossless config editing.
pub struct ConfigSession {
    ast: Ast,
}

impl ConfigSession {
    pub fn from_source(source: &str) -> Result<Self, String> {
        let ast = guarded_parse(source)?;
        if find_root_table(&ast).is_none() {
            return Err("no config table found in source".into());
        }
        Ok(Self { ast })
    }

    pub fn get_layout(&self) -> LayoutDoc {
        LayoutVisitor::new(&self.ast).extract()
    }

    pub fn get_layout_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(&self.get_layout())
            .map_err(|e| format!("serialize error: {e}"))
    }

    pub fn get_value_at_path(&self, path: &[String]) -> Result<Value, String> {
        let root = find_root_table(&self.ast).ok_or("no config table")?;
        value_at_path(root, path)
    }

    pub fn get_value_json_at_path(&self, path: Vec<String>) -> Result<String, String> {
        let value = self.get_value_at_path(&path)?;
        serde_json::to_string(&value).map_err(|e| format!("serialize error: {e}"))
    }

    /// Patch a scalar value at `path` (path includes the target field key).
    pub fn patch_value_at_path(&mut self, path: &[String], payload: &Value) -> Result<(), String> {
        let ast = self.ast.clone();
        self.ast = patch_scalar_at_path(ast, path, payload.clone())?;
        Ok(())
    }

    /// Append a new row to the table at `path`.
    pub fn patch_table_append(
        &mut self,
        table_path: &[String],
        row_key: &str,
        row_payload: &Value,
    ) -> Result<(), String> {
        let ast = self.ast.clone();
        self.ast = patch_table_row_at_path(ast, table_path, row_key, row_payload)?;
        Ok(())
    }

    /// Remove a row by key from the table at `path`.
    pub fn patch_table_remove_row(
        &mut self,
        table_path: &[String],
        row_key: &str,
    ) -> Result<(), String> {
        let ast = self.ast.clone();
        self.ast = patch_table_remove_row_at_path(ast, table_path, row_key)?;
        Ok(())
    }

    /// Lossless Lua output via full_moon's Display implementation.
    pub fn print(&self) -> String {
        self.ast.to_string()
    }

    /// Alias for [`Self::print`].
    pub fn to_lua(&self) -> String {
        self.print()
    }

    pub fn ast(&self) -> &Ast {
        &self.ast
    }
}
