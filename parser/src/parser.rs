use common::scope_map::ScopeMap;
use core::panic;
use diagnostics::result::Result;
use lexer::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use syntax::Symbol;
use syntax::{ast::*, Precedence, Span, Token, TokenKind};
use vfs::FileSystem;

use id_arena::Arena;
use syntax::visit::Visitor;
use types::Type;

use log::debug;



#[derive(Default)]
pub struct Resolver {}

impl Visitor for Resolver {
    fn visit_import(&mut self, import: &mut Import) -> Result<()> {
        debug!("Visiting import: {:#?}", import);
        Ok(())
    }

    fn visit_enum(&mut self, enum_: &mut Enum) -> Result<()> {
        debug!("Visiting enum: {:#?}", enum_);
        Ok(())
    }

    fn visit_struct(&mut self, struct_: &mut Struct) -> Result<()> {
        debug!("Visiting struct: {:#?}", struct_);
        Ok(())
    }
}

#[salsa::query_group(ParserDatabase)]
pub trait Parser: FileSystem {
    fn parse(&self, path: PathBuf) -> Result<Module>;
}

/// Database query for parsing a path.
fn parse(db: &dyn Parser, path: PathBuf) -> Result<Module> {
    let source = db.file_text(path);
    let parser = ParserImpl::new(&source);
    let mut module = parser.parse_module()?;
    Resolver::default().visit_module(&mut module)?;
    Ok(module)
}

/// Core data structure for the parser, creates the `Lexer` instance
/// and lazily creates and consumes tokens as it parses.
pub struct ParserImpl<'s> {
    lexer: Lexer<'s>,
    span: Span,
    is_newline_significant: bool,
    scope_map: ScopeMap,
    expressions: Arena<Expression>,
}

impl<'s> ParserImpl<'s> {
    /// Create a new instance of `ParseImpl`
    pub fn new(source: &'s str) -> Self {
        let lexer = Lexer::new(source);
        let scope_map = ScopeMap::default();
        let span = Span::new(0, 0);
        Self {
            lexer,
            span,
            is_newline_significant: false,
            scope_map,
            expressions: Arena::new(),
        }
    }

    /// Skip the next token and do nothing with it.
    fn skip(&mut self) -> Result<()> {
        self.next()?;
        Ok(())
    }

