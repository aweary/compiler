use crate::span::Span;
use crate::symbol::Symbol;

use types::Type;
use std::sync::Arc;

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct UniqueName(u32);

impl From<u32> for UniqueName {
    fn from(id: u32) -> Self {
        UniqueName(id)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Module {
    pub imports: Vec<Import>,
    pub definitions: Vec<Definition>,
}

impl Module {
    pub fn new(imports: Vec<Import>, definitions: Vec<Definition>) -> Self {
        Self {
            imports,
            definitions,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    pub is_public: bool,
    pub kind: DefinitionKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DefinitionKind {
    Struct(Struct),
    Const(Const),
    Enum(Enum),
    Function(Function),
    Component(Component),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Import {
    parts: Vec<ImportPart>,
}

impl Import {
    pub fn new(parts: Vec<ImportPart>) -> Self {
        Self { parts }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportPart {
    Module(Identifier),
    /// Collections can only occur at the end of
    /// an import path.
    Collection(Vec<Identifier>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParameters {
    pub span: Span,
    pub identifiers: Vec<Identifier>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: Identifier,
    pub type_: Option<TypeExpression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeExpression {
    pub name: Identifier,
    pub arguments: Option<Vec<TypeExpression>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect(pub TypeExpression);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Struct {
    pub name: Identifier,
    pub type_parameters: Option<TypeParameters>,
    pub fields: Vec<StructField>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructField {
    pub name: Identifier,
    pub type_: TypeExpression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Const {
    pub name: Identifier,
    pub type_: Option<TypeExpression>,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Enum {
    pub name: Identifier,
    pub type_parameters: Option<TypeParameters>,
    pub variants: Vec<Variant>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Variant {
    pub name: Identifier,
    pub types: Option<Vec<TypeExpression>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpressionKind {
    // TODO(aweary) don't use u32 for the value representation
    Number {
        raw: Symbol,
        value: Option<u32>,
    },
    String {
        raw: Symbol,
        // TODO(aweary) evaluate strings
    },
    Binary {
        left: Box<Expression>,
        right: Box<Expression>,
        op: BinOp,
    },
    Boolean(bool),
    Reference(Binding)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinOp {
    Equals,
    DblEquals,
    Add,
    Sub,
    Sum,
    Mul,
    Div,
    Mod,
    And,
    Or,
    GreaterThan,
    LessThan,
    Pipeline,
    BinOr,
    BinAdd,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expression {
    pub span: Span,
    pub kind: ExpressionKind,
    /// The evaluated type for this expression. Populated
    /// during the type check pass.
    pub type_: Option<Type>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatementKind {
    Let(Arc<Let>),
    ForIn(ForIn),
    While(While),
    If(If),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct If {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct While {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForIn {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Let {
    pub name: Identifier,
    pub unique_name: UniqueName,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statement {
    pub span: Span,
    pub kind: StatementKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    pub statements: Vec<Statement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Function {
    pub name: Identifier,
    pub type_parameters: Option<TypeParameters>,
    pub parameters: Option<Vec<Parameter>>,
    pub return_type: Option<TypeExpression>,
    pub effect_type: Option<Effect>,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    pub name: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identifier {
    pub span: Span,
    pub symbol: Symbol,
}

impl Identifier {
    pub fn new(symbol: Symbol, span: Span) -> Self {
        Self { symbol, span }
    }
}
