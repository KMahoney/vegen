use once_cell::sync::Lazy;
use std::collections::HashMap;

/// Static parsed mapping: tag -> (attr -> type)
static ATTRIBUTE_TYPES: Lazy<HashMap<String, HashMap<String, String>>> = Lazy::new(|| {
    // attribute_types.json is located in the same directory as this file (src/)
    let s = include_str!("attribute_types.json");
    serde_json::from_str(s).expect("Failed to parse attribute_types.json")
});

/// Return the attribute type string for a given tag and attribute if known.
///
/// Lookup strategy:
/// - Lowercase the tag and find its attribute map.
/// - Try exact `attr` key first.
/// - If not found, try lowercased `attr`.
pub fn attribute_type(tag: &str, attr: &str) -> Option<String> {
    let tag_key = tag.to_ascii_lowercase();
    let attrs = ATTRIBUTE_TYPES.get(&tag_key)?;
    // try exact attr first (preserve case if callers passed exact)
    if let Some(ty) = attrs.get(attr) {
        return Some(ty.clone());
    }
    // fallback: try lowercased attr
    attrs.get(&attr.to_ascii_lowercase()).cloned()
}
