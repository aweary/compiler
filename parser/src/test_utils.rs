use std::ops::ControlFlow;

use crate::parser::{constrct_cfg_from_block, ParserImpl};
use common::control_flow_graph::ControlFlowGraph;
use diagnostics::result::Result;
use syntax::{
    arena::{AstArena, FunctionId, StatementId},
    ast::*,
    visit::Visitor,
    Precedence, Span, Token, TokenKind,
};

pub fn parse_cfg_from_statements(stmts: &str) -> String {
    let source = format!("fn test() {{ {} }}", stmts);
    let mut ast_arena = AstArena::default();
    let parser = ParserImpl::new(&source, &mut ast_arena);
    let mut ast = parser.parse_module().unwrap();

    struct CFGVisitor<'a> {
        ast_arena: &'a AstArena,
        cfg: Option<ControlFlowGraph<u32>>,
    }

    impl<'a> Visitor for CFGVisitor<'a> {
        fn visit_function(&mut self, function_id: &mut FunctionId) -> Result<()> {
            let function = self.ast_arena.functions.get(*function_id).unwrap();
            let body = function.body.as_ref().unwrap();
            let block_cfg = constrct_cfg_from_block(body, self.ast_arena);
            self.cfg = Some(block_cfg);
            Ok(())
        }
    }

    let mut visitor = CFGVisitor {
        ast_arena: &ast_arena,
        cfg: None,
    };

    visitor.visit_module(&mut ast).unwrap();

    visitor.cfg.unwrap().format()
}
