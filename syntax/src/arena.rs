use crate::ast::*;
use id_arena::{Arena, Id};
use std::cell::RefCell;

thread_local! {
    static EXPRESSION_ARENA : RefCell<Arena<Expression>> = RefCell::new(Arena::new());
    static FUNCTION_ARENA : RefCell<Arena<Function>> = RefCell::new(Arena::new());
}

pub type FunctionId = Id<Function>;
pub type ComponentId = Id<Component>;
pub type EnumId = Id<Enum>;
pub type StatementId = Id<Statement>;
pub type ExpressionId = Id<Expression>;

pub fn alloc_expression(expression: Expression) -> ExpressionId {
    EXPRESSION_ARENA.with(|arena| arena.borrow_mut().alloc(expression))
}

pub fn with_mut_expression(expression_id: ExpressionId, f: impl FnOnce(&mut Expression)) {
    EXPRESSION_ARENA.with(|arena| {
        let mut arena = arena.borrow_mut();
        let expression = arena.get_mut(expression_id).unwrap();
        f(expression);
    });
}

pub fn alloc_function(function: Function) -> FunctionId {
    FUNCTION_ARENA.with(|arena| arena.borrow_mut().alloc(function))
}

pub fn with_mut_function(function_id: FunctionId, f: impl FnOnce(&mut Function)) {
    FUNCTION_ARENA.with(|arena| {
        let mut arena = arena.borrow_mut();
        let function = arena.get_mut(function_id).unwrap();
        f(function);
    });
}

pub fn with_function(function_id: FunctionId, f: impl FnOnce(&Function)) {
    FUNCTION_ARENA.with(|arena| {
        let arena = arena.borrow();
        let function = arena.get(function_id).unwrap();
        f(function);
    });
}

#[derive(Default)]
pub struct AstArena {
    pub functions: Arena<Function>,
    pub components: Arena<Component>,
    pub enums: Arena<Enum>,
    pub statements: Arena<Statement>,
    pub expressions: Arena<Expression>,
}
