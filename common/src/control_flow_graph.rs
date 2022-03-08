use petgraph::dot::Dot;
use petgraph::graph::{DiGraph, NodeIndex};
use std::collections::{HashMap, VecDeque};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BlockIndex(NodeIndex);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
struct Block {}

#[derive(Debug, Clone)]
pub enum ControlFlowEdge<T> {
    Normal,
    ConditionTrue(T),
    ConditionFalse(T),
    Return,
}

#[derive(Debug, Clone)]
pub enum ControlFlowNode<T> {
    Entry,
    BasicBlock(BasicBlock<T>),
    Exit,
}

#[derive(Debug, Clone)]
pub struct BasicBlock<T> {
    pub statements: Vec<T>,
}

impl<T> BasicBlock<T> {
    pub fn new() -> Self {
        BasicBlock {
            statements: Vec::new(),
        }
    }

    pub fn add(&mut self, statement: T) {
        self.statements.push(statement);
    }

    pub fn is_empty(&self) -> bool {
        self.statements.is_empty()
    }
}

pub struct ControlFlowGraph<T> {
    graph: DiGraph<ControlFlowNode<T>, ControlFlowEdge<T>>,
    edge_queue: VecDeque<(BlockIndex, ControlFlowEdge<T>)>,
    has_early_return: bool,
    entry_index: BlockIndex,
    exit_index: BlockIndex,
    first_index: Option<BlockIndex>,
    last_index: Option<BlockIndex>,
}

impl<T> Default for ControlFlowGraph<T> {
    fn default() -> Self {
        let mut graph = DiGraph::default();
        let entry_index = BlockIndex(graph.add_node(ControlFlowNode::Entry));
        let exit_index = BlockIndex(graph.add_node(ControlFlowNode::Exit));
        ControlFlowGraph {
            graph,
            edge_queue: VecDeque::new(),
            has_early_return: false,
            entry_index,
            exit_index,
            first_index: None,
            last_index: None,
        }
    }
}

impl<T: std::fmt::Debug + Clone> ControlFlowGraph<T> {
    pub fn print(&self) {
        println!("{:?}", Dot::with_config(&self.graph, &[]));
    }

    pub fn set_has_early_return(&mut self, has_early_return: bool) {
        self.has_early_return = has_early_return;
    }

    pub fn has_early_return(&self) -> bool {
        self.has_early_return
    }

    /// Consuming a sub-graph means taking another complete CFG and then integrating
    /// it into this one. This lets us recursively construct a CFG for an AST that
    /// might have arbitrarily nested control flow, like a bunch of nested if/else
    /// statements.
    pub fn consume_subgraph(
        &mut self,
        other: Self,
        entry_edge: Option<ControlFlowEdge<T>>,
        entry_index: BlockIndex,
        parent_has_early_return: bool,
    ) -> Vec<(BlockIndex, ControlFlowEdge<T>)> {
        let mut edges_to_enqueue: Vec<(BlockIndex, ControlFlowEdge<T>)> = vec![];

        let other_graph = other.graph;
        let other_node_indicies = other_graph.node_indices();
        let other_raw_edges = other_graph.raw_edges();

        let other_entry_index = other.entry_index;
        let other_exit_index = other.exit_index;

        let mut node_index_hash_map: HashMap<NodeIndex, NodeIndex> = HashMap::new();

        for other_node_index in other_node_indicies {
            // Iterate through every node in the subgraph. If its an exit or entry node,
            // we ignore it since we don't want to include those in our subgraph.
            let other_node = &other_graph[other_node_index];
            if let ControlFlowNode::BasicBlock(_) = other_node {
                // Create a clone of the node from the other graph, so we can include it in this graph
                let node = other_node.clone();
                // Add it to the graph
                let node_index = self.graph.add_node(node);
                // Add it to the hash map so we can map the old node index to the new one
                // when we add the edges
                node_index_hash_map.insert(other_node_index, node_index);
                self.last_index = Some(BlockIndex(node_index));
            }
        }

        // Now all the nodes from the subgraph have a clone in this graph, but they are no
        // edges. We need to add the edges, and handle the entry and exit edges specially.

        for other_raw_edge in other_raw_edges {
            // Where the edge *starts* in the subgraph
            let other_source_index = other_raw_edge.source();
            // Where the edge *ends* in the subgraph
            let other_target_index = other_raw_edge.target();

            // A copy of this edge's weight, to be used in this graph
            let edge_weight = if other_source_index == other_entry_index.0 {
                if parent_has_early_return {
                    continue;
                }
                // If the edge starts at the entry node, use the provided entry edge
                // instead of the subgraph's.
                entry_edge.clone().unwrap_or(other_raw_edge.weight.clone())
            } else {
                other_raw_edge.weight.clone()
            };

            // if the TARGET node is the subgraph's EXIT node, then this edge needs to be
            // removed and added to the edge queue for this graph. Since this should be
            // pointing to the *next* block.
            let target_index = if other_target_index == other_exit_index.0 {
                // Return edges are extra special, since we *know* they should
                // point to the exit node of this graph.
                if let ControlFlowEdge::Return = edge_weight {
                    self.exit_index.0
                } else {
                    edges_to_enqueue.push((
                        BlockIndex(node_index_hash_map[&other_source_index]),
                        edge_weight,
                    ));
                    continue;
                }
            } else {
                // Otherwise, we need to map the source node index to the new one
                node_index_hash_map[&other_target_index]
            };

            // If the SOURCE node is the subgraph's ENTRY node, we need to retarget it to
            // entry_index that was provided.
            let source_index = if other_source_index == other_entry_index.0 {
                entry_index.0
            } else {
                // Otherwise, we need to map the source node index to the new one
                node_index_hash_map[&other_source_index]
            };

            // Now we can add the edge to the graph
            self.graph.add_edge(source_index, target_index, edge_weight);
        }
        println!("edges_to_enqueue: {:?}", edges_to_enqueue);
        edges_to_enqueue
    }

