use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

#[cfg(feature = "schema")]
use schemars::JsonSchema;

/// Top-level document
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ConfigDoc {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub meta: Option<IndexMap<String, serde_json::Value>>,
    pub fields: ConfigIR,
}

pub type ConfigIR = IndexMap<String, ConfigNode>;

// ---------------------------------------------------------------------------
// Node variants — every node has exactly two concerns:
//   1. runtime value  (value / rows / flag)
//   2. metadata       (description, constraints, schema — all optional)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum ConfigNode {
    String(ScalarNode<String>),
    Number(ScalarNode<String>),   // keep as String so "10" round-trips losslessly
    Boolean(ScalarNode<bool>),
    Enum(EnumNode),
    Table(TableNode),
    CfxFunction(CfxFunctionNode),
    Vector2(Vector2Node),
    Vector3(Vector3Node),
}

// ---------------------------------------------------------------------------
// Scalar node  (String / Number / Boolean share the same shape)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ScalarNode<T> {
    pub value: T,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<ScalarMeta>,
}

/// Metadata for scalar fields.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ScalarMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Vec<String>>,
    /// `@RANGE = { min, max }` annotation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Vec<String>>,
}

// ---------------------------------------------------------------------------
// Vector2 / Vector3 nodes
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Vector2Node {
    pub value: Vector2Value,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<ScalarMeta>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Vector2Value {
    pub x: f64,
    pub y: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Vector3Node {
    pub value: Vector3Value,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<ScalarMeta>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Vector3Value {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

// ---------------------------------------------------------------------------
// Enum node
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct EnumNode {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<EnumMeta>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct EnumMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Vec<String>>,
    /// All allowed enum values declared via `@ENUM`.
    pub options: Vec<String>,
}

/// Table node
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct TableNode {
    /// Runtime rows keyed by row-id (the Lua key, e.g. `"key1"`).
    pub rows: ConfigIR,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<TableMeta>,
}

/// Metadata for table fields — description + optional `@TABLE` schema.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct TableMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Vec<String>>,
    /// Schema declared via `--[[@TABLE … ]]` block comment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub schema: Option<TableSchema>,
}

// ---------------------------------------------------------------------------
// @TABLE schema
// ---------------------------------------------------------------------------

/// Schema declared via `--[[@table … --]]` block comment.
/// Controls web UI behaviour for this table field.
#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct TableSchema {
    /// Allow the UI to insert new rows.
    #[serde(default)]
    pub allow_add: bool,
    /// Allow the UI to delete existing rows.
    #[serde(default)]
    pub allow_delete: bool,
    /// Allow the UI to edit cell values in existing rows.
    #[serde(default)]
    pub allow_edit: bool,
    /// Column definitions in declaration order.
    pub columns: Vec<ColumnDef>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ColumnDef {
    /// Lua field name — maps to a key inside each row sub-table.
    pub field: String,
    #[serde(rename = "type")]
    pub col_type: ColumnType,
    pub label: String,
    /// Allowed values — only meaningful when `col_type == Enum`.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub values: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub enum ColumnType {
    Key,
    String,
    Number,
    Enum,
    Boolean,
    #[serde(other)]
    Unknown,
}

// ---------------------------------------------------------------------------
// @CFX_FUNCTION metadata
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct CfxFunctionNode {
    pub metadata: CfxFunctionMeta,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct CfxFunctionMeta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Vec<String>>,
    pub args_schema: Vec<ArgDef>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct ArgDef {
    pub name: String,
    #[serde(rename = "type")]
    pub arg_type: ColumnType,
    pub label: String,
    #[serde(default)]
    pub required: bool,
}

impl From<&str> for ColumnType {
    fn from(s: &str) -> Self {
        match s {
            "key"     => Self::Key,
            "string"  => Self::String,
            "number"  => Self::Number,
            "enum"    => Self::Enum,
            "boolean" => Self::Boolean,
            _         => Self::Unknown,
        }
    }
}
