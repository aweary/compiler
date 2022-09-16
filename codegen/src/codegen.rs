use indoc::indoc;
use log::debug;
use std::{
    borrow::Borrow,
    cell::RefCell,
    collections::{HashMap, HashSet},
    fmt::format,
    vec,
};

use crate::templates::{generate_template_instructions, TemplateInstruction};

use common::{
    control_flow_graph::{BasicBlock, ControlFlowEdge, ControlFlowGraph, ControlFlowNode},
    symbol::Symbol,
};
use diagnostics::result::Result;
use evaluate::{value, Value};
use petgraph::{graph::NodeIndex, visit::EdgeRef, Direction};
use syntax::ast_::*;

type AstControlFlowGraph = ControlFlowGraph<StatementId, ExpressionId, Value>;

struct CodegenBranch {}

type CodegenBranchMap = HashMap<NodeIndex, CodegenBranch>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CodegenScopeType {
    Function(FunctionId),
    Component(ComponentId),
}

impl Into<CodegenScopeType> for FunctionId {
    fn into(self) -> CodegenScopeType {
        CodegenScopeType::Function(self)
    }
}

impl Into<CodegenScopeType> for ComponentId {
    fn into(self) -> CodegenScopeType {
        CodegenScopeType::Component(self)
    }
}

#[derive(Default, Debug)]
struct CodegenScope {
    scope: RefCell<Option<CodegenScopeType>>,
}

impl CodegenScope {
    pub fn set_scope(&self, scope: CodegenScopeType) {
        self.scope.replace(Some(scope));
    }

    pub fn get_scope(&self) -> CodegenScopeType {
        self.scope.borrow().expect("Scope not set")
    }
}

enum CodegenModuleLevelDefinition {
    Function {
        name: String,
        params: Vec<String>,
        body: String,
    },
    Constant {
        name: String,
        value: String,
    },
}

struct CodegenFunctionBody {}

/**
 * - Define module level functions and make sure references to them are resolved
 */
pub struct Codegen<'a> {
    module_name: String,
    arena: &'a mut AstArena,
    // TODO - This should be a stack
    scope: CodegenScope,
    definitions: RefCell<Vec<CodegenModuleLevelDefinition>>,
    template_function_map: RefCell<HashMap<TemplateId, String>>,
    visited: RefCell<HashSet<NodeIndex>>,
}

impl<'a> Codegen<'a> {
    pub fn new(module_name: String, arena: &'a mut AstArena) -> Self {
        Self {
            module_name,
            arena,
            scope: CodegenScope::default(),
            definitions: Default::default(),
            template_function_map: Default::default(),
            visited: Default::default(),
        }
    }

    fn define_function(&self, name: String, params: Vec<String>, body: String) {
        self.definitions
            .borrow_mut()
            .push(CodegenModuleLevelDefinition::Function { name, params, body });
    }

    pub fn write(&mut self, path: std::path::PathBuf) -> Result<()> {
        use std::fmt::Write;
        let mut output = String::new();
        self.write_header(&mut output)?;

        for definition in self.definitions.borrow().iter() {
            match definition {
                CodegenModuleLevelDefinition::Function { name, params, body } => {
                    writeln!(output, "function {}({}) {{", name, params.join(", "))?;
                    writeln!(output, "{}", body)?;
                    writeln!(output, "}}")?;
                }
                CodegenModuleLevelDefinition::Constant { name, value } => {
                    writeln!(output, "const {}: {} = {};", name, "Value", value)?;
                }
            }
        }

        std::fs::write(path, output)?;
        Ok(())
    }

    pub fn write_header(&self, output: &mut String) -> Result<()> {
        let header = format! {r"
          /**
           * GENERATED FILE - DO NOT EDIT
           * Compiled from module: {}.ws
           * Generated at {}
           */
        ",
            self.module_name,
            chrono::Utc::now().to_rfc3339()
        };
        output.push_str(&header);
        Ok(())
    }

    // TODO - This should be a symbol
    pub fn current_scope_name(&self) -> String {
        let scope = self.scope.get_scope();
        match scope {
            CodegenScopeType::Function(function_id) => {
                let function = self.arena.functions.get(function_id).unwrap().borrow();
                function.name.symbol.to_string()
            }
            CodegenScopeType::Component(component_id) => {
                let component = self.arena.components.get(component_id).unwrap().borrow();
                component.name.symbol.to_string()
            }
        }
    }

