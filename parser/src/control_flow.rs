use diagnostics::result::Result;
use std::{
    collections::{HashMap, HashSet},
    ops::Deref,
};
use syntax::{
    arena::{AstArena, FunctionId},
    ast::*,
    visit::Visitor,
};

use common::control_flow_graph::{BasicBlock, BlockIndex, ControlFlowEdge, ControlFlowGraph};

pub struct ControlFlowAnalysis<'a> {
    ast: &'a mut AstArena,
    cfg_map: HashMap<FunctionId, ControlFlowGraph<u32>>,
}

impl<'a> ControlFlowAnalysis<'a> {
    pub fn new(ast: &'a mut AstArena) -> Self {
        Self {
            ast,
            cfg_map: HashMap::default(),
        }
    }

    pub fn finish(self) -> HashMap<FunctionId, ControlFlowGraph<u32>> {
        self.cfg_map
    }
}

impl<'a> Visitor for ControlFlowAnalysis<'a> {
    fn visit_function(&mut self, function_id: &mut FunctionId) -> Result<()> {
        let function = self.ast.functions.get(*function_id).unwrap();
        let body = function.body.as_ref().unwrap();
        let cfg = constrct_cfg_from_block(body, &self.ast);

        let unreachable_block_indicies = cfg.find_unreachable_blocks();

        let unreachable: Vec<Option<&BasicBlock<u32>>> = unreachable_block_indicies
            .iter()
            .map(|block_index| cfg.get_block(*block_index))
            .collect();
        
        self.cfg_map.insert(*function_id, cfg);
        Ok(())
    }
}

pub fn constrct_cfg_from_block(block: &Block, ast: &AstArena) -> ControlFlowGraph<u32> {
    let mut loop_indicies = HashSet::<BlockIndex>::default();

    let mut cfg = ControlFlowGraph::default();
    let mut entry_block_index: Option<BlockIndex> = None;
    let mut basic_block = BasicBlock::<u32>::new();
    for statement_id in &block.statements {
        let statement = ast.statements.get(*statement_id).unwrap();
        match &statement.kind {
                // Non control-flow related statements, add to the currentf basic block
                StatementKind::Let(_)
                | StatementKind::State(_)
                | StatementKind::Expression(_) => {
                    basic_block.statements.push(0);
                }
                // Control flow
                StatementKind::Return(_) => {
                    cfg.set_has_early_return(true);
                    basic_block.statements.push(1);

                    let block_index = cfg.add_block(basic_block);
                    if entry_block_index.is_none() {
                        entry_block_index = Some(block_index);
                    }

                    cfg.add_edge_to_exit(block_index,  ControlFlowEdge::Return);
                    basic_block = BasicBlock::<u32>::new();
                },
                StatementKind::If(if_) => {

                    let block_index = cfg.add_block(basic_block);

                    let if_cfg = construct_cfg_from_if(if_,  ast, &2);

                    let if_cfg_has_early_return = if_cfg.has_early_return();

                    if if_cfg_has_early_return {
                        cfg.set_has_early_return(true);
                    }

                    cfg.consume_subgraph(if_cfg, None, block_index);

                    basic_block = BasicBlock::<u32>::new();
                }

                StatementKind::While(while_) => {
                    let block_index = cfg.add_block(basic_block);

                    let mut while_body_cfg = constrct_cfg_from_block(&while_.body, ast);
                    let while_body_has_early_return = while_body_cfg.has_early_return();

                    // Delete the normal flow edge from the last block to the exit node
                    while_body_cfg.delete_normal_edge(
                        while_body_cfg.last_index(),
                        while_body_cfg.exit_index(),
                    );

                    let true_edge = ControlFlowEdge::ConditionTrue(1);
                    let false_edge = ControlFlowEdge::ConditionFalse(1);

                    while_body_cfg.print();

                    cfg.consume_subgraph(while_body_cfg, Some(true_edge), block_index);

                    loop_indicies.insert(cfg.last_index());

                    if !while_body_has_early_return {
                      cfg.add_edge(cfg.last_index(), block_index, ControlFlowEdge::Normal);
                    }

                    cfg.enqueue_edge(block_index, false_edge);

                    println!("after while");
                    cfg.print();


                    // // let while_cfg = construct_cfg_from_while(while_, ast, &2);

                    // let while_cfg_has_early_return = while_cfg.has_early_return();

                    // if while_cfg_has_early_return {
                    //     cfg.set_has_early_return(true);
                    // }

                    // cfg.consume_subgraph(while_cfg, Some(block_index), block_index);

                    basic_block = BasicBlock::<u32>::new();
                }

                _ => {
                    // Do nothing for now...
                }
                // StatementKind::While(_) => todo!(),
                // StatementKind::For(_) => todo!(),
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

    cfg
}

pub fn construct_cfg_from_if(
    if_: &If,
    ast: &AstArena,
    statement_id: &u32,
) -> ControlFlowGraph<u32> {
    let mut cfg = ControlFlowGraph::<u32>::default();

    let true_edge = ControlFlowEdge::ConditionTrue(*statement_id);
    let false_edge = ControlFlowEdge::ConditionFalse(*statement_id);

    let If {
        body, alternate, ..
    } = if_;

    // The block for the `true` branch of the if statement
    let if_true_cfg = constrct_cfg_from_block(body, ast);

    // Whether the `true` branch of the if statement has an early return
    let if_true_cfg_has_early_return = if_true_cfg.has_early_return();

    cfg.consume_subgraph(if_true_cfg, Some(true_edge), cfg.entry_index());

    if let Some(ref else_) = alternate {
        match else_.deref() {
            Else::Block(else_block) => {
                let else_cfg = constrct_cfg_from_block(else_block, ast);
                let else_cfg_has_early_return = else_cfg.has_early_return();

                cfg.consume_subgraph(else_cfg, Some(false_edge), cfg.entry_index());

                if if_true_cfg_has_early_return && else_cfg_has_early_return {
                    // Both the `truƒe` and `false` branches of the if statement have an early return.
                    // So we know this block also has an early return
                    cfg.set_has_early_return(true)
                }
            }
            Else::If(_if) => {
                /*
                  FOR AN ELSE_IF

                  - false_edge points to entry of else_if block

                */
                let else_if_cfg = construct_cfg_from_if(_if, ast, &(statement_id + 1));
                let else_if_cfg_has_early_return = else_if_cfg.has_early_return();

                // Replaces the entry block of this if CFG
                let else_if_entry_block = BasicBlock::<u32>::new();
                let else_if_entry_block_index = cfg.add_block_raw(else_if_entry_block);

                cfg.add_edge(cfg.entry_index(), else_if_entry_block_index, false_edge);

                println!("else_if root before consume");
                cfg.print();

                // println!("else_if_cfg");
                // else_if_cfg.print();
                // cfg.print();
                // println!("entry_index {:?}", cfg.entry_index());
                // println!("last_index {:?}", cfg.last_index());

                if if_true_cfg_has_early_return && else_if_cfg_has_early_return {
                    cfg.set_has_early_return(true)
                }

                cfg.consume_subgraph(else_if_cfg, None, else_if_entry_block_index);
                // cfg.print();
                // println!("entry_index {:?}", cfg.entry_index());
                // println!("last_index {:?}", cfg.last_index());
                // println!("exit_index {:?}", cfg.exit_index());
            }
        }
    } else {
        // Assuming we have no `else` chains, the `false` edge should point to the *next* block.
        cfg.add_edge(cfg.entry_index(), cfg.exit_index(), false_edge)
    }

    if !cfg.has_early_return() {
        cfg.flush_edge_queue(cfg.exit_index());
    }
    cfg
}
