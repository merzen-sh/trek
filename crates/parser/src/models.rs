use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

pub type ConfigIR = IndexMap<String, ConfigNode>;

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ConfigNode {
    String(StringValue),
    Number(NumberValue),
    Boolean(BooleanValue),
    Enum(EnumValue),
    Vector2(Vector2Value),
    Vector3(Vector3Value),
    Table(TableValue),
    DynamicTable(DynamicTableValue),
    Function(FunctionValue),
    Expression(ExpressionValue),
    Nil(NilValue),
    Array(ArrayValue),
}

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct StringValue {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<Box<FieldMetadata>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct NumberValue {
    pub value: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<Box<FieldMetadata>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct BooleanValue {
    pub value: bool,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<Box<FieldMetadata>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct EnumValue {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<Box<FieldMetadata>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct Vector2Value {
    pub x: f64,
    pub y: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<Box<FieldMetadata>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct Vector3Value {
    pub x: f64,
    pub y: f64,
    pub z: f64,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<Box<FieldMetadata>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct TableValue {
    pub fields: IndexMap<String, ConfigNode>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<Box<FieldMetadata>>,
}

pub type Row = IndexMap<String, serde_json::Value>;

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct DynamicTableValue {
    pub rows: Vec<Row>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<Box<FieldMetadata>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct ArrayValue {
    pub items: Vec<ConfigNode>,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<Box<FieldMetadata>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct FunctionValue {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<Box<FieldMetadata>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct ExpressionValue {
    pub value: String,
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<Box<FieldMetadata>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct NilValue {
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub metadata: Option<Box<FieldMetadata>>,
}

#[derive(Serialize, Deserialize, Debug, Clone, Default, schemars::JsonSchema)]
pub struct FieldMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<RangeValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enum_options: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub map: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub nil_marker: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub function_info: Option<FunctionInfo>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub table_schema: Option<TableSchema>,
}

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct RangeValue {
    pub min: f64,
    pub max: f64,
}

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct FunctionInfo {
    pub resource_name: String,
    pub function_name: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct TableSchema {
    pub layout: String,
    pub schema: Vec<ColumnSchema>,
}

#[derive(Serialize, Deserialize, Debug, Clone, schemars::JsonSchema)]
pub struct ColumnSchema {
    pub name: String,
    #[serde(rename = "type")]
    pub column_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_key: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

impl FieldMetadata {
    pub fn merge(&mut self, other: FieldMetadata) {
        if let Some(desc) = other.description {
            self.description = Some(desc);
        }
        if other.range.is_some() {
            self.range = other.range;
        }
        if other.enum_options.is_some() {
            self.enum_options = other.enum_options;
        }
        if let Some(m) = other.map {
            self.map = Some(m);
        }
        if other.nil_marker.is_some() {
            self.nil_marker = other.nil_marker;
        }
        if other.function_info.is_some() {
            self.function_info = other.function_info;
        }
        if other.table_schema.is_some() {
            self.table_schema = other.table_schema;
        }
    }
}
