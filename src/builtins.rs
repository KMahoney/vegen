use std::collections::HashMap;

use once_cell::sync::Lazy;

#[derive(Clone)]
pub enum BuiltinType {
    Prim(String),
    Var(u32),
}

type Builtin = (Vec<BuiltinType>, BuiltinType);

pub static BUILTINS: Lazy<HashMap<String, Builtin>> = Lazy::new(|| {
    let mut s = HashMap::new();

    let num = BuiltinType::Prim("number".to_string());
    let str = BuiltinType::Prim("string".to_string());
    let bool = BuiltinType::Prim("boolean".to_string());

    fn v(id: u32) -> BuiltinType {
        BuiltinType::Var(id)
    }

    s.insert("numberToString".to_string(), (vec![num], str.clone()));
    s.insert("boolean".to_string(), (vec![bool, v(0), v(0)], v(0)));
    s.insert("lookup".to_string(), (vec![v(0), str.clone(), v(1)], v(1)));

    s
});