    pub fn codegen_component(
        &self,
        component_id: ComponentId,
        cfg: &AstControlFlowGraph,
    ) -> Result<()> {
        self.scope.set_scope(component_id.into());

        let component_body = self.codegen_from_cfg(cfg, None, None)?;
        let component_name = self.current_scope_name();

        self.define_function(component_name, vec![], component_body);
        // let mut visitor = TemplateVisitor::new(self.arena);
        // visitor.visit_component(&mut component.clone())?;

        Ok(())
    }

    pub fn codegen_function(
        &self,
        function_id: FunctionId,
        cfg: &AstControlFlowGraph,
    ) -> Result<()> {
        self.scope.set_scope(function_id.into());
        self.codegen_from_cfg(cfg, None, None)?;
        // let mut visitor = TemplateVisitor::new(self.arena);
        // visitor.visit_function(&mut function.clone())?;
        Ok(())
    }

    fn codegen_branch(
        &self,
        cfg: &AstControlFlowGraph,
        start: NodeIndex,
        end: NodeIndex,
    ) -> Result<String> {
        use std::fmt::Write;
        // let node = cfg.graph.node_weight(node_index).unwrap();
        let mut branch_code = String::new();

        writeln!(branch_code, "// ...")?;
        let code = self.codegen_from_cfg(cfg, Some(start), Some(end))?;
        // ...
        Ok(code)
    }