    /// Consume a token if it matches the provided `kind`,
    /// otherwise do nothing. Returns whether the token was consumed.
    fn eat(&mut self, kind: TokenKind) -> Result<bool> {
        debug!("eat {:?}", kind);
        if self.peek()? == &kind {
            self.skip()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Consume the next token, if it has the provided `kind`.
    /// If not, we throw an unexpected token error.
    fn expect(&mut self, kind: TokenKind) -> Result<Token> {
        let token = self.next()?;
        let span = self.span;
        if token != kind {
            use diagnostics::error::unexpected_token_error;
            unexpected_token_error(span, kind, token.kind)
        } else {
            Ok(token)
        }
    }

    /// Consume the next token from the lexer
    fn next(&mut self) -> Result<Token> {
        let token = self.lexer.next_token()?;
        debug!("next {:?}", token.kind);
        // Ignore newlines when they are not considered significant
        if token.is_newline() && !self.is_newline_significant {
            debug!("skipping newline");
            self.next()
        } else {
            self.span = token.span;
            Ok(token)
        }
    }

    /// Look at the next token without consuming it
    fn peek(&mut self) -> Result<&Token> {
        let token_kind = &self.lexer.peek()?.kind;
        debug!("peek {:#?}", token_kind);
        // Ignore newlines when they are not considered significant
        if token_kind == &TokenKind::Newline && !self.is_newline_significant {
            self.lexer.next_token()?;
            self.peek()
        } else {
            self.lexer.peek()
        }
    }

    /// Primary public API for the `ParseImpl`. Parses all
    /// imports and definitions in a module, which is currently assumed
    /// to be a single file.
    pub fn parse_module(mut self) -> Result<Module> {
        let imports = self.imports()?;
        let definitions = self.definitions()?;
        let module = Module::new(imports, definitions);
        Ok(module)
    }

    /// Parses all imports at the top of a module. We currently require
    /// that all imports are grouped together at the top of the module.
    fn imports(&mut self) -> Result<Vec<Import>> {
        debug!("imports");
        let mut imports = vec![];
        loop {
            match self.peek()?.kind {
                TokenKind::Import => {
                    debug!("found an import");
                    let import = self.import()?;
                    imports.push(import)
                }
                _ => break,
            }
        }
        Ok(imports)
    }

    /// Parse a single import statement
    fn import(&mut self) -> Result<Import> {
        debug!("import");
        self.expect(TokenKind::Import)?;
        let mut parts = vec![];
        loop {
            match self.peek()?.kind {
                // Single import or module
                TokenKind::Identifier(_) => {
                    let ident = self.identifier()?;
                    let part = ImportPart::Module(ident);
                    parts.push(part)
                }
                // Collection of imports
                TokenKind::LBrace => {
                    self.skip()?;
                    let mut collection = vec![];
                    loop {
                        match self.peek()?.kind {
                            TokenKind::Identifier(_) => {
                                let ident = self.identifier()?;
                                collection.push(ident);
                                self.eat(TokenKind::Comma)?;
                            }
                            _ => break,
                        }
                        if self.eat(TokenKind::RBrace)? {
                            break;
                        }
                    }
                    if self.eat(TokenKind::Dot)? {
                        let next_token = self.next()?;
                        use diagnostics::error::dot_after_import_list;
                        return dot_after_import_list(self.span.merge(next_token.span));
                    }
                    let part = ImportPart::Collection(collection);
                    parts.push(part);
                    // arly here as collections must occur at the end
                    // of an import path.
                    return Ok(Import::new(parts));
                }
                _ => break,
            }
            if !self.eat(TokenKind::Dot)? {
                break;
            }
        }
        debug!("import parts {:#?}", parts);
        Ok(Import::new(parts))
    }

    /// Parse an identifier
    fn identifier(&mut self) -> Result<Identifier> {
        let token = self.next()?;
        match token.kind {
            TokenKind::Identifier(symbol) => Ok(Identifier::new(symbol, token.span)),
            _ => {
                use diagnostics::error::expected_identifier;
                expected_identifier(token.span, token.kind)
            }
        }
    }

    /// Parse a single definition in a module
    fn definition(&mut self, is_public: bool) -> Result<Definition> {
        // Public exports
        if self.eat(TokenKind::Pub)? {
            return self.definition(true);
        }

        let kind = match self.peek()?.kind {
            TokenKind::Fn => {
                let function = self.function()?;
                DefinitionKind::Function(function)
            }
            TokenKind::Component => {
                let component = self.component()?;
                DefinitionKind::Component(component)
            }
            TokenKind::Enum => {
                let enum_ = self.enum_()?;
                debug!("enum: {:#?}", enum_);
                DefinitionKind::Enum(enum_)
            }
            TokenKind::Struct => {
                let struct_ = self.struct_()?;
                DefinitionKind::Struct(struct_)
            }
            TokenKind::Const => {
                let const_ = self.const_()?;
                DefinitionKind::Const(const_)
            }
            // TokenKind::EOF => return Ok(definitions),
            _ => {
                use diagnostics::error::unexpected_token_error_with_multiple_options;
                let token = self.next()?;
                return unexpected_token_error_with_multiple_options(
                    token.span,
                    vec![
                        TokenKind::Fn,
                        TokenKind::Component,
                        TokenKind::Enum,
                        TokenKind::Struct,
                    ],
                    token.kind,
                )
                .map_err(|err| err);
            }
        };
        Ok(Definition { is_public, kind })
    }

    /// Parse all definitions in a module
    fn definitions(&mut self) -> Result<Vec<Definition>> {
        let mut definitions = vec![];
        loop {
            if let TokenKind::EOF = self.peek()?.kind {
                break;
            } else {
                // TODO support checking for visibility modifiers here
                let definition = self.definition(false)?;
                definitions.push(definition)
            }
        }
        Ok(definitions)
    }

    fn type_parameters(&mut self) -> Result<Option<TypeParameters>> {
        use TokenKind::{Comma, GreaterThan, Identifier, LessThan};
        if self.eat(LessThan)? {
            let mut identifiers = vec![];
            let lo = self.span;
            loop {
                match self.peek()?.kind {
                    Identifier(_) => {
                        let identifier = self.identifier()?;
                        identifiers.push(identifier);
                        self.eat(Comma)?;
                    }
                    _ => break,
                }
            }
            self.expect(GreaterThan)?;
            let span = lo.merge(self.span);
            if identifiers.is_empty() {
                use diagnostics::error::empty_type_parameters;
                return empty_type_parameters(span);
            }
            Ok(Some(TypeParameters { identifiers, span }))
        } else {
            Ok(None)
        }
    }

    /// Parses a type reference, like what you would see in type
    /// annotations. It does not parse type _definitions_.
    fn type_(&mut self) -> Result<TypeExpression> {
        let name = self.identifier()?;
        let arguments = if self.eat(TokenKind::LessThan)? {
            let mut arguments = vec![];
            loop {
                if let TokenKind::Identifier(_) = self.peek()?.kind {
                    let type_ = self.type_()?;
                    arguments.push(type_);
                    self.eat(TokenKind::Comma)?;
                } else {
                    break;
                }
            }
            self.expect(TokenKind::GreaterThan)?;
            Some(arguments)
        } else {
            None
        };
        Ok(TypeExpression { name, arguments })
    }

    /// Parses a function or component parameter.
    fn parameter(&mut self) -> Result<Parameter> {
        let name = self.identifier()?;
        let type_ = if self.eat(TokenKind::Colon)? {
            Some(self.type_()?)
        } else {
            None
        };
        Ok(Parameter { name, type_ })
    }

    fn parameters(&mut self) -> Result<Option<Vec<Parameter>>> {
        use TokenKind::{Comma, LParen, RParen};
        if self.eat(LParen)? {
            let mut parameters = vec![];
            loop {
                if let TokenKind::Identifier(_) = self.peek()?.kind {
                    let parameter = self.parameter()?;
                    parameters.push(parameter);
                    self.eat(Comma)?;
                } else {
                    break;
                }
            }
            self.expect(RParen)?;
            if parameters.is_empty() {
                Ok(None)
            } else {
                Ok(Some(parameters))
            }
        } else {
            Ok(None)
        }
    }

    fn prefix_expression(&mut self) -> Result<Expression> {
        let token = self.next()?;
        match token.kind {
            TokenKind::Number(symbol) => Ok(Expression {
                kind: ExpressionKind::Number {
                    raw: symbol,
                    value: None,
                },
                span: token.span,
                type_: Some(Type::Number),
            }),
            TokenKind::String(symbol) => Ok(Expression {
                kind: ExpressionKind::String { raw: symbol },
                span: token.span,
                type_: Some(Type::String),
            }),
            TokenKind::True => Ok(Expression {
                kind: ExpressionKind::Boolean(true),
                span: token.span,
                type_: Some(Type::Boolean),
            }),
            // TODO(aweary) how to dedupe with `true`?
            TokenKind::False => Ok(Expression {
                kind: ExpressionKind::Boolean(false),
                span: token.span,
                type_: Some(Type::Boolean),
            }),
            TokenKind::Identifier(symbol) => {
                match self.scope_map.resolve(&symbol) {
                    Some(binding) => {
                        let kind = ExpressionKind::Reference(binding);
                    }
                    None => {
                        use diagnostics::error::unknown_reference_error;
                        return unknown_reference_error(token.span, symbol);
                    }
                }
                panic!("lol")
            }
            _ => panic!("Unknown token for expression"),
        }
    }

    fn binary_expression(&mut self, left: Expression) -> Result<Expression> {
        let (op, precedence) = {
            let token = self.next()?;
            let precedence = token.precedence();
            let op: BinOp = token.into();
            (op, precedence)
        };
        let right = self.expression(precedence)?;
        let span = left.span.merge(right.span);
        Ok(Expression {
            span,
            kind: ExpressionKind::Binary {
                left: left.into(),
                right: right.into(),
                op,
            },
            type_: None,
        })
    }

    fn dot(&mut self, left: Expression) -> Result<Expression> {
        self.expect(TokenKind::Dot)?;
        let right = self.expression(Precedence::Prefix)?;
        match (left.kind, right.kind) {
            (
                ExpressionKind::Number { raw: left_raw, .. },
                ExpressionKind::Number { raw: right_raw, .. },
            ) => {
                let raw = Symbol::intern(&format!("{:?}.{:?}", left_raw, right_raw));
                let span = left.span.merge(right.span);
                let expression = Expression {
                    span,
                    kind: ExpressionKind::Number { raw, value: None },
                    type_: Some(Type::Number),
                };
                Ok(expression)
            }
            _ => {
                todo!("Other dot expressions")
                // ...
            }
        }
    }

    fn infix_expression(&mut self, prefix: Expression) -> Result<Expression> {
        use TokenKind::*;
        match self.peek()?.kind {
            Plus | Minus | Star | Slash => self.binary_expression(prefix),
            Dot => self.dot(prefix),
            _ => Ok(prefix),
        }
    }

    fn expression(&mut self, precedence: Precedence) -> Result<Expression> {
        let mut expression = self.prefix_expression()?;
        debug!("PREFIX {:#?}", expression);
        while precedence < self.peek()?.precedence() {
            expression = self.infix_expression(expression)?;
        }
        Ok(expression)
    }

    fn let_(&mut self) -> Result<Statement> {
        self.expect(TokenKind::Let)?;
        let name = self.identifier()?;
        self.expect(TokenKind::Equals)?;
        let value = self.expression(Precedence::None)?;
        let symbol = name.symbol;
        let span = name.span.merge(value.span);
        let unique_name = self.scope_map.unique_name();
        let let_ = Let {
            name,
            value,
            unique_name,
        };
        let let_ = Arc::new(let_);
        let binding = Binding::Let(let_.clone());
        self.scope_map.define(symbol, binding);
        Ok(Statement {
            kind: StatementKind::Let(let_),
            span,
        })
    }

    fn statement(&mut self) -> Result<Statement> {
        match self.peek()?.kind {
            // Let statement, e.g., `let a = 1`
            TokenKind::Let => self.let_(),
            _ => panic!("unknown"),
        }
    }

    fn block(&mut self) -> Result<Block> {
        self.expect(TokenKind::LBrace)?;
        let mut statements = vec![];
        while !self.peek()?.follows_statement() {
            let statement = self.statement()?;
            statements.push(statement);
        }
        self.expect(TokenKind::RBrace)?;
        Ok(Block { statements })
    }

    /// Functions and components can be annotated with a return value type as well
    /// as an effect type.
    fn type_and_effect_annotation(&mut self) -> Result<(Option<TypeExpression>, Option<Effect>)> {
        let annotations = if self.eat(TokenKind::Colon)? {
            let type_ = self.type_()?;
            let effect = if self.eat(TokenKind::Plus)? {
                let effect_type = self.type_()?;
                let effect = Effect(effect_type);
                Some(effect)
            } else {
                None
            };
            (Some(type_), effect)
        } else {
            (None, None)
        };
        Ok(annotations)
    }

    fn function(&mut self) -> Result<Function> {
        self.expect(TokenKind::Fn)?;
        let name = self.identifier()?;
        let type_parameters = self.type_parameters()?;
        let parameters = self.parameters()?;
        let (return_type, effect_type) = self.type_and_effect_annotation()?;
        let body = self.block()?;
        Ok(Function {
            name,
            type_parameters,
            parameters,
            return_type,
            effect_type,
            body,
        })
    }

    fn component(&mut self) -> Result<Component> {
        self.expect(TokenKind::Fn)?;
        let name = self.identifier()?;
        Ok(Component { name })
    }

    fn enum_variant(&mut self) -> Result<Variant> {
        let name = self.identifier()?;
        let types = if self.eat(TokenKind::LParen)? {
            let mut types = vec![];
            loop {
                if let TokenKind::Identifier(_) = self.peek()?.kind {
                    let type_ = self.type_()?;
                    types.push(type_);
                    self.eat(TokenKind::Comma)?;
                } else {
                    break;
                }
            }
            self.expect(TokenKind::RParen)?;
            Some(types)
        } else {
            None
        };
        Ok(Variant { name, types })
    }

    fn enum_(&mut self) -> Result<Enum> {
        use TokenKind::{LBrace, RBrace};
        self.expect(TokenKind::Enum)?;
        let name = self.identifier()?;
        let type_parameters = self.type_parameters()?;
        let mut variants = vec![];
        self.expect(LBrace)?;
        loop {
            if let TokenKind::Identifier(_) = self.peek()?.kind {
                let variant = self.enum_variant()?;
                variants.push(variant);
            } else {
                break;
            }
        }
        self.expect(RBrace)?;
        Ok(Enum {
            name,
            type_parameters,
            variants,
        })
    }

    fn struct_(&mut self) -> Result<Struct> {
        self.expect(TokenKind::Struct)?;
        let name = self.identifier()?;
        let type_parameters = self.type_parameters()?;
        self.expect(TokenKind::LBrace)?;
        let fields = self.struct_fields()?;
        self.expect(TokenKind::RBrace)?;
        Ok(Struct {
            name,
            type_parameters,
            fields,
        })
    }

    fn const_(&mut self) -> Result<Const> {
        self.expect(TokenKind::Const)?;
        let name = self.identifier()?;
        let type_ = if self.eat(TokenKind::Colon)? {
            Some(self.type_()?)
        } else {
            None
        };
        self.expect(TokenKind::Equals)?;
        let value = self.expression(Precedence::None)?;
        Ok(Const { name, type_, value })
    }

    fn struct_fields(&mut self) -> Result<Vec<StructField>> {
        let mut fields = vec![];
        loop {
            let name = self.identifier()?;
            self.expect(TokenKind::Colon)?;
            let type_ = self.type_()?;
            let field = StructField { name, type_ };
            fields.push(field);
            if let TokenKind::Identifier(_) = self.peek()?.kind {
                continue;
            } else {
                break;
            }
        }
        Ok(fields)
    }
}
