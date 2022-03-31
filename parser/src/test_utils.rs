use crate::control_flow::constrct_cfg_from_block;
use crate::parser_::ParserImpl;
use common::control_flow_graph::ControlFlowGraph;
use diagnostics::result::Result;

use std::cell::{RefCell};
use syntax::ast_::*;
use syntax::visit_::Visitor;

pub fn parse_cfg_from_statements(stmts: &str) -> String {
    let source = format!("fn test() {{ {} }}", stmts);
    let mut ast_arena = AstArena::default();
    let mut parser = ParserImpl::new(&source, &mut ast_arena);
    let ast = parser.parse_module().unwrap();

    struct CFGVisitor<'a> {
        ast_arena: &'a mut AstArena,
        cfg: RefCell<Option<ControlFlowGraph<StatementId, ExpressionId>>>,
    }

    impl<'a> Visitor for CFGVisitor<'a> {
        fn visit_function(&self, function_id: FunctionId) -> Result<()> {
            let arena = self.context();
            let function = arena.functions.get(function_id).unwrap();
            let function = function.borrow();
            let body = arena.blocks.get(function.body).unwrap();
            let cfg = constrct_cfg_from_block(body, arena);
            let mut cfg_cell = self.cfg.borrow_mut();
            *cfg_cell = Some(cfg);
            Ok(())
        }

        fn context_mut(&mut self) -> &mut AstArena {
            &mut self.ast_arena
        }

        fn context(&self) -> &AstArena {
            self.ast_arena
        }
    }

    let visitor = CFGVisitor {
        ast_arena: &mut ast_arena,
        cfg: RefCell::new(None),
    };

    visitor.visit_module(ast).unwrap();
    let formatted = {
        let cfg = visitor.cfg.borrow();
        let cfg = cfg.as_ref().unwrap();
        cfg.format()
    };
    formatted
}
