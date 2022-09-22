use indexmap::IndexSet;
use log::debug;
use std::{
    borrow::Borrow,
    cell::RefCell,
    collections::{HashMap, HashSet},
    vec,
};
use Direction::{Incoming, Outgoing};

use crate::templates::{generate_template_instructions, TemplateInstruction};

use common::petgraph::dot::Dot;
use common::petgraph::graph::DiGraph;

use common::control_flow_graph::{
    ControlFlowEdge, ControlFlowGraph, ControlFlowMap, ControlFlowMapKey, ControlFlowNode,
};
use diagnostics::result::Result;
use evaluate::Value;
use petgraph::{
    graph::NodeIndex,
    visit::{EdgeRef, IntoEdgeReferences, NodeRef},
    Direction,
};
use syntax::ast_::*;

type AstControlFlowGraph = ControlFlowGraph<StatementId, ExpressionId, Value>;

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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
enum CodegenModuleLevelDefinition {
    Function {
        name: String,
        is_public: bool,
        params: Vec<String>,
        body: String,
    },
    Class {
        name: String,
        is_public: bool,
        extends: Option<String>,
        constructor: String,
        constructor_params: Vec<String>,
        methods: Vec<String>,
    },
    Constant {
        name: String,
        is_public: bool,
        value: String,
    },
}

#[derive(Default)]
struct Minifier {
    offset: usize,
    bindings: HashMap<Binding, String>,
}

impl Minifier {
    fn get_minified_binding(&mut self, binding: &Binding) -> &str {
        if self.bindings.contains_key(binding) {
            self.bindings.get(binding).unwrap()
        } else {
            let minified_binding =
                format!("${}_", char::from_u32(97 + self.offset as u32).unwrap());
            self.offset += 1;
            self.bindings.insert(*binding, minified_binding);
            self.bindings.get(binding).unwrap()
        }
    }
}

struct CodegenContext {
    block_depth: usize,
}

/**
 * - Define module level functions and make sure references to them are resolved
 */
pub struct Codegen<'a> {
    module_name: String,
    control_flow_map:
        ControlFlowMap<FunctionId, ComponentId, StatementId, ExpressionId, evaluate::Value>,
    arena: &'a mut AstArena,
    // TODO - This should be a stack
    scope: CodegenScope,
    definitions: RefCell<IndexSet<CodegenModuleLevelDefinition>>,
    template_function_map: RefCell<HashMap<TemplateId, String>>,
    minifier: RefCell<Minifier>,
    completed_functions: RefCell<HashSet<FunctionId>>,
}

impl<'a> Codegen<'a> {
    pub fn new(
        module_name: String,
        arena: &'a mut AstArena,
        control_flow_map: ControlFlowMap<
            FunctionId,
            ComponentId,
            StatementId,
            ExpressionId,
            evaluate::Value,
        >,
    ) -> Self {
        Self {
            module_name,
            arena,
            scope: CodegenScope::default(),
            definitions: Default::default(),
            template_function_map: Default::default(),
            minifier: Default::default(),
            control_flow_map,
            completed_functions: Default::default(),
        }
    }

    pub fn codegen_module(&self, module_id: ModuleId) -> Result<()> {
        let module = self.arena.modules.get(module_id).unwrap();
        for definition in &module.definitions {
            match definition.kind {
                DefinitionKind::Function(function_id) => {
                    if definition.public {
                        let cfg = self
                            .control_flow_map
                            .get(&ControlFlowMapKey::Function(function_id))
                            .unwrap();
                        self.codegen_function(function_id, cfg, true)?;
                    }
                }
                DefinitionKind::Component(component_id) => {
                    if definition.public {
                        let cfg = self
                            .control_flow_map
                            .get(&ControlFlowMapKey::Component(component_id))
                            .unwrap();
                        self.codegen_component(component_id, cfg, true)?;
                    }
                }
                DefinitionKind::Const(_) => {
                    // ...
                }
                DefinitionKind::Struct(_) => todo!(),
            }
        }
        // ...
        Ok(())
    }

    fn define_function(&self, name: String, is_public: bool, params: Vec<String>, body: String) {
        self.definitions
            .borrow_mut()
            .insert(CodegenModuleLevelDefinition::Function {
                name,
                is_public,
                params,
                body,
            });
    }

