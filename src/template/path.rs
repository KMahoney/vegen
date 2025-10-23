use path_clean::PathClean;
use std::path::PathBuf;

/// Collapse `.`/`..` segments without touching the filesystem. We use
/// `path_clean` instead of `std::fs::canonicalize` so we can handle missing
/// templates and avoid resolving symlinks.
pub fn normalize_path(original: PathBuf) -> PathBuf {
    original.clean()
}
