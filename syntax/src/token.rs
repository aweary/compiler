use crate::span::Span;
use common::symbol::Symbol;
use crate::Precedence;
use std::fmt::{Debug, Display};

use crate::ast::BinOp;

#[derive(Clone, PartialEq, Eq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl PartialEq<TokenKind> for Token {
    fn eq(&self, other: &TokenKind) -> bool {
        &self.kind == other
    }
}

impl PartialEq<Token> for TokenKind {
    fn eq(&self, other: &Token) -> bool {
        self == &other.kind
    }
}

impl Into<BinOp> for Token {
    fn into(self) -> BinOp {
        use BinOp::*;
        match self.kind {
            TokenKind::Equals => Equals,
            TokenKind::Plus => Add,
            TokenKind::Minus => Sub,
            TokenKind::Star => Mul,
            TokenKind::Slash => Div,
            TokenKind::And => And,
            TokenKind::Or => Or,
            TokenKind::GreaterThan => GreaterThan,
            TokenKind::LessThan => LessThan,
            TokenKind::Pipeline => Pipeline,
            _ => panic!("Cannot covert {:?} to BinOp", self),
        }
    }
}

impl Token {
    /// Creates a new `Token`
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Token { kind, span }
    }

    /// Returns whether this token is a newline
    pub fn is_newline(&self) -> bool {
        *self == TokenKind::Newline
    }

    pub fn follows_statement(&self) -> bool {
        match self.kind {
            TokenKind::EOF | TokenKind::RBrace => true,
            _ => false,
        }
    }

    pub fn precedence(&self) -> Precedence {
        use Precedence::*;
        use TokenKind::*;
        match &self.kind {
            LBrace | LParen => Prefix,
            Dot => Prefix,
            Equals => Assignment,
            // PlusEquals => ASSIGNMENT,
            // QuestionDot => ASSIGNMENT,
            // Question => CONDITIONAL,
            Plus => Sum,
            // TODO idk if this is the right precedence
            Or | And | Pipeline => Conditional,
            Minus => Sum,
            Star | Slash => Product,
            // Mul => PRODUCT,
            // Div => PRODUCT,
            // DblEquals => COMPARE,
            LessThan | GreaterThan => Compare,
            _ => None,
        }
    }
}

impl Debug for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Token")
            .field(&self.kind)
            .field(&self.span)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// The 'import' keyword
    Import,
    /// The 'let' keyword
    Let,
    /// The 'fn' keyword
    Fn,
    /// The 'state' keyword
    State,
    /// The 'component' keyword
    Component,
    /// The 'enum' keyword
    Enum,
    /// The 'struct' keyword
    Struct,
    /// The 'const' keyword
    Const,
    /// The 'for' keyword
    For,
    /// The 'if' keyword
    If,
    /// The 'else' keyword
    Else,
    /// The 'in' keyword
    In,
    /// The 'while' keyword
    While,
    /// The 'await' keyword
    Await,
    /// The 'async' keyword
    Async,
    /// The 'true' keyword
    True,
    /// The 'false' keyword
    False,
    /// The 'interface' keyword
    Interface,
    /// The 'pub' keyword
    Pub,
    /// The 'return' keyword
    Return,
    /// A user-defined identifier
    Identifier(Symbol),
    /// A string literggal
    String(Symbol),
    /// A number literal
    Number(Symbol),
    /// Represents a Unicode newline
    Newline,
    /// The '=' character
    Equals,
    /// The '.' character
    Dot,
    /// The ',' character
    Comma,
    /// The '(' character
    LParen,
    /// The ')' character
    RParen,
    /// The '{' character
    LBrace,
    /// The '}' character
    RBrace,
    /// The '[' character
    LBracket,
    /// The ']' character
    RBracket,
    /// The '*' character
    Star,
    /// The '+' character
    Plus,
    /// The '-' character
    Minus,
    /// The '/' character
    Slash,
    /// The ':' character
    Colon,
    /// The '<' character
    LessThan,
    /// The '>' character
    GreaterThan,
    /// The '|' character
    Pipe,
    /// The range operator, '..'S
    Range,
    /// Logical OR `||`
    Or,
    /// Logical AND `&&`
    And,
    /// Pipeline operator, `|>`,
    Pipeline,
    /// End-of-file
    EOF,
}

impl Display for TokenKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenKind::Import => write!(f, "import"),
            TokenKind::Let => write!(f, "let"),
            TokenKind::Fn => write!(f, "fn"),
            TokenKind::State => write!(f, "state"),
            TokenKind::Component => write!(f, "component"),
            TokenKind::Enum => write!(f, "enum"),
            TokenKind::Struct => write!(f, "struct"),
            TokenKind::Const => write!(f, "const"),
            TokenKind::For => write!(f, "for"),
            TokenKind::If => write!(f, "if"),
            TokenKind::Else => write!(f, "else"),
            TokenKind::In => write!(f, "in"),
            TokenKind::While => write!(f, "while"),
            TokenKind::Await => write!(f, "await"),
            TokenKind::Async => write!(f, "async"),
            TokenKind::True => write!(f, "true"),
            TokenKind::False => write!(f, "false"),
            TokenKind::Interface => write!(f, "interface"),
            TokenKind::Pub => write!(f, "pub"),
            TokenKind::Return => write!(f, "return"),
            // TODO implement Display for Symbol
            TokenKind::Identifier(sym) => write!(f, "{:?}", sym),
            TokenKind::String(sym) => write!(f, "\"{:?}\"", sym),
            TokenKind::Number(sym) => write!(f, "{:?}", sym),
            TokenKind::Newline => write!(f, "\\n"),
            TokenKind::Equals => write!(f, "="),
            TokenKind::Dot => write!(f, "."),
            TokenKind::Comma => write!(f, ","),
            TokenKind::LParen => write!(f, "("),
            TokenKind::RParen => write!(f, ")"),
            TokenKind::LBrace => write!(f, "{{"),
            TokenKind::RBrace => write!(f, "}}"),
            TokenKind::LBracket => write!(f, "["),
            TokenKind::RBracket => write!(f, "]"),
            TokenKind::Star => write!(f, "*"),
            TokenKind::Plus => write!(f, "+"),
            TokenKind::Minus => write!(f, "-"),
            TokenKind::Slash => write!(f, "/"),
            TokenKind::Colon => write!(f, ":"),
            TokenKind::LessThan => write!(f, "<"),
            TokenKind::GreaterThan => write!(f, ">"),
            TokenKind::Pipe => write!(f, "|"),
            TokenKind::Range => write!(f, ".."),
            TokenKind::EOF => write!(f, "EOF"),
            TokenKind::Or => write!(f, "||"),
            TokenKind::And => write!(f, "&&"),
            TokenKind::Pipeline => write!(f, "|>"),
        }
    }
}
