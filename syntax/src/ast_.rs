use crate::{ast::BinOp, span::Span};
use common::scope_map::{Referant, Reference};
use common::symbol::Symbol;
use id_arena::{Arena, Id};
use std::cell::RefCell;

#[derive(Default)]
pub struct AstArena {
    pub modules: Arena<Module>,
    pub blocks: Arena<Block>,
    pub structs: Arena<Struct>,
    pub expressions: Arena<RefCell<Expression>>,
    pub functions: Arena<RefCell<Function>>,
    pub components: Arena<RefCell<Component>>,
    pub statements: Arena<Statement>,
    pub consts: Arena<Const>,
    pub parameters: Arena<Parameter>,
    pub templates: Arena<RefCell<Template>>,
    pub states: Arena<State>,
}

impl AstArena {
    pub fn alloc_expression(&mut self, expression: Expression) -> ExpressionId {
        self.expressions.alloc(RefCell::new(expression))
    }

    pub fn alloc_template(&mut self, template: Template) -> TemplateId {
        self.templates.alloc(RefCell::new(template))
    }

    pub fn alloc_function(&mut self, function: Function) -> FunctionId {
        self.functions.alloc(RefCell::new(function))
    }

    pub fn alloc_component(&mut self, component: Component) -> ComponentId {
        self.components.alloc(RefCell::new(component))
    }
}

pub type ModuleId = Id<Module>;
pub type BlockId = Id<Block>;
pub type StructId = Id<Struct>;
pub type ExpressionId = Id<RefCell<Expression>>;
pub type TemplateId = Id<RefCell<Template>>;
pub type FunctionId = Id<RefCell<Function>>;
pub type ComponentId = Id<RefCell<Component>>;
pub type StatementId = Id<Statement>;
pub type ConstId = Id<Const>;
pub type ParameterId = Id<Parameter>;
pub type EnumId = Id<Enum>;
pub type StateId = Id<State>;

pub struct Module {
    pub definitions: Vec<Definition>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Definition {
    pub kind: DefinitionKind,
    pub public: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DefinitionKind {
    Function(FunctionId),
    Component(ComponentId),
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
    Unary {
        op: BinOp,
        operand: ExpressionId,
    },
    Number(f64),
    Boolean(bool),
    String(Symbol),
    Reference(Binding),
    Call {
        callee: ExpressionId,
        arguments: Vec<Argument>,
    },
    Template(TemplateId),
    Function(FunctionId),
}

impl Expression {
    pub fn is_constant(&self) -> bool {
        match self {
            Expression::Number(_) => true,
            Expression::Boolean(_) => true,
            Expression::String(_) => true,
            _ => false,
        }
    }
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
    State(StateId),
    Return(ExpressionId),
    If(If),
    While {
        condition: ExpressionId,
        body: BlockId,
    },
    Assignment {
        name: Binding,
        value: ExpressionId,
    },
}

#[derive(Debug)]
pub struct State {
    pub name: Identifier,
    pub value: ExpressionId,
}

impl Statement {
    pub fn is_state_assignment(&self) -> bool {
        match self {
            Statement::Assignment { name, .. } => match name {
                Binding::State(_) => true,
                _ => false,
            },
            _ => false,
        }
    }
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

pub struct Component {
    pub name: Identifier,
    pub body: Option<BlockId>,
    pub parameters: Option<Vec<ParameterId>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Identifier {
    pub span: Span,
    pub symbol: Symbol,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Binding {
    Let(StatementId),
    State(StatementId),
    Const(ConstId),
    Function(FunctionId),
    Parameter(ParameterId),
    Component(ComponentId),
}

impl Binding {
    pub fn to_string(&self, arena: &AstArena) -> String {
        match self {
            Binding::Let(statent_id) => {
                let statement = &arena.statements[*statent_id];
                match statement {
                    Statement::Let { name, .. } => name.symbol.to_string(),
                    _ => unreachable!(),
                }
            }
            Binding::State(statement_id) => {
                let statement = &arena.statements[*statement_id];
                match statement {
                    Statement::State(state_id) => {
                        let state = &arena.states[*state_id];
                        state.name.symbol.to_string()
                    }
                    _ => unreachable!(),
                }
            }
            Binding::Function(function_id) => {
                let function = &arena.functions[*function_id].borrow();
                function.name.symbol.to_string()
            }
            Binding::Const(_) => todo!(),
            Binding::Component(_) => todo!(),
            Binding::Parameter(parameter_id) => {
                let parameter = &arena.parameters[*parameter_id];
                parameter.name.symbol.to_string()
            }
        }
    }

    pub fn to_state(&self, arena: &AstArena) -> Option<StateId> {
        match self {
            Binding::State(state_id) => {
                let statement = &arena.statements[*state_id];
                match statement {
                    Statement::State(state_id) => Some(*state_id),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

impl Into<ComponentId> for Binding {
    fn into(self) -> ComponentId {
        match self {
            Binding::Component(id) => id,
            _ => unreachable!(),
        }
    }
}

impl Referant for Binding {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enum {
    pub name: Identifier,
    // pub type_parameters: Option<TypeParameters>,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub name: Identifier,
    // pub types: Option<Vec<TypeExpression>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct TemplateOpenTag {
    pub name: Identifier,
    pub reference: Option<Binding>,
    pub attributes: Vec<TemplateAttribute>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateAttribute {
    pub name: Identifier,
    pub value: ExpressionId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TemplateCloseTag {
    pub name: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    pub open_tag: TemplateOpenTag,
    pub children: Option<Vec<TemplateChild>>,
    pub close_tag: Option<TemplateCloseTag>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TemplateChild {
    String(Symbol),
    Expression(ExpressionId),
    Template(TemplateId),
}
