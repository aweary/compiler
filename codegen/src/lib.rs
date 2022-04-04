mod call_graph;

use common::control_flow_graph::{BlockIndex, ControlFlowGraph};
use diagnostics::result::Result;
use evaluate::Value;
use syntax::ast_::*;

type AstControlFlowGraph = ControlFlowGraph<StatementId, ExpressionId, Value>;

pub fn codegen_from_cfg(cfg: &AstControlFlowGraph) -> Result<String> {
    use petgraph::algo::dominators::simple_fast;
    use petgraph::algo::kosaraju_scc;

    // let dominators = simple_fast(&cfg.graph, graph.entry_index().into());
    // let ssc = kosaraju_scc(&cfg.graph);

    let entry_index = cfg.entry_index();
    let entry_node = &cfg.graph[entry_index.0];

    println!("entry, {:?}", entry_node);

    // graph.print();
    // println!("{:#?}", dominators);
    // println!("{:#?}", ssc);
    Ok(String::new())
}
