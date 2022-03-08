use crate::ast::*;
use id_arena::{Arena, Id};

pub type FunctionId = Id<Function>;
pub type ComponentId = Id<Component>;
pub type EnumId = Id<Enum>;
pub type StatementId = Id<Statement>;

#[derive(Default)]
pub struct AstArena {
    pub functions: Arena<Function>,
    pub components: Arena<Component>,
    pub enums: Arena<Enum>,
    pub statements: Arena<Statement>,
}
