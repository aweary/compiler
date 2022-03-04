use crate::span::Span;
use common::scope_map::Referant;
use common::symbol::Symbol;

use std::borrow::Borrow;
use std::sync::Arc;

#[derive(Debug, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct UniqueName(u32);

type Type = types::Type<Arc<Struct>, Arc<EffectDef>, Arc<TypeParameter>>;

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
    Struct(Arc<Struct>),
    Const(Arc<Const>),
    Type(Arc<TypeDef>),
    Effect(Arc<EffectDef>),
    Enum(Arc<Enum>),
    Function(Arc<Function>),
    Component(Arc<Component>),
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
pub struct TypeParameter {
    pub name: Identifier,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeParameters {
    pub span: Span,
    pub identifiers: Vec<Arc<TypeParameter>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Parameter {
    pub name: Identifier,
    pub type_: Option<TypeExpression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeExpression {
    pub kind: TypeExpressionKind,
    pub type_: Option<Type>,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeExpressionKind {
    Number,
    String,
    Boolean,
    Unit,
    Reference {
        name: Identifier,
        arguments: Option<Vec<TypeExpression>>,
    },
    Function {
        parameters: Vec<TypeExpression>,
        return_type: Box<TypeExpression>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Effect(pub TypeExpression);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypeDef {
    pub span: Span,
    pub name: Identifier,
    pub type_: TypeExpression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectDef {
    pub span: Span,
    pub name: Identifier,
}

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
pub enum Binding {
    Let(Arc<Let>),
    State(Arc<State>),
    Enum(Arc<Enum>),
    Function(Arc<Function>),
    Component(Arc<Component>),
    Parameter(Arc<Parameter>),
    Const(Arc<Const>),
    Iterator(Identifier),
    // TODO make this reference something that can be resolved
    Import(Span),
}

impl Binding {
    pub fn span(&self) -> Span {
        match self {
            Binding::Let(let_) => (&**let_).name.span,
            Binding::State(state) => (&**state).name.span,
            Binding::Enum(enum_) => (&**enum_).name.span,
            Binding::Function(func) => (&**func).name.span,
            Binding::Component(component) => (&**component).name.span,
            Binding::Parameter(param) => (&**param).name.span,
            Binding::Const(const_) => (&**const_).name.span,
            Binding::Iterator(iter) => iter.span,
            Binding::Import(span) => *span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TypeBinding {
    Struct(Arc<Struct>),
    Effect(Arc<EffectDef>),
    TypeParameter(Arc<TypeParameter>),
}

impl TypeBinding {
    pub fn span(&self) -> Span {
        match self {
            TypeBinding::Struct(struct_) => (&**struct_).name.span,
            TypeBinding::Effect(effect) => (&**effect).name.span,
            TypeBinding::TypeParameter(param) => (&**param).name.span,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectBinding(pub Arc<EffectDef>);

impl Referant for Binding {}
impl Referant for TypeBinding {}
impl Referant for EffectBinding {}

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
    Call(Call),
    Boolean(bool),
    Reference(Binding),
    Array(Vec<Expression>),
    Member {
        object: Box<Expression>,
        property: Identifier,
    },
    Range {
        start: Box<Expression>,
        end: Box<Expression>,
    },
    Assignment {
        left: Box<Expression>,
        right: Box<Expression>,
    },
    Match {
        value: Box<Expression>,
        cases: Vec<MatchCase>,
    },
    Block(Block),
    Await(Box<Expression>),
    View(Box<View>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Call {
    pub callee: Box<Expression>,
    pub arguments: Vec<Argument>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct View {
    pub constructor: Call,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Argument {
    pub span: Span,
    pub name: Option<Identifier>,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchCase {
    pub pattern: MatchPattern,
    pub body: Box<Expression>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchPattern {
    Wildcard,
    Expression(Box<Expression>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinOp {
    Equals,
    DoubleEquals,
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
    BinAnd,
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
    State(Arc<State>),
    For(For),
    While(While),
    If(If),
    Return(Expression),
    Expression(Expression),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct If {
    pub condition: Expression,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct While {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct For {
    pub iterator: Identifier,
    pub iterable: Expression,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Let {
    pub name: Identifier,
    pub unique_name: UniqueName,
    pub value: Expression,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct State {
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
    pub is_async: bool,
    pub type_parameters: Option<TypeParameters>,
    pub parameters: Option<Vec<Arc<Parameter>>>,
    pub return_type: Option<TypeExpression>,
    pub effect_type: Option<Effect>,
    pub body: Block,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Component {
    pub name: Identifier,
    pub is_async: bool,
    pub type_parameters: Option<TypeParameters>,
    pub parameters: Option<Vec<Arc<Parameter>>>,
    pub return_type: Option<TypeExpression>,
    pub effect_type: Option<Effect>,
    pub body: Block,
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
