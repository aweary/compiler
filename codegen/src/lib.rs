mod call_graph;

use diagnostics::result::Result;
use syntax::ast_::*;
use common::control_flow_graph::ControlFlowGraph;

type AstControlFlowGraph = ControlFlowGraph<StatementId, ExpressionId>;

pub fn codegen_from_cfg(graph: &AstControlFlowGraph) -> Result<String> {
    println!("codgen from cfg");
    Ok(String::new())
}
