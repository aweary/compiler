use std::cell::RefCell;
use std::collections::{HashMap, HashSet};

use common::symbol::Symbol;
use diagnostics::result::Result;
use log::debug;
use syntax::ast_::{
    AstArena, Binding, ComponentId, Expression, ExpressionId, StateId, Template, TemplateAttribute,
    TemplateChild,
};

use syntax::visit_::{walk_expression, Visitor};

pub struct TemplateExpressionVisitor<'a> {
    expression_id: ExpressionId,
    stateful_expressions: RefCell<Option<HashMap<ExpressionId, StateId>>>,
    arena: &'a AstArena,
}

impl<'a> TemplateExpressionVisitor<'a> {
    pub fn new(expression_id: ExpressionId, arena: &'a AstArena) -> Self {
        Self {
            expression_id,
            stateful_expressions: Default::default(),
            arena,
        }
    }

    pub fn stateful_expressions(&self) -> Option<HashMap<ExpressionId, StateId>> {
        self.visit_expression(self.expression_id).unwrap();
        self.stateful_expressions.take()
    }
}

impl<'a> Visitor for TemplateExpressionVisitor<'a> {
    fn context(&self) -> &AstArena {
        &self.arena
    }

    fn visit_expression(&self, expression_id: ExpressionId) -> Result<()> {
        let expression = self.arena.expressions.get(expression_id).unwrap();
        let expression = expression.borrow();
        if let Expression::Reference(binding) = *expression {
            if let Binding::State(_) = binding {
                let state_id = binding.to_state(self.arena).unwrap();
                let mut stateful_expressions = self.stateful_expressions.borrow_mut();
                if let Some(stateful_expressions) = stateful_expressions.as_mut() {
                    stateful_expressions.insert(expression_id, state_id);
                } else {
                    *stateful_expressions = Some(HashMap::new());
                    stateful_expressions
                        .as_mut()
                        .unwrap()
                        .insert(expression_id, state_id);
                }
            }
        }
        walk_expression(self, expression_id)
    }
}

#[derive(Debug, Clone)]
pub struct TemplateInstructionSet {
    pub instructions: Vec<TemplateInstruction>,
    pub embedded_expressions: HashSet<ExpressionId>,
    pub stateful_expressions: HashMap<ExpressionId, StateId>,
}

#[derive(Debug, Clone)]
pub enum TemplateInstruction {
    CreateElement(Symbol),
    MountComponent(ComponentId),
    SetAttribute(Symbol, ExpressionId),
    FinishElementAttributes,
    CloseElement,
    StartChildren,
    EndChildren,
    EmbedExpression(ExpressionId),
    SetText(Symbol),
}

pub fn generate_template_instructions(
    template: &Template,
    arena: &AstArena,
) -> TemplateInstructionSet {
    let Template {
        open_tag, children, ..
    } = template;

    let mut instructions = Vec::new();
    let mut embedded_expressions = HashSet::new();
    let mut stateful_expressions = HashMap::new();

    instructions.push(TemplateInstruction::CreateElement(open_tag.name.symbol));

    for TemplateAttribute { name, value } in &open_tag.attributes {
        instructions.push(TemplateInstruction::SetAttribute(name.symbol, *value));
        let expression = arena.expressions.get(*value).unwrap().borrow();
        if !expression.is_constant() {
            embedded_expressions.insert(*value);
        }

        if let Some(s) = TemplateExpressionVisitor::new(*value, arena).stateful_expressions() {
            println!("GOT SOME");
            stateful_expressions.extend(s);
        }
    }

    instructions.push(TemplateInstruction::FinishElementAttributes);

    if let Some(children) = children {
        for child in children {
            match child {
                TemplateChild::String(symbol) => {
                    instructions.push(TemplateInstruction::SetText(*symbol));
                }
                TemplateChild::Expression(expression_id) => {
                    embedded_expressions.insert(*expression_id);
                    if let Some(s) =
                        TemplateExpressionVisitor::new(*expression_id, arena).stateful_expressions()
                    {
                        stateful_expressions.extend(s);
                    }
                    instructions.push(TemplateInstruction::EmbedExpression(*expression_id));
                }
                TemplateChild::Template(template_id) => {
                    let template = arena.templates.get(*template_id).unwrap().borrow();
                    if let Some(binding) = template.open_tag.reference {
                        instructions.push(TemplateInstruction::MountComponent(binding.into()));
                        println!("Referencing another component")
                    }

                    let child_instructions = generate_template_instructions(&template, arena);
                    println!("Child instructions: {:#?}", child_instructions);
                    drop(template);
                    instructions.push(TemplateInstruction::StartChildren);
                    instructions.extend(child_instructions.instructions);
                    instructions.push(TemplateInstruction::EndChildren);
                    embedded_expressions.extend(child_instructions.embedded_expressions);
                    stateful_expressions.extend(child_instructions.stateful_expressions);
                }
            }
        }
    }

    instructions.push(TemplateInstruction::CloseElement);

    TemplateInstructionSet {
        instructions,
        embedded_expressions,
        stateful_expressions,
    }
}
