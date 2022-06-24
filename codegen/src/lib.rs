mod call_graph;

use std::{collections::HashMap, vec};

use common::control_flow_graph::{BlockIndex, ControlFlowEdge, ControlFlowGraph, ControlFlowNode};
use diagnostics::result::Result;
use evaluate::Value;
use petgraph::{algo::dominators, graph::NodeIndex};
use syntax::ast_::*;

type AstControlFlowGraph = ControlFlowGraph<StatementId, ExpressionId, Value>;

struct CodegenBranch {}

type CodegenBranchMap = HashMap<NodeIndex, CodegenBranch>;

pub fn codegen_from_cfg(cfg: &AstControlFlowGraph, arena: &mut AstArena) -> Result<String> {
    use petgraph::visit::depth_first_search;
    use petgraph::visit::{Control, DfsEvent};

    cfg.print();

    let mut code = vec![];

    let start = cfg.first_index().expect("first").0;

    let mut branch_map: CodegenBranchMap = HashMap::default();

    depth_first_search(&cfg.graph, Some(start), |event| {
        match event {
            DfsEvent::Discover(node_index, _) => {
                println!("Discover: {:?}", node_index);
                match cfg.graph.node_weight(node_index).unwrap() {
                    ControlFlowNode::BranchCondition(value) => {
                        // We've encountered a new branch! Add it to the map so the conditional
                        // edges can reference it
                        let branch = CodegenBranch {};

                        branch_map.insert(node_index, branch);

                        let expression = arena.expressions.get(*value).unwrap().borrow();
                        let expression_codegen = codegen_expression(&expression).unwrap();
                        let codegen_branch = format!("if ({})", expression_codegen);
                        code.push(codegen_branch);
                    }
                    ControlFlowNode::LoopCondition(_) => {
                        code.push("while ($cond) ".to_string());
                    }
                    ControlFlowNode::BasicBlock(block) => {
                        for statement_id in &block.statements {
                            let codegened_statement =
                                codegen_statement(*statement_id, arena).unwrap();
                            code.push(codegened_statement)
                            // ...
                        }
                    }
                    ControlFlowNode::Exit => {
                        // Nothing
                    }
                    ControlFlowNode::Entry => {
                        // Nothing
                    }
                }
            }
            DfsEvent::TreeEdge(u, v) => {
                let edge_index = cfg.graph.find_edge(u, v).unwrap();
                let weight = cfg.graph.edge_weight(edge_index).unwrap();

                match weight {
                    ControlFlowEdge::ConditionTrue => {
                        // code.push("if (true) {".to_string());
                    }
                    ControlFlowEdge::ConditionFalse => {
                        // code.push("if (false) {".to_string());
                    }
                    ControlFlowEdge::Return => {
                        // code.push("return;".to_string());
                    }
                    ControlFlowEdge::Normal => {
                        // code.push("{".to_string());
                    }
                }

                println!("\nTreeEdge: {:?} -> {:?}", u, v);
                println!("Edge: {:?}", edge_index);
                println!("Weight: {:?}\n", weight);
            }
            DfsEvent::BackEdge(u, v) => {
                println!("BackEdge: {:?} -> {:?}", u, v);
            }
            DfsEvent::CrossForwardEdge(u, v) => {
                println!("CrossForwardEdge: {:?} -> {:?}", u, v);
            }
            DfsEvent::Finish(u, _) => {
                println!("Finish: {:?}", u);
            }
        }

        if let DfsEvent::TreeEdge(_, v) = event {
            // Just fixing the types
            if false {
                return Control::Break(v);
            }
        }

        Control::Continue
    });

    println!("{:?}", code);

    Ok(String::new())
}

fn codegen_statement(statement: StatementId, arena: &mut AstArena) -> Result<String> {
    let statement = arena.statements.get(statement).unwrap();
    match statement {
        Statement::Let { name, value } => {
            let expression = arena.expressions.get(*value).unwrap().borrow();
            let value = codegen_expression(&expression)?;
            Ok(format!("let {} = {};", name.symbol, value))
        }
        Statement::Return(value) => {
            let expression = arena.expressions.get(*value).unwrap().borrow();
            let value = codegen_expression(&expression)?;
            Ok(format!("return {};", value))
        }
        Statement::Expression(_) => todo!(),
        Statement::If(_) => todo!(),
        Statement::While { condition, body } => todo!(),
    }
}

fn codegen_expression(expression: &Expression) -> Result<String> {
    match expression {
        Expression::Number(value) => Ok(format!("{}", value)),
        _ => Ok(String::from("$value")),
        // Expression::Binary { left, right, op } => todo!(),
        // Expression::Boolean(_) => todo!(),
        // Expression::String(_) => todo!(),
        // Expression::Reference(_) => todo!(),
        // Expression::Call { callee, arguments } => todo!(),
        // Expression::If {
        //     condition,
        //     then_branch,
        //     else_branch,
        // } => todo!(),
    }
}
