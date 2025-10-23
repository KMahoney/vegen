use crate::error::Error;
use crate::lang::{expect_element, find_literal_attr, validate_single_child, Node, Span};
use crate::template::source_map::TemplatePath;

#[derive(Debug, Clone)]
pub struct RequiredTemplate {
    pub span: Span,
    pub raw_src: String,
}

#[derive(Debug, Clone)]
pub struct ViewStub {
    pub name: String,
    pub name_span: Span,
    pub view_span: Span,
    pub root: Node,
    pub component_refs: Vec<ComponentRef>,
}

#[derive(Debug, Clone)]
pub struct ComponentRef {
    pub name: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct TemplateModule {
    pub path: TemplatePath,
    pub requires: Vec<RequiredTemplate>,
    pub views: Vec<ViewStub>,
}

impl TemplateModule {
    pub fn from_nodes(path: TemplatePath, nodes: Vec<Node>) -> Result<Self, Error> {
        let mut requires = Vec::new();
        let mut views = Vec::new();

        for node in &nodes {
            match node {
                Node::Element { name, .. } if name == "require" => {
                    requires.push(parse_require(node)?);
                }
                Node::Element { name, .. } if name == "view" => {
                    views.push(parse_view(node)?);
                }
                _ => {
                    let span = *node.span();
                    return Err(Error {
                        message: "Only <require> and <view> elements are allowed at the top level."
                            .to_string(),
                        main_span: span,
                        labels: vec![(
                            span,
                            "Remove or wrap this node inside a <view> element.".to_string(),
                        )],
                    });
                }
            }
        }

        Ok(Self {
            path,
            requires,
            views,
        })
    }
}

fn parse_require(node: &Node) -> Result<RequiredTemplate, Error> {
    let (attrs, children, span) = expect_element(node, "require")?;

    if !children.is_empty() {
        return Err(Error {
            message: "<require> must not have children.".to_string(),
            main_span: *span,
            labels: vec![(
                (*span),
                "Remove nested content; <require> is self-closing.".to_string(),
            )],
        });
    }

    for attr in attrs {
        if attr.name != "src" {
            return Err(Error {
                message: format!("Unexpected '{}' attribute on <require>.", attr.name),
                main_span: attr.span,
                labels: vec![(
                    (attr.span),
                    "Only the 'src' attribute is supported.".to_string(),
                )],
            });
        }
    }

    let (raw_src, _) = find_literal_attr(attrs, "src", span)?;

    Ok(RequiredTemplate {
        span: *span,
        raw_src,
    })
}

fn parse_view(node: &Node) -> Result<ViewStub, Error> {
    let (attrs, children, span) = expect_element(node, "view")?;
    let (name, name_span) = find_literal_attr(attrs, "name", span)?;

    if !name.chars().next().is_some_and(|c| c.is_ascii_uppercase()) {
        return Err(Error {
            message: "View names must start with an uppercase letter.".to_string(),
            main_span: name_span,
            labels: vec![(
                (name_span),
                "Rename this view to begin with an uppercase letter.".to_string(),
            )],
        });
    }

    validate_single_child(span, children)?;
    let root = children[0].clone();

    if matches!(root, Node::Expr(_)) {
        return Err(Error {
            message: "Expressions cannot be the root of a view.".to_string(),
            main_span: *span,
            labels: vec![(
                *span,
                "Wrap this expression inside an element or fragment.".to_string(),
            )],
        });
    }

    let mut component_refs = Vec::new();
    collect_component_refs(&root, &mut component_refs);

    Ok(ViewStub {
        name,
        name_span,
        view_span: *span,
        root,
        component_refs,
    })
}

fn collect_component_refs(node: &Node, refs: &mut Vec<ComponentRef>) {
    match node {
        Node::ComponentCall {
            name,
            name_span,
            children,
            ..
        } => {
            refs.push(ComponentRef {
                name: name.clone(),
                span: *name_span,
            });
            for child in children {
                collect_component_refs(child, refs);
            }
        }
        Node::Element { children, .. } => {
            for child in children {
                collect_component_refs(child, refs);
            }
        }
        Node::Text { .. } | Node::Expr(_) => {}
    }
}
