mod call_graph;

use diagnostics::result::Result;
use syntax::ast::Module;
use syntax::visit::Visitor;

#[derive(Default)]
pub struct Codegen {}

impl Visitor for Codegen {
    fn visit_module(&mut self, module: &mut Module) -> Result<()> {
        let Module { .. } = module;
        // Start parsing the module from the `main` function.
        Ok(())
    }
}
