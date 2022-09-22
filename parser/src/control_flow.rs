use diagnostics::result::Result;
use evaluate::Value;
use log::debug;
use std::cell::RefCell;
use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
};
use syntax::ast_::*;
use syntax::visit_::{walk_component, walk_function, Visitor};

use common::control_flow_graph::{
    BasicBlock, BlockIndex, ControlFlowEdge, ControlFlowGraph, ControlFlowMap, ControlFlowMapKey,
};

use crate::evaluate::{evaluate_expression, CallContext};

pub struct ControlFlowAnalysis<'a, T, E, V> {
    ast: &'a mut AstArena,
    cfg_map: RefCell<ControlFlowMap<FunctionId, ComponentId, T, E, V>>,
}

impl<'a, T, E, V> ControlFlowAnalysis<'a, T, E, V> {
    pub fn new(ast: &'a mut AstArena) -> Self {
        Self {
            ast,
            cfg_map: RefCell::new(HashMap::default()),
        }
    }

    pub fn finish(
        self,
    ) -> HashMap<ControlFlowMapKey<FunctionId, ComponentId>, ControlFlowGraph<T, E, V>> {
        self.cfg_map.into_inner()
    }
}

impl<'a> Visitor for ControlFlowAnalysis<'a, StatementId, ExpressionId, evaluate::Value> {
    fn context_mut(&mut self) -> &mut AstArena {
        &mut self.ast
    }

    fn context(&self) -> &AstArena {
        &self.ast
    }

    fn visit_function(&self, function_id: FunctionId) -> Result<()> {
        println!("Visiting function {:?}", function_id);
        let arena = self.context();
        let function = arena.functions.get(function_id).unwrap();
        let function = function.borrow();
        let body = arena.blocks.get(function.body.unwrap()).unwrap();
        let cfg = constrct_cfg_from_block(body, arena, None);
        self.cfg_map
            .borrow_mut()
            .insert(ControlFlowMapKey::Function(function_id), cfg);
        walk_function(self, function_id)
    }

    fn visit_component(&self, component_id: ComponentId) -> Result<()> {
        println!("Visiting component {:?}", component_id);
        let arena = self.context();
        let component = arena.components.get(component_id).unwrap();
        let component = component.borrow();
        let body = arena.blocks.get(component.body.unwrap()).unwrap();
        let cfg = constrct_cfg_from_block(body, arena, None);
        // cfg.print();
        self.cfg_map
            .borrow_mut()
            .insert(ControlFlowMapKey::Component(component_id), cfg);
        walk_component(self, component_id)
    }
}

pub fn constrct_cfg_from_block(
    block: &Block,
    ast: &AstArena,
    call_context: Option<&CallContext>,
) -> ControlFlowGraph<StatementId, ExpressionId, evaluate::Value> {
    debug!("constrct_cfg_from_block:start");
    let mut loop_indicies = HashSet::<BlockIndex>::default();

    let mut cfg = ControlFlowGraph::default();
    let mut basic_block = BasicBlock::new();
    for statement_id in &block.statements {
        let statement = ast.statements.get(*statement_id).unwrap();

        match statement {
            Statement::Let { .. }
            | Statement::State { .. }
            | Statement::Expression(_)
            | Statement::Assignment { .. } => {
                basic_block.statements.push(*statement_id);
            }
            Statement::Return(expression_id) => {
                let value_expr = ast.expressions.get(*expression_id).unwrap();
                let value_expr = value_expr.borrow();
                let value = evaluate_expression(ast, &value_expr, call_context);
                if cfg.value.is_none() {
                    cfg.value = value;
                }
                cfg.set_has_early_return(true);
                basic_block.statements.push(*statement_id);
                let block_index = cfg.add_block(basic_block);
                cfg.add_edge_to_exit(block_index, ControlFlowEdge::Return);
                basic_block = BasicBlock::new();
            }
            Statement::If(if_) => {
                if !basic_block.is_empty() {
                    cfg.add_block(basic_block);
                    basic_block = BasicBlock::new();
                }
                // The edge queue here should be flushed to the NEW entry node
                // for the consumed if statement.
                debug!("edge_queue before if: {:?}", cfg.edge_queue);
                debug!("last_index before if: {:?}", cfg.last_index());

                let if_cfg = construct_cfg_from_if(if_, ast, call_context);

                let if_cfg_has_early_return = if_cfg.has_early_return();

                if if_cfg_has_early_return {
                    cfg.set_has_early_return(true);
                }
                cfg.consume_subgraph(if_cfg, None, cfg.last_index(), true);
            }
            Statement::While { condition, body } => {
                if !basic_block.is_empty() {
                    cfg.add_block(basic_block);
                    basic_block = BasicBlock::new();
                }

                let last_index = cfg.last_index();
                let loop_condition_index = cfg.add_loop_condition(*condition);
                cfg.add_edge(last_index, loop_condition_index, ControlFlowEdge::Normal);

                let body = ast.blocks.get(*body).unwrap();
                let mut while_body_cfg = constrct_cfg_from_block(body, ast, call_context);
                let while_body_has_early_return = while_body_cfg.has_early_return();

                // Delete the normal flow edge from the last block to the exit node
                while_body_cfg
                    .delete_normal_edge(while_body_cfg.last_index(), while_body_cfg.exit_index());

                let true_edge = ControlFlowEdge::ConditionTrue;
                let false_edge = ControlFlowEdge::ConditionFalse;

                cfg.consume_subgraph(while_body_cfg, Some(true_edge), loop_condition_index, true);

                loop_indicies.insert(cfg.last_index());

                if !while_body_has_early_return {
                    cfg.add_edge(
                        cfg.last_index(),
                        loop_condition_index,
                        ControlFlowEdge::Normal,
                    );
                }

                cfg.enqueue_edge(loop_condition_index, false_edge);
            }
        }
    }

    if !basic_block.is_empty() {
        cfg.add_block(basic_block);
    }

    // Don't automatically add an edge to the exit node if there is an early return
    // in this block, or the last block is a loop
    if !cfg.has_early_return() && !loop_indicies.contains(&cfg.last_index()) {
        cfg.add_edge(cfg.last_index(), cfg.exit_index(), ControlFlowEdge::Normal);
    }

    cfg.flush_edge_queue(cfg.exit_index());

    debug!("constrct_cfg_from_block:end\n");
    cfg
}

