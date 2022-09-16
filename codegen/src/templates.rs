use std::borrow::Borrow;

use common::symbol::Symbol;
use diagnostics::result::Result;
use syntax::ast_::{
    AstArena, Expression, ExpressionId, Template, TemplateAttribute, TemplateChild,
};

use syntax::visit_::Visitor;

#[derive(Debug, Clone)]
pub struct TemplateInstructionSet {
    pub instructions: Vec<TemplateInstruction>,
    pub embedded_expressions: Vec<ExpressionId>,
}

#[derive(Debug, Clone)]
pub enum TemplateInstruction {
    CreateElement(Symbol),
    SetAttribute(Symbol, ExpressionId),
    FinishElementAttributes,
    CloseElement,
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
    let mut embedded_expressions = Vec::new();

    instructions.push(TemplateInstruction::CreateElement(open_tag.name.symbol));

    for TemplateAttribute { name, value } in &open_tag.attributes {
        instructions.push(TemplateInstruction::SetAttribute(name.symbol, *value));
    }

    instructions.push(TemplateInstruction::FinishElementAttributes);

    if let Some(children) = children {
        for child in children {
            match child {
                TemplateChild::String(symbol) => {
                    instructions.push(TemplateInstruction::SetText(*symbol));
                }
                TemplateChild::Expression(expression_id) => {
                    embedded_expressions.push(*expression_id);
                    instructions.push(TemplateInstruction::EmbedExpression(*expression_id));
                }
                TemplateChild::Template(template_id) => {
                    let template = arena.templates.get(*template_id).unwrap().borrow();
                    let child_instructions = generate_template_instructions(&template, arena);
                    drop(template);
                    instructions.extend(child_instructions.instructions);
                }
            }
        }
    }

    instructions.push(TemplateInstruction::CloseElement);

    TemplateInstructionSet {
        instructions,
        embedded_expressions,
    }
}
