use crate::ast_::*;
use diagnostics::result::Result;

pub trait Visitor: Sized {
    fn context_mut(&mut self) -> &mut AstArena;
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

    fn visit_expression(&self, expression: &mut Expression) -> Result<()> {
        Ok(())
    }

    fn visit_const(&self, const_id: ConstId) -> Result<()> {
        let arena = self.context();
        let const_ = arena.consts.get(const_id).unwrap();
        let value = arena.expressions.get(const_.value).unwrap();
        let mut value = value.borrow_mut();
        self.visit_expression(&mut value)?;
        Ok(())
    }
}

fn walk_module(visitor: &impl Visitor, module_id: ModuleId) -> Result<()> {
    let module = visitor.context().modules.get(module_id).unwrap();
    for definition in &module.definitions {
        match definition {
            Definition::Function(function_id) => {
                visitor.visit_function(*function_id)?;
            }
            Definition::Component(component_id) => {
                visitor.visit_component(*component_id)?;
            }
            Definition::Const(const_) => {
                let arena = visitor.context();
                let const_ = arena.consts.get(*const_).unwrap();
                let const_value = arena.expressions.get(const_.value).unwrap();
                let mut const_value = const_value.borrow_mut();
                visitor.visit_expression(&mut const_value)?;
            }
            Definition::Struct(_) => todo!(),
        }
    }
    Ok(())
}

fn walk_function(visitor: &impl Visitor, function_id: FunctionId) -> Result<()> {
    let arena = visitor.context();
    let function = arena.functions.get(function_id).unwrap();
    let function = function.borrow();
    walk_block(visitor, function.body.unwrap())
}

fn walk_component(visitor: &impl Visitor, component_id: ComponentId) -> Result<()> {
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
                visit_expression(visitor, *expression_id)?;
            }
            Statement::Let { value, .. } => {
                visit_expression(visitor, *value)?;
            }
            Statement::State { value, .. } => {
                visit_expression(visitor, *value)?;
            }
            Statement::Return(expression_id) => {
                visit_expression(visitor, *expression_id)?;
            }
            Statement::If(if_) => {
                walk_if(visitor, if_)?;
            }
            Statement::While { condition, body } => {
                visit_expression(visitor, *condition)?;
                walk_block(visitor, *body)?;
            }
        }
    }
    Ok(())
}

fn walk_if(visitor: &impl Visitor, if_: &If) -> Result<()> {
    visit_expression(visitor, if_.condition)?;
    walk_block(visitor, if_.body)?;
    if let Some(else_) = &if_.alternate {
        match &**else_ {
            Else::If(if_) => walk_if(visitor, if_)?,
            Else::Block(block_id) => walk_block(visitor, *block_id)?,
        }
    }
    Ok(())
}

fn visit_expression(visitor: &impl Visitor, expression_id: ExpressionId) -> Result<()> {
    let arena = visitor.context();
    let expression = arena.expressions.get(expression_id).unwrap();
    let mut expression = expression.borrow_mut();
    visitor.visit_expression(&mut expression)?;
    Ok(())
}
