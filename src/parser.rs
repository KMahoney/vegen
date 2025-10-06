use crate::ast::{
    AttrValue, AttrValueTemplateSegment, Node, SourceId, Span, SpannedAttribute, SpannedBinding,
};
use crate::error::Error;
use crate::expr::{self};
use chumsky::prelude::*;
use chumsky::span::SimpleSpan;

pub type ParseError = Error;

pub fn parse_template(input: &str, source: SourceId) -> Result<Vec<Node>, Vec<ParseError>> {
    let parser = template_parser(source);

    match parser.parse(input).into_result() {
        Ok(nodes) => Ok(nodes),
        Err(errors) => Err(errors
            .into_iter()
            .map(|error| {
                let span = Span::new(source, error.span().start..error.span().end);
                ParseError {
                    message: format!("{}", error.reason()),
                    main_span: span,
                    labels: vec![(span, "Parse error here".to_string())],
                }
            })
            .collect()),
    }
}

type Extra<'a> = extra::Err<Rich<'a, char>>;

fn template_parser<'a>(source: SourceId) -> impl Parser<'a, &'a str, Vec<Node>, Extra<'a>> {
    recursive(move |node_parser| {
        // Helper to create a span with source context
        fn sourced_span(source: SourceId, span: SimpleSpan) -> Span {
            SimpleSpan {
                start: span.start,
                end: span.end,
                context: source,
            }
        }

        // Identifier with span parser
        let identifier_with_span = any()
            .filter(|c: &char| c.is_ascii_alphabetic() || *c == '_')
            .then(
                any()
                    .filter(|c: &char| c.is_ascii_alphanumeric() || *c == '_' || *c == '-')
                    .repeated()
                    .collect::<Vec<char>>(),
            )
            .map_with(move |(first, rest), e| {
                let mut result = String::new();
                result.push(first);
                result.extend(rest);
                (result, sourced_span(source, e.span()))
            })
            .padded()
            .labelled("identifier")
            .boxed();

        // Expression parser for bindings
        let expr_parser = expr::expr_parser(source);

        // Binding parser - parse full expressions inside { ... }
        let binding_parser = just('{')
            .ignore_then(expr_parser)
            .then_ignore(just('}'))
            .map_with(move |expr, e| SpannedBinding {
                expr,
                span: sourced_span(source, e.span()),
            })
            .labelled("binding")
            .boxed();

        // Quoted template parser
        let quoted_template_parser = just('"')
            .ignore_then(
                choice((
                    // Binding segment: {variable} (full binding parser, may include pipelines)
                    binding_parser
                        .clone()
                        .map(AttrValueTemplateSegment::Binding),
                    // Literal segment: any text except { and "
                    none_of("{\"")
                        .repeated()
                        .at_least(1)
                        .to_slice()
                        .map(|s: &str| AttrValueTemplateSegment::Literal(s.to_string())),
                ))
                .repeated()
                .collect(),
            )
            .then_ignore(just('"'))
            .map(AttrValue::Template)
            .labelled("string")
            .boxed();

        // Unquoted binding parser
        let unquoted_binding_parser = binding_parser.clone().map(AttrValue::Binding).boxed();

        // Attribute value parser
        let attr_value_parser = choice((
            // Quoted template: "hello {world}"
            quoted_template_parser,
            // Unquoted binding: {binding}
            unquoted_binding_parser,
        ))
        .boxed();

        // Attribute parser
        let attribute_parser = identifier_with_span
            .clone()
            .then_ignore(just('=').padded())
            .then(attr_value_parser)
            .map_with(move |((name, name_span), value), e| SpannedAttribute {
                name,
                name_span,
                value,
                span: sourced_span(source, e.span()),
            })
            .padded()
            .boxed();

        // Attributes parser
        let attributes_parser = attribute_parser.repeated().collect().padded().boxed();

        // Element parser
        let element_parser = just('<')
            .ignore_then(identifier_with_span.clone())
            .then(attributes_parser)
            .then(choice((
                // Self-closing tag: />
                just("/>").to(None),
                // Opening tag with children: >...content...</tag>
                just('>')
                    .ignore_then(
                        node_parser
                            .repeated()
                            .collect::<Vec<Option<Node>>>()
                            .map(|v| v.into_iter().flatten().collect::<Vec<Node>>())
                            .then_ignore(just("</"))
                            .then(identifier_with_span.clone())
                            .then_ignore(just('>')),
                    )
                    .map(Some),
            )))
            .map_with(move |(((name, name_span), attrs), children_option), e| {
                let span = sourced_span(source, e.span());
                (name, name_span, attrs, children_option, span)
            })
            .validate(
                |(name, name_span, attrs, children_option, span), _, emitter| {
                    let children = if let Some((c, (cn, cn_span))) = children_option {
                        if cn != name {
                            emitter.emit(Rich::custom(
                                SimpleSpan::from(cn_span.start..cn_span.end),
                                format!(
                                    "Closing tag '{}' does not match opening tag '{}'",
                                    cn, name
                                ),
                            ));
                        }
                        c
                    } else {
                        vec![]
                    };

                    Node::Element {
                        name,
                        name_span,
                        attrs,
                        children,
                        span,
                    }
                },
            )
            .padded()
            .labelled("XML element")
            .boxed();

        // Binding node parser
        let binding_node_parser = binding_parser.map(Node::Binding).boxed();

        // Text node parser - preserve whitespace
        let text_node_parser = none_of("<{")
            .repeated()
            .at_least(1)
            .to_slice()
            .map_with(move |text: &str, e| {
                if text.is_empty() {
                    None
                } else {
                    Some(Node::Text {
                        content: text.to_string(),
                        span: sourced_span(source, e.span()),
                    })
                }
            })
            .boxed();

        // Comment parser
        let comment_parser = just("<!--")
            .then(
                choice((
                    none_of("-").to(()),
                    just("-").then(none_of("-")).to(()),
                    just("--").then(none_of(">")).to(()),
                ))
                .repeated(),
            )
            .then_ignore(just("-->"))
            .to(None)
            .boxed();

        choice((
            element_parser.map(Some),
            binding_node_parser.map(Some),
            text_node_parser,
            comment_parser,
        ))
    })
    .repeated()
    .collect()
    .map(|v: Vec<Option<Node>>| v.into_iter().flatten().collect())
    .then_ignore(end())
    .padded()
    .boxed()
}
