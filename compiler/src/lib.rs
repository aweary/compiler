use db::*;

pub struct Compiler {
    pub db: Database
}

impl Compiler {
    pub fn new() -> Self {
        let db = Database::default();
        Compiler { db }
    }
}