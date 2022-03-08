use diagnostics::result::Result;
use std::path::PathBuf;

use parser::ParserDatabase;
use vfs::FileSystemDatabase;

use syntax::ast::Const;
use syntax::visit::Visitor;

use codegen::Codegen;

use log::debug;

#[derive(Default)]
struct ConstEvaluationStep {}

// enum Value {
//     Number(f64),
//     Boolean(bool),
// }

impl Visitor for ConstEvaluationStep {
    fn visit_const(&mut self, const_: &mut std::sync::Arc<Const>) -> Result<()> {
        debug!("evaluate: {:#?}", const_);
        // let value = evaluate_numeric_expression(&const_.value);
        // debug!("value: {:#?}", value);
        Ok(())
    }
}

#[derive(Default)]
struct ViewCompiler {}

impl Visitor for ViewCompiler {}

// fn evaluate_numeric_expression(expression: &Expression) -> Value {
//     match &expression.kind {
//         syntax::ast::ExpressionKind::Number { raw, .. } => (*raw).into(),
//         syntax::ast::ExpressionKind::Binary { left, right, op } => {
//             let left: f64 = evaluate_numeric_expression(&*left);
//             let right: f64 = evaluate_numeric_expression(&*right);
//             match op {
//                 syntax::ast::BinOp::Add => Value::Number(left + right),
//                 syntax::ast::BinOp::Sub => Value::Number(left - right),
//                 syntax::ast::BinOp::Sum => Value::Number(left + right),
//                 syntax::ast::BinOp::Mul => Value::Number(left * right),
//                 syntax::ast::BinOp::Div => Value::Number(left / right),
//                 syntax::ast::BinOp::Mod => Value::Number(left % right),
//                 syntax::ast::BinOp::LessThan => Value::Boolean(left < right),
//                 syntax::ast::BinOp::GreaterThan => Value::Boolean(left > right),
//                 // syntax::ast::BinOp::BinAdd => left & right,
//                 // syntax::ast::BinOp::BinOr => left | right,
//                 // syntax::ast::BinOp::GreaterThan => {}
//                 // syntax::ast::BinOp::Pipeline => {}
//                 // syntax::ast::BinOp::BinOr => {}
//                 // syntax::ast::BinOp::BinAdd => {}
//                 _ => panic!("unsupported"),
//             }
//         }
//         _ => panic!("unsupported"), // syntax::ast::ExpressionKind::String { raw } => {}
//                                     // syntax::ast::ExpressionKind::Boolean() => {}
//     }
// }

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
    Codegen::default().visit_module(&mut ast)?;
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