    pub fn codegen_from_cfg(
        &self,
        cfg: &AstControlFlowGraph,
        start: Option<NodeIndex>,
        end: Option<NodeIndex>,
    ) -> Result<String> {
        use petgraph::visit::{depth_first_search, Dfs};
        use petgraph::visit::{Control, DfsEvent};
        use std::fmt::Write;

        let start = start.unwrap_or(cfg.first_index().expect("first").0);
        debug!("codegen_from_cfg, start: {:?}", start);
        cfg.print();

        let mut visitor = Dfs::new(&cfg.graph, start);

        let mut codegen = String::new();

        while let Some(node_index) = visitor.next(&cfg.graph) {
            if let Some(end) = end {
                if node_index == end {
                    break;
                }
            }
            if self.visited.borrow().contains(&node_index) {
                continue;
            }
            let node = cfg.graph.node_weight(node_index).unwrap();
            debug!("codegen_from_cfg, node: {:?}", node);
            match node {
                ControlFlowNode::BasicBlock(block) => {
                    self.visited.borrow_mut().insert(node_index);
                    for statement_id in block.statements.iter() {
                        let code = self.codegen_statement(*statement_id)?;
                        writeln!(codegen, "{}", code)?;
                    }
                }
                ControlFlowNode::BranchCondition(condition) => {
                    self.visited.borrow_mut().insert(node_index);
                    debug!("BranchCondition");
                    // This is a branching condition, which will have edges to the blocks
                    // that are executed if the condition is true and false.
                    // The order in which we encounter these edges does not match the order
                    // we generate the code in (false edges come first due to how the graph
                    // is constructed).

                    let directed_edges = cfg.graph.edges_directed(node_index, Direction::Outgoing);
                    let (true_edge_target, false_edge_target) = {
                        let mut true_edge_target = None;
                        let mut false_edge_targe = None;
                        for edge in directed_edges {
                            let edge_target = edge.target();
                            let edge_weight = edge.weight();
                            match edge_weight {
                                ControlFlowEdge::ConditionTrue => {
                                    true_edge_target = Some(edge_target);
                                }
                                ControlFlowEdge::ConditionFalse => {
                                    false_edge_targe = Some(edge_target)
                                }
                                _ => {}
                            }
                        }
                        (true_edge_target.unwrap(), false_edge_targe.unwrap())
                    };

                    let codegen_condition = self.codegen_expression(*condition)?;
                    let codegen_branch_block_code =
                        self.codegen_branch(cfg, true_edge_target, false_edge_target)?;

                    debug!("codegen_condition: {}", codegen_condition);
                    debug!("codegen_branch_block_code: {}", codegen_branch_block_code);

                    let condition_codegen = format!(
                        r"if ({}) {{
                                {}
                            }}",
                        codegen_condition, codegen_branch_block_code
                    );
                    writeln!(codegen, "{}", condition_codegen)?;
                }
                // ...
                ControlFlowNode::LoopCondition(_) => todo!(),
                ControlFlowNode::Entry | ControlFlowNode::Exit => {
                    // Nothing for now
                }
            }
        }

        // depth_first_search(&cfg.graph, Some(start), |event| {
        //     match event {
        //         DfsEvent::Discover(node_index, _) => {
        //             match cfg.graph.node_weight(node_index).unwrap() {
        //                 ControlFlowNode::BasicBlock(block) => {
        //                     debug!("BasicBlock: {:?}, {:?}", block, node_index);
        //                     let mut block_statements = String::new();

        //                     for statement_id in &block.statements {
        //                         let codegened_statement =
        //                             self.codegen_statement(*statement_id).unwrap();
        //                         write!(block_statements, "{}", codegened_statement).unwrap();
        //                     }

        //                     debug!("Block statements: {}", block_statements);
        //                 }
        //                 ControlFlowNode::BranchCondition(value) => {
        //                     debug!("BranchCondition: {:?}", value);
        //                     let edges = cfg.graph.edges_directed(node_index, Direction::Outgoing);
        //                     for edge in edges {
        //                         let target = edge.target();
        //                         let weight = edge.weight();
        //                         match weight {
        //                             ControlFlowEdge::ConditionTrue => {}
        //                             ControlFlowEdge::Normal => {}
        //                             ControlFlowEdge::ConditionFalse => {}
        //                             ControlFlowEdge::Return => {}
        //                         }
        //                     }
        //                 }
        //                 ControlFlowNode::LoopCondition(_) => {
        //                     // code.push("while ($cond) ".to_string());
        //                 }

        //                 ControlFlowNode::Exit => {
        //                     debug!("Exit");
        //                 }
        //                 ControlFlowNode::Entry => {
        //                     debug!("Entry");
        //                 }
        //             }
        //         }
        //         DfsEvent::TreeEdge(u, v) => {
        //             let edge_index = cfg.graph.find_edge(u, v).unwrap();
        //             let weight = cfg.graph.edge_weight(edge_index).unwrap();

        //             match weight {
        //                 ControlFlowEdge::ConditionTrue => {
        //                     debug!("ConditionTrue from {:?} to {:?}", u, v);
        //                     // code.push("if (true) {".to_string());
        //                 }
        //                 ControlFlowEdge::ConditionFalse => {
        //                     debug!("ConditionFalse from {:?} to {:?}", u, v);
        //                     // code.push("if (false) {".to_string());
        //                 }
        //                 ControlFlowEdge::Return => {
        //                     debug!("Return from {:?} to {:?}", u, v);
        //                     // code.push("return;".to_string());
        //                 }
        //                 ControlFlowEdge::Normal => {
        //                     debug!("Normal from {:?} to {:?}", u, v);
        //                     // code.push("}".to_string());
        //                 }
        //             }

        //             // println!("\nTreeEdge: {:?} -> {:?}", u, v);
        //             // println!("Edge: {:?}", edge_index);
        //             // println!("Weight: {:?}\n", weight);
        //         }
        //         DfsEvent::BackEdge(u, v) => {
        //             debug!("BackEdge: {:?} -> {:?}", u, v);
        //             // println!("BackEdge: {:?} -> {:?}", u, v);
        //         }
        //         DfsEvent::CrossForwardEdge(u, v) => {
        //             debug!("CrossForwardEdge: {:?} -> {:?}", u, v);
        //             // println!("CrossForwardEdge: {:?} -> {:?}", u, v);
        //         }
        //         DfsEvent::Finish(u, _) => {
        //             debug!("Finish: {:?}", u);
        //         }
        //     }

        //     if let DfsEvent::TreeEdge(_, v) = event {
        //         // Just fixing the types
        //         if false {
        //             return Control::Break(v);
        //         }
        //     }

        //     Control::Continue
        // });

        Ok(codegen)
    }

    fn codegen_statement(&self, statement: StatementId) -> Result<String> {
        let statement = self.arena.statements.get(statement).unwrap();
        match statement {
            Statement::Let { name, value } => {
                let expression_id = *value;
                drop(statement);
                let value = self.codegen_expression(expression_id)?;
                Ok(format!("let {} = {};", name.symbol, value))
            }
            Statement::Return(value) => {
                drop(statement);
                let value = self.codegen_expression(*value)?;
                Ok(format!("return {};", value))
            }
            Statement::State { value, name } => {
                drop(statement);
                let value = self.codegen_expression(*value)?;
                Ok(format!("let {} = {};", name.symbol, value))
            }
            Statement::Expression(_) => todo!(),
            Statement::If(_) => todo!(),
            Statement::While { .. } => todo!(),
        }
    }

    fn codegen_expression(&self, expression_id: ExpressionId) -> Result<String> {
        let expression = self.arena.expressions.get(expression_id).unwrap().borrow();
        match &*expression {
            Expression::Number(value) => Ok(format!("{}", value)),
            Expression::Template(template_id) => self.codegen_template(*template_id),
            // Expression::Binary { left, right, op } => todo!(),
            Expression::Boolean(value) => Ok(format!("{}", value)),
            Expression::String(value) => Ok(format!("\"{}\"", value)),
            Expression::Reference(binding) => Ok(binding.to_string(&self.arena)),
            _ => Ok(String::from("$value")),
            // Expression::Call { callee, arguments } => todo!(),
            // Expression::If {
            //     condition,
            //     then_branch,
            //     else_branch,
            // } => todo!(),
        }
    }

    fn codegen_template(&self, template_id: TemplateId) -> Result<String> {
        let template = self.arena.templates.get(template_id).unwrap().borrow();
        let instruction_set = generate_template_instructions(&template, self.arena);

        let template_gen_function_name = format!(
            "{}${}$create_fragment_{}",
            self.module_name,
            self.current_scope_name(),
            template_id.index()
        );

        let mut fragment_variable_declarations = String::new();
        let mut fragment_create_statements = String::new();
        let mut fragment_mount_statements = vec![];

        self.template_function_map
            .borrow_mut()
            .insert(template_id, template_gen_function_name.clone());

        let mut current_element_offset = 0;
        let mut element_offset_stack = vec![];

        let mut template_gen_function_parameters = vec![];

        for embedded_expression in instruction_set.embedded_expressions {
            let expression = self
                .arena
                .expressions
                .get(embedded_expression)
                .unwrap()
                .borrow();
            if let Expression::Reference(binding) = *expression {
                let parameter_name = binding.to_string(&self.arena);
                template_gen_function_parameters.push(parameter_name);
            }
        }

        for instruction in instruction_set.instructions {
            use std::fmt::Write;
            match instruction {
                TemplateInstruction::CreateElement(element_name) => {
                    current_element_offset += 1;
                    element_offset_stack.push(current_element_offset);

                    // Declare a variable for the element
                    writeln!(
                        fragment_variable_declarations,
                        "let ${};",
                        current_element_offset
                    )?;

                    // Create the element
                    writeln!(
                        fragment_create_statements,
                        "${} = document.createElement(\"{}\");",
                        current_element_offset, element_name
                    )?;
                }
                TemplateInstruction::SetAttribute(name, value) => {
                    let value = self.codegen_expression(value)?;
                    writeln!(
                        fragment_create_statements,
                        "${}.setAttribute(\"{}\", {});",
                        current_element_offset, name, value
                    )?;
                }
                TemplateInstruction::FinishElementAttributes => {
                    // ...
                }
                TemplateInstruction::CloseElement => {
                    let element_offset = element_offset_stack
                        .pop()
                        .expect("Offset should exist for CloseElement");

                    if element_offset > 1 {
                        fragment_mount_statements.push(
                            format!("${}.appendChild(${});", element_offset - 1, element_offset)
                                .to_string(),
                        );
                    } else {
                        fragment_mount_statements
                            .push(format!("target.appendChild(${})", element_offset));
                    }
                }
                TemplateInstruction::EmbedExpression(expression_id) => {
                    let expression = self.codegen_expression(expression_id)?;
                    // writeln!(
                    //     fragment_mount_statements,
                    //     "${}.appendChild(document.createTextNode({}));",
                    //     current_element_offset, expression
                    // )?;
                    // ...
                }
                TemplateInstruction::SetText(text) => {
                    writeln!(
                        fragment_create_statements,
                        "${}.textContent = \"{}\";",
                        current_element_offset, text
                    )?;
                    // ...
                }
            }
        }

        fragment_mount_statements.reverse();

        let template_gen_function_body = format!(
            r"
           {}
           return {{
            create() {{
                {}
            }},
            mount(target) {{
                {}
            }},
           }}
        ",
            fragment_variable_declarations,
            fragment_create_statements,
            fragment_mount_statements.join("\n")
        );

        self.define_function(
            template_gen_function_name.clone(),
            template_gen_function_parameters,
            template_gen_function_body,
        );

        Ok(template_gen_function_name)
    }
}
