use chumsky::span::SimpleSpan;

use crate::expr::Expr;

pub type SourceId = usize;
pub type Span = SimpleSpan<usize, SourceId>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    Element {
        name: String,
        name_span: Span,
        attrs: Vec<SpannedAttribute>,
        children: Vec<Node>,
        span: Span,
    },
    ComponentCall {
        name: String,
        name_span: Span,
        attrs: Vec<SpannedAttribute>,
        children: Vec<Node>,
        span: Span,
    },
    Text {
        content: String,
        span: Span,
    },
    Expr(Expr),
}

impl Node {
    pub fn span(&self) -> &Span {
        match self {
            Node::Element { span, .. } => span,
            Node::ComponentCall { span, .. } => span,
            Node::Text { span, .. } => span,
            Node::Expr(expr) => expr.span(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedAttribute {
    pub name: String,
    pub name_span: Span,
    pub value: AttrValue,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrValue {
    Template(Vec<AttrValueTemplateSegment>),
    Expr(Expr),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrValueTemplateSegment {
    Literal(String),
    Expr(Expr),
}
