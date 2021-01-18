use crate::ast::*;
use diagnostics::result::Result;

/// An AST visitor
pub trait Visitor: Sized {
    fn visit_module(&mut self, module: &mut Module) -> Result<()> {
        walk_module(self, module)
    }

    fn visit_enum(&mut self, _enum: &mut Enum) -> Result<()> {
        Ok(())
    }

    fn visit_struct(&mut self, _struct: &mut Struct) -> Result<()> {
        Ok(())
    }

    fn visit_const(&mut self, _const: &mut Const) -> Result<()> {
        Ok(())
    }

    fn visit_function(&mut self, _function: &mut Function) -> Result<()> {
        Ok(())
    }

    fn visit_component(&mut self, _component: &mut Component) -> Result<()> {
        Ok(())
    }

    fn visit_import(&mut self, _import: &mut Import) -> Result<()> {
        Ok(())
    }
}

pub fn walk_function(_visitor: &mut impl Visitor, _function: &mut Function) -> Result<()> {
    Ok(())
}

pub fn walk_module(visitor: &mut impl Visitor, module: &mut Module) -> Result<()> {
    for import in &mut module.imports {
        visitor.visit_import(import)?;
    }

    for definition in &mut module.definitions {
        match &mut definition.kind {
            DefinitionKind::Enum(enum_) => {
                visitor.visit_enum(enum_)?;
            }
            DefinitionKind::Function(function) => {
                visitor.visit_function(function)?;
            }
            // DefinitionKind::Struct(_) => {}
            DefinitionKind::Struct(struct_) => {
                visitor.visit_struct(struct_)?;
            }
            DefinitionKind::Component(component) => {
                visitor.visit_component(component)?;
            }
            DefinitionKind::Const(const_) => {
                visitor.visit_const(const_)?;
            }
        }
    }

    Ok(())
}