    fn define_class(
        &self,
        name: String,
        is_public: bool,
        extends: Option<String>,
        constructor: String,
        constructor_params: Vec<String>,
        methods: Vec<String>,
    ) {
        self.definitions
            .borrow_mut()
            .insert(CodegenModuleLevelDefinition::Class {
                name,
                is_public,
                extends,
                constructor,
                constructor_params,
                methods,
            });
    }

    pub fn write(&mut self, path: std::path::PathBuf) -> Result<()> {
        use std::fmt::Write;
        let mut output = String::new();
        self.write_header(&mut output)?;

        writeln!(output, "import {{signal}} from '@preact/signals-core';")?;

        for definition in self.definitions.borrow().iter() {
            match definition {
                CodegenModuleLevelDefinition::Function {
                    name,
                    is_public,
                    params,
                    body,
                } => {
                    if *is_public {
                        write!(output, "export ")?;
                    }
                    writeln!(output, "function {}({}) {{", name, params.join(", "))?;
                    writeln!(output, "{}", body)?;
                    writeln!(output, "}}")?;
                }
                CodegenModuleLevelDefinition::Constant {
                    name,
                    is_public,
                    value,
                } => {
                    if *is_public {
                        write!(output, "export ")?;
                    }
                    writeln!(output, "const {}: {} = {};", name, "Value", value)?;
                }
                CodegenModuleLevelDefinition::Class {
                    name,
                    is_public,
                    extends,
                    constructor,
                    constructor_params,
                    methods,
                } => {
                    if *is_public {
                        write!(output, "export ")?;
                    }
                    writeln!(output, "class {} ", name)?;
                    if let Some(extends) = extends {
                        write!(output, " extends {} {{", extends)?;
                    } else {
                        write!(output, " {{")?;
                    }
                    writeln!(output, "constructor({}) {{", constructor_params.join(", "))?;
                    writeln!(output, "{}", constructor)?;
                    writeln!(output, "}}")?;
                    for method in methods {
                        writeln!(output, "{}", method)?;
                    }
                    writeln!(output, "}}")?;
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
        is_public: bool,
    ) -> Result<()> {
        self.scope.set_scope(component_id.into());

        let component = self.arena.components.get(component_id).unwrap().borrow();

        let component_parameters = if let Some(parameters) = &component.parameters {
            parameters
                .iter()
                .map(|parameter| {
                    // self.minifier
                    //     .borrow_mut()
                    //     .get_minified_binding(&Binding::Parameter(*parameter))
                    //     .to_string()
                    self.arena
                        .parameters
                        .get(*parameter)
                        .unwrap()
                        .borrow()
                        .name
                        .symbol
                        .to_string()
                })
                .collect()
        } else {
            vec![]
        };

        let component_body = self.codegen_from_cfg(cfg, None, None, &Default::default())?;
        let component_name = self.current_scope_name();

        self.define_class(
            component_name,
            is_public,
            None,
            component_body,
            component_parameters,
            vec![],
        );
        Ok(())
    }

    pub fn codegen_function(
        &self,
        function_id: FunctionId,
        cfg: &AstControlFlowGraph,
        is_public: bool,
    ) -> Result<()> {
        if self.completed_functions.borrow().contains(&function_id) {
            return Ok(());
        }
        let function = self.arena.functions.get(function_id).unwrap().borrow();
        let function_name = function.name.symbol.to_string();
        println!("codegen_function {}", function_name);
        let function_parameters = if let Some(parameters) = &function.parameters {
            parameters
                .iter()
                .map(|parameter| {
                    self.arena
                        .parameters
                        .get(*parameter)
                        .unwrap()
                        .borrow()
                        .name
                        .symbol
                        .to_string()
                })
                .collect()
        } else {
            vec![]
        };
        println!("codegen_function_expression: {}", function_name);
        cfg.print();

        let codegen_body = self.codegen_from_cfg(cfg, None, None, &Default::default())?;
        self.define_function(function_name, is_public, function_parameters, codegen_body);
        self.completed_functions.borrow_mut().insert(function_id);
        Ok(())
    }

    pub fn codegen_function_expression(&self, function_id: FunctionId) -> Result<String> {
        use std::fmt::Write;
        let function = self.arena.functions.get(function_id).unwrap().borrow();
        let function_name = function.name.symbol.to_string();
        let function_parameters = if let Some(parameters) = &function.parameters {
            parameters
                .iter()
                .map(|parameter| {
                    self.arena
                        .parameters
                        .get(*parameter)
                        .unwrap()
                        .borrow()
                        .name
                        .symbol
                        .to_string()
                })
                .collect()
        } else {
            vec![]
        };
        let mut output = String::new();

        let cfg = self
            .control_flow_map
            .get(&ControlFlowMapKey::Function(function_id))
            .unwrap();

        println!("codegen_function_expression: {}", function_name);
        cfg.print();

        let codegen_body = self.codegen_from_cfg(cfg, None, None, &Default::default())?;

        writeln!(
            output,
            "function {}({}) {{",
            function_name,
            function_parameters.join(",")
        )?;
        writeln!(output, "{}", codegen_body)?;
        writeln!(output, "}}")?;
        Ok(output)
    }

    fn codegen_branch(
        &self,
        cfg: &AstControlFlowGraph,
        start: NodeIndex,
        end: NodeIndex,
        visited: &RefCell<HashSet<NodeIndex>>,
    ) -> Result<String> {
        use std::fmt::Write;
        // let node = cfg.graph.node_weight(node_index).unwrap();
        let mut branch_code = String::new();

        writeln!(branch_code, "// ...")?;
        let code = self.codegen_from_cfg(cfg, Some(start), Some(end), visited)?;
        // ...
        Ok(code)
    }

    pub fn codegen_from_cfg(
        &self,
        cfg: &AstControlFlowGraph,
        start: Option<NodeIndex>,
        end: Option<NodeIndex>,
        visited: &RefCell<HashSet<NodeIndex>>,
    ) -> Result<String> {
        use petgraph::visit::Dfs;
        use std::fmt::Write;

        let start = start.unwrap_or(cfg.first_index().unwrap_or(cfg.entry_index()).0);
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
            if visited.borrow().contains(&node_index) {
                continue;
            }
            let node = cfg.graph.node_weight(node_index).unwrap();
            debug!("codegen_from_cfg, node: {:?}", node);
            match node {
                ControlFlowNode::BasicBlock(block) => {
                    visited.borrow_mut().insert(node_index);
                    for statement_id in block.statements.iter() {
                        let code = self.codegen_statement(*statement_id)?;
                        writeln!(codegen, "{}", code)?;
                    }
                }
                ControlFlowNode::BranchCondition(condition) => {
                    visited.borrow_mut().insert(node_index);
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
                        self.codegen_branch(cfg, true_edge_target, false_edge_target, visited)?;

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
            Statement::State(state_id) => {
                let State { name, value } = self.arena.states.get(*state_id).unwrap();
                drop(statement);
                let value = self.codegen_expression(*value)?;
                Ok(format!("let {} = signal({});", name.symbol, value))
            }
            Statement::Expression(expression_id) => {
                drop(statement);
                let expression = self.codegen_expression(*expression_id)?;
                Ok(format!("{};", expression))
            }
            Statement::Assignment { name, value } => {
                drop(statement);
                if let Binding::State(_) = name {
                    let name = name.to_string(&self.arena);
                    let value = self.codegen_expression(*value)?;
                    Ok(format!("{}.value = {};", name, value))
                } else {
                    let name = name.to_string(&self.arena);
                    let value = self.codegen_expression(*value)?;
                    Ok(format!("{} = {};", name, value))
                }
            }
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
            Expression::Reference(binding) => {
                match binding {
                    Binding::State(_) => Ok(format!("{}.value", binding.to_string(&self.arena))),
                    _ => Ok(binding.to_string(&self.arena)),
                }
                // ...
                // Ok(self
                //     .minifier
                //     .borrow_mut()
                //     .get_minified_binding(binding)
                //     .to_string())
            }
            Expression::Function(function_id) => self.codegen_function_expression(*function_id),
            Expression::Binary { left, right, op } => {
                let left = self.codegen_expression(*left)?;
                let right = self.codegen_expression(*right)?;
                Ok(format!("{} {} {}", left, op, right))
            }
            Expression::Call { callee, arguments } => {
                // Make sure this function gets compiled.
                let callee_expression = self.arena.expressions.get(*callee).unwrap().borrow();
                println!("callee_expression: {:?}", callee_expression);
                if let Expression::Reference(binding) = &*callee_expression {
                    if let Binding::Function(function_id) = binding {
                        let cfg = self
                            .control_flow_map
                            .get(&ControlFlowMapKey::Function(*function_id))
                            .unwrap();
                        println!("calle_expression");
                        self.codegen_function(*function_id, cfg, false)?;
                        // if function.is_builtin {
                        //     let arguments = arguments
                        //         .iter()
                        //         .map(|argument| self.codegen_expression(*argument))
                        //         .collect::<Result<Vec<_>>>()?;
                        //     let arguments = arguments.join(", ");
                        //     return Ok(format!("{}({})", callee, arguments));
                        // }
                    }
                }

                let arguments = arguments
                    .iter()
                    .map(|argument| self.codegen_expression(argument.value))
                    .collect::<Result<Vec<_>>>()?
                    .join(", ");
                let callee = self.codegen_expression(*callee)?;
                Ok(format!("{}({})", callee, arguments))
            }
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
        let mut fragment_subscription_statements = HashMap::new();

        self.template_function_map
            .borrow_mut()
            .insert(template_id, template_gen_function_name.clone());

        // The monotonically increasing index of the current element.
        let mut node_offset = 0;
        // The current depth of the tree.
        let mut node_depth = 0;
        let mut parent_child_node_map: HashMap<i32, Vec<i32>> = HashMap::new();
        let mut node_offset_to_depth_map: HashMap<i32, i32> = HashMap::new();

        let mut template_gen_function_parameters = vec![];

        let mut seen_expression = HashSet::new();

        let mut template_graph: DiGraph<i32, i32> = DiGraph::new();
        let template_graph_root = template_graph.add_node(node_offset);
        let mut current_node = template_graph_root;

        debug!("instruction_set: {:#?}", instruction_set);
        for embedded_expression in instruction_set.embedded_expressions {
            let expression = self
                .arena
                .expressions
                .get(embedded_expression)
                .unwrap()
                .borrow();
            if let Expression::Reference(binding) = *expression {
                if !seen_expression.contains(&binding) {
                    seen_expression.insert(binding);
                    // let parameter_name = self
                    //     .minifier
                    //     .borrow_mut()
                    //     .get_minified_binding(&binding)
                    //     .to_string();
                    let parameter_name = binding.to_string(&self.arena);
                    template_gen_function_parameters.push(parameter_name);
                }
            }
        }

        for instruction in instruction_set.instructions {
            use std::fmt::Write;
            match instruction {
                TemplateInstruction::CreateElement(element_name) => {
                    node_offset += 1;
                    let template_graph_node_index = template_graph.add_node(node_offset);
                    template_graph.add_edge(current_node, template_graph_node_index, -node_offset);
                    current_node = template_graph_node_index;

                    // template_graph.add_edge();
                    // Declare a variable for the element
                    writeln!(fragment_variable_declarations, "let ${};", node_offset)?;

                    // Create the element
                    writeln!(
                        fragment_create_statements,
                        "${} = document.createElement(\"{}\");",
                        node_offset, element_name
                    )?;

                    // Add the element to the parent
                    parent_child_node_map
                        .entry(node_depth)
                        .or_insert(vec![])
                        .push(node_offset);
                }
                TemplateInstruction::SetAttribute(name, value) => {
                    let value = self.codegen_expression(value)?;
                    let name = name.to_string();
                    if name.starts_with("on") {
                        writeln!(
                            fragment_create_statements,
                            "${}.addEventListener(\"{}\", {});",
                            node_offset,
                            &name[2..].to_lowercase(),
                            value
                        )?;
                    } else {
                        writeln!(
                            fragment_create_statements,
                            "${}.setAttribute(\"{}\", {});",
                            node_offset, name, value
                        )?;
                    }
                }
                TemplateInstruction::FinishElementAttributes => {
                    // ...
                }
                TemplateInstruction::CloseElement => {
                    // Get the parent node of the current node.
                    current_node = template_graph
                        .neighbors_directed(current_node, Incoming)
                        .next()
                        .unwrap();
                    // let element_offset = element_offset_stack
                    //     .pop()
                    //     .expect("Offset should exist for CloseElement");

                    // while let Some(embed_offset) = embed_offset_stack.pop() {
                    //     fragment_mount_statements.push(format!(
                    //         "${}.appendChild($t{});",
                    //         element_offset, embed_offset
                    //     ))
                    // }

                    // if element_offset > 1 {
                    //     fragment_mount_statements.push(
                    //         format!("${}.appendChild(${});", element_offset - 1, element_offset)
                    //             .to_string(),
                    //     );
                    // } else {
                    //     fragment_mount_statements
                    //         .push(format!("target.appendChild(${})", element_offset));
                    // }
                }
                TemplateInstruction::EmbedExpression(expression_id) => {
                    let expression = self.arena.expressions.get(expression_id).unwrap().borrow();

                    node_offset += 1;
                    let template_graph_node_index = template_graph.add_node(node_offset);
                    template_graph.add_edge(current_node, template_graph_node_index, -node_offset);

                    // Declare a variable for the element
                    writeln!(fragment_variable_declarations, "let ${};", node_offset)?;

                    parent_child_node_map
                        .entry(node_depth)
                        .or_insert(vec![])
                        .push(node_offset);

                    let expression_value = self.codegen_expression(expression_id)?;

                    // Create the text element
                    writeln!(
                        fragment_create_statements,
                        "${} = document.createTextNode({});",
                        node_offset, expression_value
                    )?;

                    if let Expression::Reference(binding) = *expression {
                        if let Binding::State(_statement_id) = binding {
                            fragment_subscription_statements
                                .entry(binding)
                                .or_insert(vec![])
                                .push(format!("${}.textContent = v;", node_offset).to_string());
                        }
                    }

                    // writeln!(
                    //     fragment_create_statements,
                    //     "${}.appendChild(document.createTextNode({}));",
                    //     current_element_offset, expression
                    // )?;
                }
                TemplateInstruction::SetText(text) => {
                    node_offset += 1;
                    let template_graph_node_index = template_graph.add_node(node_offset);
                    template_graph.add_edge(current_node, template_graph_node_index, -node_offset);

                    // Create the text element
                    writeln!(
                        fragment_create_statements,
                        "${} = document.createTextNode(\"{}\");",
                        node_offset, text
                    )?;

                    parent_child_node_map
                        .entry(node_depth)
                        .or_insert(vec![])
                        .push(node_offset);
                    // ...
                }
                TemplateInstruction::MountComponent(_component_id) => {
                    // let component = self.arena.components.get(component_id).unwrap().borrow();
                    // let component_name = component.name.symbol.to_string();
                    // current_element_offset += 1;
                    // element_offset_stack.push(current_element_offset);
                    // writeln!(
                    //     fragment_create_statements,
                    //     "${} = new {}({});",
                    //     current_element_offset,
                    //     component_name,
                    //     template_gen_function_parameters.join(", ")
                    // )?;
                    // ...
                }
                TemplateInstruction::StartChildren => {
                    node_offset_to_depth_map.insert(node_depth, node_offset);
                    node_depth += 1;
                }
                TemplateInstruction::EndChildren => {
                    node_depth -= 1;
                }
            }
        }

        for edge in template_graph.raw_edges() {
            let source = edge.source();
            let target = edge.target();
            if source == template_graph_root {
                fragment_mount_statements
                    .push(format!("target.appendChild(${});", target.index()).to_string());
            } else {
                fragment_mount_statements.push(
                    format!("${}.appendChild(${});", source.index(), target.index()).to_string(),
                );
            }
        }

        let fragment_subscription_statements = fragment_subscription_statements
            .into_iter()
            .map(|(binding, statements)| {
                let binding = binding.to_string(&self.arena);
                format!(
                    "{}.subscribe((v) => {{ {} }});",
                    binding,
                    statements.join("\n")
                )
            })
            .collect::<Vec<_>>()
            .join("\n");

        let template_gen_function_body = format!(
            r"
           {}
           return {{
            create() {{
                {}
                // Subscriptions
                {}
            }},
            mount(target) {{
                {}
            }},
           }}
        ",
            fragment_variable_declarations,
            fragment_create_statements,
            fragment_subscription_statements,
            fragment_mount_statements.join("\n")
        );

        self.define_function(
            template_gen_function_name.clone(),
            false,
            template_gen_function_parameters.clone(),
            template_gen_function_body,
        );

        Ok(format!(
            "{}({})",
            template_gen_function_name,
            template_gen_function_parameters.join(", ")
        ))
    }
}
