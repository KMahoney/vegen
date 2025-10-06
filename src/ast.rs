use chumsky::span::SimpleSpan;

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
    Text {
        content: String,
        span: Span,
    },
    Binding(SpannedBinding),
}

impl Node {
    pub fn span(&self) -> &Span {
        match self {
            Node::Element { span, .. } => span,
            Node::Text { span, .. } => span,
            Node::Binding(binding) => &binding.span,
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
    Binding(SpannedBinding),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttrValueTemplateSegment {
    Literal(String),
    Binding(SpannedBinding),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpannedBinding {
    pub expr: crate::expr::Expr,
    pub span: Span,
}