    pub fn entry_index(&self) -> BlockIndex {
        self.entry_index
    }

    pub fn exit_index(&self) -> BlockIndex {
        self.exit_index
    }

    pub fn first_index(&self) -> Option<BlockIndex> {
        self.first_index
    }

    pub fn last_index(&self) -> BlockIndex {
        self.last_index.unwrap_or(self.entry_index)
    }

    pub fn enqueue_edge(&mut self, block_index: BlockIndex, edge: ControlFlowEdge<T>) {
        self.edge_queue.push_back((block_index, edge));
    }

    pub fn add_block(&mut self, block: BasicBlock<T>) -> BlockIndex {
        let index = BlockIndex(self.graph.add_node(ControlFlowNode::BasicBlock(block)));
        if self.first_index.is_none() {
            self.add_edge(self.entry_index, index, ControlFlowEdge::Normal);
            self.first_index = Some(index);
        }
        self.last_index = Some(index);
        self.flush_edge_queue(index);
        index
    }

    pub fn flush_edge_queue(&mut self, to: BlockIndex) {
        while let Some((block_index, edge)) = self.edge_queue.pop_front() {
            self.add_edge(block_index, to, edge);
        }
    }

    pub fn edge_queue_contains(&mut self, block_index: BlockIndex) -> bool {
        self.edge_queue
            .iter()
            .any(|(index, _)| *index == block_index)
    }

    pub fn add_edge(&mut self, from: BlockIndex, to: BlockIndex, edge: ControlFlowEdge<T>) {
        self.graph.add_edge(from.0, to.0, edge);
    }

    pub fn add_edge_to_exit(&mut self, from: BlockIndex, edge: ControlFlowEdge<T>) {
        self.graph.add_edge(from.0, self.exit_index.0, edge);
    }

    pub fn add_edge_to_entry(&mut self, to: BlockIndex, edge: ControlFlowEdge<T>) {
        self.graph.add_edge(self.entry_index.0, to.0, edge);
    }

    pub fn add_edge_to_first(&mut self, to: BlockIndex, edge: ControlFlowEdge<T>) {
        self.graph.add_edge(self.first_index.unwrap().0, to.0, edge);
    }

    pub fn add_edge_to_last(&mut self, to: BlockIndex, edge: ControlFlowEdge<T>) {
        self.graph.add_edge(self.last_index.unwrap().0, to.0, edge);
    }

    pub fn get_block(&self, index: BlockIndex) -> Option<&BasicBlock<T>> {
        match &self.graph[index.0] {
            ControlFlowNode::BasicBlock(block) => Some(block),
            ControlFlowNode::Entry => None,
            ControlFlowNode::Exit => None,
        }
    }

    pub fn find_unreachable_blocks(&self) -> Vec<BlockIndex> {
        let mut unreachable_blocks = Vec::new();
        for node_index in self.graph.node_indices() {
            match &self.graph[node_index] {
                ControlFlowNode::Entry | ControlFlowNode::Exit => continue,
                ControlFlowNode::BasicBlock(_) => {
                    if self
                        .graph
                        .neighbors_directed(node_index, petgraph::Incoming)
                        .count()
                        == 0
                    {
                        unreachable_blocks.push(BlockIndex(node_index));
                    }
                }
            }
        }
        println!("unreachable_blocks: {:?}", unreachable_blocks);
        unreachable_blocks
    }
}