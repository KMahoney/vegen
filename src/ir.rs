use crate::{ast::AttrValue, expr::Expr, ts_type::TsType};

use std::collections::BTreeMap;

#[derive(Debug, Clone)]
pub struct ViewDefinition {
    pub view_name: String,
    pub context: CompileContext,
    pub ts_type: TsType,
    pub root: JsExpr,
}

#[derive(Debug, Clone)]
pub struct CompileContext {
    pub constructors: Vec<JsExpr>,
    pub updaters: Vec<JsUpdater>,
    pub child_views: Vec<CompiledView>,
    pub for_loops: Vec<ForLoopInfo>,
    pub ifs: Vec<IfInfo>,
    pub switches: Vec<SwitchInfo>,
    pub component_calls: Vec<ComponentCallInfo>,
    pub use_views: Vec<UseInfo>,
}

impl CompileContext {
    pub fn new() -> Self {
        Self {
            constructors: Vec::new(),
            updaters: Vec::new(),
            child_views: Vec::new(),
            for_loops: Vec::new(),
            ifs: Vec::new(),
            switches: Vec::new(),
            component_calls: Vec::new(),
            use_views: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CompiledView {
    pub root: JsExpr,
    pub context: CompileContext,
}

#[derive(Debug, Clone)]
pub enum JsExpr {
    Element {
        tag: String,
        props: Vec<(String, AttrValue)>,
        dataset: Vec<(String, AttrValue)>,
        children: Vec<JsExpr>,
    },
    Text(String),
    Expr(Expr),
    Ref(usize),
    LoopElements(usize),
    ConditionalElement(usize),
    SwitchElement(usize),
    Use(usize),
    ComponentCall(usize),
}

#[derive(Debug, Clone)]
pub enum UpdateKind {
    Text {
        node_idx: usize,
        value: AttrValue,
    },
    Prop {
        node_idx: usize,
        prop: String,
        value: AttrValue,
    },
    Dataset {
        node_idx: usize,
        key: String,
        value: AttrValue,
    },
}

#[derive(Debug, Clone)]
pub struct JsUpdater {
    pub dependencies: Vec<String>,
    pub kind: UpdateKind,
}

#[derive(Debug, Clone)]
pub struct ForLoopInfo {
    pub child_view_idx: usize,
    pub sequence_expr: Expr,
    pub var_name: String,
}

#[derive(Debug, Clone)]
pub struct IfInfo {
    pub then_view_idx: Option<usize>,
    pub else_view_idx: Option<usize>,
    pub condition_expr: Expr,
}

#[derive(Debug, Clone)]
pub struct SwitchInfo {
    pub case_view_idxs: Vec<usize>,
    pub case_names: Vec<String>,
    pub on_expr: Expr,
}

#[derive(Debug, Clone)]
pub struct ComponentCallInfo {
    pub target_view_name: String,
    pub input_attrs: BTreeMap<String, Expr>,
}

#[derive(Debug, Clone)]
pub struct UseInfo {
    pub view_expr: Expr,
    pub view_dependencies: Vec<String>,
    pub input_attrs: BTreeMap<String, Expr>,
}