pub fn construct_cfg_from_if(
    if_: &If,
    ast: &AstArena,
    call_context: Option<&CallContext>,
) -> ControlFlowGraph<StatementId, ExpressionId, Value> {
    debug!("construct_cfg_from_if:start");

    let condition = ast.expressions.get(if_.condition).unwrap();
    let condition = condition.borrow();

    if let Some(value) = evaluate_expression(ast, &*condition, call_context) {
        if let Value::Boolean(should_run_branch) = value {
            if should_run_branch {
                let body = ast.blocks.get(if_.body).unwrap();
                return constrct_cfg_from_block(body, ast, call_context);
            } else if let Some(else_) = &if_.alternate {
                match &**else_ {
                    Else::If(if_) => return construct_cfg_from_if(if_, ast, call_context),
                    Else::Block(block_id) => {
                        let block = ast.blocks.get(*block_id).unwrap();
                        return constrct_cfg_from_block(block, ast, call_context);
                    }
                }
            } else {
                return ControlFlowGraph::default();
            }
        }
    }

    let mut cfg = ControlFlowGraph::default();

    let branch_condition_index = cfg.add_branch_condition(if_.condition);

    let true_edge = ControlFlowEdge::ConditionTrue;
    let false_edge = ControlFlowEdge::ConditionFalse;

    let If {
        body, alternate, ..
    } = if_;

    let body = ast.blocks.get(*body).unwrap();

    // The block for the `true` branch of the if statement
    let if_true_cfg = constrct_cfg_from_block(body, ast, call_context);

    // Whether the `true` branch of the if statement has an early return
    let if_true_cfg_has_early_return = if_true_cfg.has_early_return();

    cfg.consume_subgraph(if_true_cfg, Some(true_edge), branch_condition_index, false);

    if let Some(ref else_) = alternate {
        match else_.deref() {
            Else::Block(else_block_id) => {
                let else_block = ast.blocks.get(*else_block_id).unwrap();
                let else_cfg = constrct_cfg_from_block(else_block, ast, call_context);
                let else_cfg_has_early_return = else_cfg.has_early_return();

                cfg.consume_subgraph(else_cfg, Some(false_edge), branch_condition_index, false);

                if if_true_cfg_has_early_return && else_cfg_has_early_return {
                    // Both the `true` and `false` branches of the if statement have an early return.
                    // So we know this block also has an early return
                    cfg.set_has_early_return(true)
                }
            }
            Else::If(_if) => {
                let else_if_cfg = construct_cfg_from_if(_if, ast, call_context);
                let else_if_cfg_has_early_return = else_if_cfg.has_early_return();

                // cfg.add_edge(last_index, branch_condition_index, false_edge);

                if if_true_cfg_has_early_return && else_if_cfg_has_early_return {
                    cfg.set_has_early_return(true)
                }

                cfg.consume_subgraph(else_if_cfg, Some(false_edge), branch_condition_index, false);
            }
        }
    } else {
        // Assuming we have no `else` chains, the `false` edge should point to the *next* block.
        cfg.add_edge(branch_condition_index, cfg.exit_index(), false_edge)
    }

    if !cfg.has_early_return() {
        cfg.flush_edge_queue(cfg.exit_index());
    }
    debug!("construct_cfg_from_if:end\n");
    cfg
}
