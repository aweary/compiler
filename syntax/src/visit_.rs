use crate::ast_::*;
use diagnostics::result::Result;

pub trait Visitor: Sized {
    fn context_mut(&mut self) -> &mut AstArena {
        unimplemented!()
    }

    fn context(&self) -> &AstArena;

    fn visit_module(&self, module_id: ModuleId) -> Result<()> {
        walk_module(self, module_id)
    }

    fn visit_function(&self, function_id: FunctionId) -> Result<()> {
        walk_function(self, function_id)
    }

    fn visit_component(&self, component_id: ComponentId) -> Result<()> {
        walk_component(self, component_id)
    }

    fn visit_expression(&self, expression: ExpressionId) -> Result<()> {
        walk_expression(self, expression)
    }

    fn visit_const(&self, const_id: ConstId) -> Result<()> {
        let arena = self.context();
        let const_ = arena.consts.get(const_id).unwrap();
        // let value = arena.expressions.get(const_.value).unwrap();
        // let mut value = value.borrow_mut();
        self.visit_expression(const_.value);
        Ok(())
    }
}

fn walk_module(visitor: &impl Visitor, module_id: ModuleId) -> Result<()> {
    let module = visitor.context().modules.get(module_id).unwrap();
    for definition in &module.definitions {
        match definition.kind {
            DefinitionKind::Function(function_id) => {
                visitor.visit_function(function_id)?;
            }
            DefinitionKind::Component(component_id) => {
                visitor.visit_component(component_id)?;
            }
            DefinitionKind::Const(const_) => {
                let arena = visitor.context();
                let const_ = arena.consts.get(const_).unwrap();
                visitor.visit_expression(const_.value)?;
            }
            DefinitionKind::Struct(_) => todo!(),
        }
    }
    Ok(())
}

fn walk_template(visitor: &impl Visitor, template_id: TemplateId) -> Result<()> {
    let template = visitor.context().templates.get(template_id).unwrap();
    let template = template.borrow();
    let open_tag = &template.open_tag;

    for TemplateAttribute { value, .. } in &open_tag.attributes {
        visitor.visit_expression(*value)?;
    }

    if let Some(children) = &template.children {
        for child in children {
            match child {
                TemplateChild::String(_) => {}
                TemplateChild::Expression(expression_id) => {
                    visitor.visit_expression(*expression_id)?;
                }
                TemplateChild::Template(template_id) => walk_template(visitor, *template_id)?,
            }
        }
    }
    Ok(())
}

pub fn walk_expression(visitor: &impl Visitor, expression: ExpressionId) -> Result<()> {
    let arena = visitor.context();
    let expression = arena.expressions.get(expression).unwrap();
    let expression = expression.borrow();
    match &*expression {
        Expression::Template(template_id) => {
            walk_template(visitor, *template_id)?;
        }
        Expression::Function(function_id) => {
            visitor.visit_function(*function_id)?;
        }
        Expression::Binary { left, right, .. } => {
            visitor.visit_expression(*left)?;
            visitor.visit_expression(*right)?;
        }
        Expression::Unary { operand, .. } => {
            visitor.visit_expression(*operand)?;
        }
        Expression::Call { callee, arguments } => {
            visitor.visit_expression(*callee)?;
            for argument in arguments {
                visitor.visit_expression(argument.value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub fn walk_function(visitor: &impl Visitor, function_id: FunctionId) -> Result<()> {
    let arena = visitor.context();
    let function = arena.functions.get(function_id).unwrap();
    let function = function.borrow();
    walk_block(visitor, function.body.unwrap())
}

pub fn walk_component(visitor: &impl Visitor, component_id: ComponentId) -> Result<()> {
    let arena = visitor.context();
    let component = arena.components.get(component_id).unwrap();
    let component = component.borrow();
    walk_block(visitor, component.body.unwrap())
}

fn walk_block(visitor: &impl Visitor, block_id: BlockId) -> Result<()> {
    let arena = visitor.context();
    let block = arena.blocks.get(block_id).unwrap();
    for statement_id in &block.statements {
        let statement = arena.statements.get(*statement_id).unwrap();
        match statement {
            Statement::Expression(expression_id) => {
                visitor.visit_expression(*expression_id)?;
            }
            Statement::Let { value, .. } => {
                visitor.visit_expression(*value)?;
            }
            Statement::State(state_id) => {
                let state = arena.states.get(*state_id).unwrap();
                visitor.visit_expression(state.value)?;
            }
            Statement::Return(expression_id) => {
                visitor.visit_expression(*expression_id)?;
            }
            Statement::If(if_) => {
                walk_if(visitor, if_)?;
            }
            Statement::While { condition, body } => {
                visitor.visit_expression(*condition)?;
                walk_block(visitor, *body)?;
            }
            Statement::Assignment { value, .. } => {
                visitor.visit_expression(*value)?;
            }
        }
    }
    Ok(())
}

fn walk_if(visitor: &impl Visitor, if_: &If) -> Result<()> {
    visitor.visit_expression(if_.condition)?;
    walk_block(visitor, if_.body)?;
    if let Some(else_) = &if_.alternate {
        match &**else_ {
            Else::If(if_) => walk_if(visitor, if_)?,
            Else::Block(block_id) => walk_block(visitor, *block_id)?,
        }
    }
    Ok(())
}
