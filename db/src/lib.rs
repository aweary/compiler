use diagnostics::result::Result;
use std::path::PathBuf;

use parser::ParserDatabase;
use vfs::FileSystemDatabase;

use syntax::ast::{Const, Expression};
use syntax::visit::Visitor;

use log::debug;

#[derive(Default)]
struct ConstEvaluationStep {}

impl Visitor for ConstEvaluationStep {
    fn visit_const(&mut self, const_: &mut Const) -> Result<()> {
        debug!("evaluate: {:#?}", const_);
        let value = evaluate_numeric_expression(&const_.value);
        debug!("value: {:#?}", value);
        Ok(())
    }
}

fn evaluate_numeric_expression(expression: &Expression) -> f64 {
    match &expression.kind {
        syntax::ast::ExpressionKind::Number { raw, .. } => {
            (*raw).into()
        },
        syntax::ast::ExpressionKind::Binary { left, right, op } => {
            let left: f64 = evaluate_numeric_expression(&*left);
            let right: f64 = evaluate_numeric_expression(&*right);
            match op {
                syntax::ast::BinOp::Add => left + right,
                syntax::ast::BinOp::Sub => left - right,
                syntax::ast::BinOp::Sum => left + right,
                syntax::ast::BinOp::Mul => left * right,
                syntax::ast::BinOp::Div => left / right,
                syntax::ast::BinOp::Mod => left % right,
                // syntax::ast::BinOp::BinAdd => left & right,
                // syntax::ast::BinOp::BinOr => left | right,
                // syntax::ast::BinOp::GreaterThan => {}
                // syntax::ast::BinOp::LessThan => {}
                // syntax::ast::BinOp::Pipeline => {}
                // syntax::ast::BinOp::BinOr => {}
                // syntax::ast::BinOp::BinAdd => {}
                _ => panic!("unsupported"),
            }
        }
        _ => panic!("unsupported"), // syntax::ast::ExpressionKind::String { raw } => {}
                                    // syntax::ast::ExpressionKind::Boolean() => {}
    }
}

///////////////

// Re-export traits
pub use parser::Parser;
pub use vfs::{FileId, FileSystem, Files};

#[salsa::query_group(CompilerDatabase)]
pub trait Compiler: Parser + FileSystem {
    fn compile(&self, path: PathBuf) -> Result<Vec<usize>>;
}

fn compile(db: &dyn Compiler, path: PathBuf) -> Result<Vec<usize>> {
    let mut ast = db.parse(path)?;
    // Evaluate constants
    ConstEvaluationStep::default().visit_module(&mut ast)?;
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
