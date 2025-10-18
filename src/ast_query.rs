use crate::ast::{AttrValue, AttrValueTemplateSegment, Node, Span, SpannedAttribute};
use crate::attribute_types::attribute_type;
use crate::error::Error;
use crate::expr::Expr;

// Validate that a node has exactly one child
pub fn validate_single_child(parent_span: &Span, children: &[Node]) -> Result<(), Error> {
    if children.len() != 1 {
        let mut labels = vec![(*parent_span, "Parent element".to_string())];
        if children.len() > 1 {
            for child in &children[1..] {
                labels.push((*child.span(), "Extraneous child element".to_string()));
            }
        }
        return Err(Error {
            message: "Element must have exactly one child.".to_string(),
            main_span: *parent_span,
            labels,
        });
    }
    Ok(())
}

// Validate that children are exactly the expected element names
pub fn validate_child_element_names(
    parent_span: &Span,
    children: &[Node],
    expected_names: &[&str],
) -> Result<(), Error> {
    for child in children {
        if let Node::Element { name, span, .. } = child {
            if !expected_names.contains(&name.as_str()) {
                return Err(Error {
                    message: format!(
                        "Unexpected child '{}' in element; expected one of: {}",
                        name,
                        expected_names.join(", ")
                    ),
                    main_span: *parent_span,
                    labels: vec![
                        (*parent_span, "Parent element".to_string()),
                        (*span, format!("Unexpected '{}' element", name)),
                    ],
                });
            }
        } else {
            return Err(Error {
                message: "Children must be elements".to_string(),
                main_span: *parent_span,
                labels: vec![
                    (*parent_span, "Parent element".to_string()),
                    (*child.span(), "Non-element child".to_string()),
                ],
            });
        }
    }
    Ok(())
}

// Validate that children are elements (for if statement validation)
pub fn validate_all_children_are_elements(
    parent_span: &Span,
    children: &[Node],
) -> Result<(), Error> {
    for child in children {
        if !matches!(child, Node::Element { .. }) {
            return Err(Error {
                message: "All children must be elements".to_string(),
                main_span: *parent_span,
                labels: vec![
                    (*parent_span, "Parent element".to_string()),
                    (*child.span(), "Non-element child".to_string()),
                ],
            });
        }
    }
    Ok(())
}

// Find and validate a binding attribute
pub fn find_binding_attr(
    attrs: &[SpannedAttribute],
    name: &str,
    span: &Span,
) -> Result<Expr, Error> {
    let attr = attrs
        .iter()
        .find(|attr| attr.name == name)
        .ok_or_else(|| Error {
            message: format!("Missing '{}' attribute", name),
            main_span: *span,
            labels: vec![(*span, "Missing attribute".to_string())],
        })?;

    match &attr.value {
        AttrValue::Expr(b) => Ok(b.clone()),
        _ => Err(Error {
            message: format!("'{}' attribute must be a binding", name),
            main_span: attr.span,
            labels: vec![(attr.span, "Attribute must be a binding".to_string())],
        }),
    }
}

// Find and validate a literal attribute
pub fn find_literal_attr(
    attrs: &[SpannedAttribute],
    name: &str,
    span: &Span,
) -> Result<(String, Span), Error> {
    let attr = attrs
        .iter()
        .find(|attr| attr.name == name)
        .ok_or_else(|| Error {
            message: format!("Missing '{}' attribute", name),
            main_span: *span,
            labels: vec![(*span, "Missing attribute".to_string())],
        })?;

    match &attr.value {
        AttrValue::Template(segments) if segments.len() == 1 => match &segments[0] {
            AttrValueTemplateSegment::Literal(s) => Ok((s.clone(), attr.span)),
            _ => Err(Error {
                message: format!("'{}' attribute must be a literal string", name),
                main_span: attr.span,
                labels: vec![(attr.span, "Attribute must be a literal string".to_string())],
            }),
        },
        AttrValue::Template(_) => Err(Error {
            message: format!("'{}' attribute must be a simple literal string", name),
            main_span: attr.span,
            labels: vec![(
                attr.span,
                "Attribute must be a simple literal string".to_string(),
            )],
        }),
        AttrValue::Expr(_) => Err(Error {
            message: format!(
                "'{}' attribute must be a literal string, not a binding",
                name
            ),
            main_span: attr.span,
            labels: vec![(
                attr.span,
                "Attribute must be a literal string, not a binding".to_string(),
            )],
        }),
    }
}

// Infer attribute type based on tag and attribute name
pub fn infer_attr_type(attr: &str, tag: &str) -> String {
    attribute_type(tag, attr).unwrap_or_else(|| "string".to_string())
}

// Expect node to be a specific element type, return Error if not
pub fn expect_element<'a>(
    node: &'a Node,
    expected_name: &str,
) -> Result<(&'a [SpannedAttribute], &'a [Node], &'a Span), Error> {
    if let Node::Element {
        name,
        attrs,
        children,
        span,
        ..
    } = node
    {
        if name == expected_name {
            Ok((attrs, children, span))
        } else {
            Err(Error {
                message: format!("Expected '{}' element, found '{}'", expected_name, name),
                main_span: *span,
                labels: vec![(*span, format!("Found '{}' instead", name))],
            })
        }
    } else {
        Err(Error {
            message: format!(
                "Expected '{}' element, found non-element node",
                expected_name
            ),
            main_span: *node.span(),
            labels: vec![(*node.span(), "Not an element".to_string())],
        })
    }
}

// Check if node matches element name (simple boolean check)
pub fn match_element_name(node: &Node, name: &str) -> bool {
    matches!(node, Node::Element { name: n, .. } if n == name)
}

// Collect all expression dependencies from an attribute value
pub fn collect_attr_dependencies(value: &AttrValue) -> Vec<String> {
    use crate::expr::expr_dependencies;

    match value {
        AttrValue::Template(segments) => {
            let mut deps = Vec::new();
            for seg in segments {
                if let AttrValueTemplateSegment::Expr(expr) = seg {
                    deps.extend(expr_dependencies(expr));
                }
            }
            deps
        }
        AttrValue::Expr(expr) => expr_dependencies(expr).into_iter().collect(),
    }
}

// Check if an attribute value contains any bindings (not purely static)
pub fn has_bindings(value: &AttrValue) -> bool {
    match value {
        AttrValue::Template(segments) => segments
            .iter()
            .any(|seg| matches!(seg, AttrValueTemplateSegment::Expr(_))),
        AttrValue::Expr(_) => true,
    }
}

// Find all children matching a specific element name
pub fn find_children_by_name<'a>(children: &'a [Node], name: &str) -> Vec<&'a Node> {
    children
        .iter()
        .filter(|child| match_element_name(child, name))
        .collect()
}

// Find a unique optional child by name, returning error if duplicates exist
pub fn find_unique_child_by_name<'a>(
    children: &'a [Node],
    name: &str,
    parent_span: &Span,
) -> Result<Option<&'a Node>, Error> {
    let matches = find_children_by_name(children, name);

    if matches.len() > 1 {
        Err(Error {
            message: format!("Multiple '{}' elements found; only one allowed", name),
            main_span: *parent_span,
            labels: vec![(*parent_span, format!("Multiple '{}' elements", name))],
        })
    } else {
        Ok(matches.first().copied())
    }
}

// Extract dataset key from data- attribute (returns None if not a data attribute)
pub fn split_data_attribute(attr_name: &str) -> Option<String> {
    attr_name.strip_prefix("data-").map(|key| key.to_string())
}
