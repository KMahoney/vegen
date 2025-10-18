use itertools::Itertools;
use std::collections::BTreeMap;
use std::fmt;

use crate::ts_util::render_key;
use crate::type_system::environment::Env;
use crate::type_system::solver::canonical_type;
use crate::type_system::types::{RowDescriptor, Type};
use crate::type_system::uf;

#[derive(Debug, Clone)]
pub enum TsType {
    SimpleType(String),
    Object(BTreeMap<String, TsType>),
    Array(Box<TsType>),
    Function(Vec<TsType>, Box<TsType>),
    Union(Vec<TsType>),
    View(BTreeMap<String, TsType>),
}

impl fmt::Display for TsType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn render_object(obj: &BTreeMap<String, TsType>) -> String {
            if obj.is_empty() {
                "{}".to_string()
            } else {
                let field_strings: Vec<String> = obj
                    .iter()
                    .map(|(key, value)| format!("{}: {}", render_key(key), value))
                    .collect();
                format!("{{ {} }}", field_strings.join(", "))
            }
        }

        match self {
            TsType::SimpleType(s) => write!(f, "{}", s),
            TsType::Object(fields) => write!(f, "{}", render_object(fields)),
            TsType::Array(element_type) => write!(f, "{}[]", element_type),
            TsType::Function(params, return_type) => {
                let param_strings: Vec<String> = params
                    .iter()
                    .enumerate()
                    .map(|(i, p)| format!("v{}: {}", i, p))
                    .collect();
                write!(f, "({}) => {}", param_strings.join(", "), return_type)
            }
            TsType::Union(types) => {
                let parts: Vec<String> = types.iter().map(|t| t.to_string()).collect();
                write!(f, "{}", parts.join(" | "))
            }
            TsType::View(fields) => write!(f, "View<{}>", render_object(fields)),
        }
    }
}

/// Convert an environment (variable name -> type mapping) to a TsType::Object
pub fn env_to_ts_type(env: &Env) -> TsType {
    let mut fields = BTreeMap::new();
    for (name, ty) in env {
        let canonical = canonical_type(ty);
        let ts_type = type_to_ts_type(&canonical);
        fields.insert(name.clone(), ts_type);
    }
    TsType::Object(fields)
}

/// Convert a canonical Type to TsType
/// The input type should already be canonical (all type variables resolved)
pub fn type_to_ts_type(ty: &Type) -> TsType {
    match ty {
        Type::Prim(name) => TsType::SimpleType(name.clone()),
        Type::Fun(params, ret) => {
            let param_types = params.iter().map(type_to_ts_type).collect_vec();
            let ret_type = Box::new(type_to_ts_type(ret));
            TsType::Function(param_types, ret_type)
        }
        Type::Array(elem) => TsType::Array(Box::new(type_to_ts_type(elem))),
        Type::Var(_) => {
            // If we get an unbound variable after canonicalization, treat it as 'any'
            TsType::SimpleType("any".to_string())
        }
        Type::Record(row) => {
            let fields = row_to_fields(row);
            TsType::Object(fields)
        }
        Type::DiscriminatedUnion(map) => {
            let mut variants: Vec<TsType> = Vec::new();
            for (k, row) in map {
                let mut fields = row_to_fields(row);
                fields.insert("type".to_string(), TsType::SimpleType(format!("\"{}\"", k)));
                variants.push(TsType::Object(fields));
            }
            TsType::Union(variants)
        }
        Type::View(attributes) => {
            let mut fields = BTreeMap::new();
            for (name, ty) in attributes {
                let canonical = canonical_type(ty);
                fields.insert(name.clone(), type_to_ts_type(&canonical));
            }
            TsType::View(fields)
        }
    }
}

/// Extract object fields from a row descriptor
fn row_to_fields(row: &crate::type_system::uf::Point<RowDescriptor>) -> BTreeMap<String, TsType> {
    let mut fields = BTreeMap::new();
    let descriptor = uf::get(row);

    match descriptor {
        RowDescriptor::RowExtend(row_fields, rest) => {
            // Add fields from this row extension
            for (name, ty) in row_fields {
                let canonical = canonical_type(&ty);
                fields.insert(name.clone(), type_to_ts_type(&canonical));
            }
            // Recursively collect fields from the rest of the row
            let rest_fields = row_to_fields(&rest);
            fields.extend(rest_fields);
        }
        RowDescriptor::RowFlex(_) => {
            // Open row, no additional fields
        }
    }

    fields
}
