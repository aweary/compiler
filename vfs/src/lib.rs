use std::path::PathBuf;
use std::sync::Arc;
pub use codespan_reporting::files::{Files};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(usize);

#[salsa::query_group(FileSystemDatabase)]
pub trait FileSystem {
    #[salsa::input]
    fn file_text(&self, path: PathBuf) -> Arc<String>;
}

