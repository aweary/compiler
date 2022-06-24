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
    pub parameters: Arena<Parameter>,
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
pub type ParameterId = Id<Parameter>;

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
    String(Symbol),
    Reference(Binding),
    Call {
        callee: ExpressionId,
        arguments: Vec<Argument>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Argument {
    pub name: Option<Identifier>,
    pub value: ExpressionId,
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

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub enum Type {
    Number,
    String,
    Boolean,
    Function {
        parameters: Vec<Type>,
        return_type: Box<Type>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: Identifier,
    pub type_: Option<Type>,
}

pub struct Function {
    pub name: Identifier,
    pub body: Option<BlockId>,
    pub parameters: Option<Vec<ParameterId>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identifier {
    pub span: Span,
    pub symbol: Symbol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binding {
    Let(StatementId),
    Const(ConstId),
    Function(FunctionId),
    Parameter(ParameterId),
}

impl Referant for Binding {}
