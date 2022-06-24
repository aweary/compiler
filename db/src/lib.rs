use diagnostics::result::Result;
use std::path::PathBuf;

use parser::parser_::ParserDatabase;
use vfs::FileSystemDatabase;

///////////////

// Re-export traits
pub use parser::parser_::Parser;
pub use vfs::{FileId, FileSystem, Files};

#[salsa::query_group(CompilerDatabase)]
pub trait Compiler: Parser + FileSystem {
    fn compile(&self, path: PathBuf) -> Result<Vec<usize>>;
}

fn compile(db: &dyn Compiler, path: PathBuf) -> Result<Vec<usize>> {
    let ast = db.parse(path)?;
    Ok(vec![])
}

#[salsa::database(FileSystemDatabase, CompilerDatabase, ParserDatabase)]
pub struct Database {
    storage: salsa::Storage<Self>,
}

impl Default for Database {
    fn default() -> Self {
        let storage = salsa::Storage::default();
        Database { storage }
    }
}

impl salsa::Database for Database {}

impl salsa::ParallelDatabase for Database {
    fn snapshot(&self) -> salsa::Snapshot<Self> {
        salsa::Snapshot::new(Database {
            storage: self.storage.snapshot(),
        })
    }
}
