use std::fmt;

use crate::ast::Span;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub message: String,
    pub main_span: Span,
    pub labels: Vec<(Span, String)>,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at {:?}", self.message, self.main_span)
    }
}
