pub fn is_ts_identifier(s: &str) -> bool {
    let mut chars = s.chars();
    match chars.next() {
        Some(c) if c == '_' || c == '$' || c.is_ascii_alphabetic() => (),
        _ => return false,
    }
    chars.all(|c| c == '_' || c == '$' || c.is_ascii_alphanumeric())
}

pub fn render_key(k: &str) -> String {
    if is_ts_identifier(k) {
        k.to_string()
    } else {
        format!("\"{}\"", k)
    }
}
