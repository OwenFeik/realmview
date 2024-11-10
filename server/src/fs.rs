use std::path::{Path, PathBuf};

use once_cell::sync::Lazy;

// TODO tempfile for tests
static DATA: Lazy<PathBuf> =
    Lazy::new(|| PathBuf::from(std::env::args().nth(1).expect(super::USAGE)));
pub static CONTENT: Lazy<PathBuf> = Lazy::new(|| DATA.join("content"));
pub static SAVES: Lazy<PathBuf> = Lazy::new(|| DATA.join("saves"));

/// Join a path with a relative path, that may start with a slash. If the
/// second argument starts with a slash, all leading slashes will be removed
/// before joining.
pub fn join_relative_path<S: AsRef<str>>(left: &Path, right: S) -> PathBuf {
    left.join(right.as_ref().trim_start_matches('/'))
}
