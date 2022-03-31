use crate::{ast::BinOp, span::Span};
use common::scope_map::Referant;
use common::symbol::Symbol;
use diagnostics::result::Result;
use id_arena::{Arena, Id};
use std::cell::RefCell;

#[derive(Default)]
pub struct AstArena {
    pub modules: Arena<Module>,
    pub blocks: Arena<Block>,
    pub structs: Arena<Struct>,
    pub expressions: Arena<RefCell<Expression>>,
    pub functions: Arena<RefCell<Function>>,
    pub statements: Arena<Statement>,
    pub consts: Arena<Const>,
}

impl AstArena {
    pub fn alloc_expression(&mut self, expression: Expression) -> ExpressionId {
        self.expressions.alloc(RefCell::new(expression))
    }

    pub fn alloc_function(&mut self, function: Function) -> FunctionId {
        self.functions.alloc(RefCell::new(function))
    }
}

pub type ModuleId = Id<Module>;
pub type BlockId = Id<Block>;
pub type StructId = Id<Struct>;
pub type ExpressionId = Id<RefCell<Expression>>;
pub type FunctionId = Id<RefCell<Function>>;
pub type StatementId = Id<Statement>;
pub type ConstId = Id<Const>;

pub struct Module {
    pub definitions: Vec<Definition>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Definition {
    Function(FunctionId),
    Const(ConstId),
    Struct(StructId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Const {
    pub name: Identifier,
    pub value: ExpressionId,
}

pub struct Struct {}

#[derive(Clone, Debug, PartialEq)]
pub enum Expression {
    If {
        condition: ExpressionId,
        then_branch: BlockId,
        else_branch: Option<BlockId>,
    },
    Binary {
        left: ExpressionId,
        right: ExpressionId,
        op: BinOp,
    },
    Number(f64),
    Boolean(bool),
    Reference(Binding),
}

#[derive(Debug)]
pub enum Statement {
    Expression(ExpressionId),
    Let {
        name: Identifier,
        value: ExpressionId,
    },
    Return(ExpressionId),
    If(If),
    While {
        condition: ExpressionId,
        body: BlockId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct If {
    pub condition: ExpressionId,
    pub body: BlockId,
    pub alternate: Option<Box<Else>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Else {
    If(If),
    Block(BlockId),
}

pub struct Block {
    pub statements: Vec<StatementId>,
}

pub struct Function {
    pub name: Identifier,
    pub body: BlockId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identifier {
    pub span: Span,
    pub name: Symbol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    Let(StatementId),
    Const(ConstId),
    Function(FunctionId),
}

impl Referant for Binding {}
