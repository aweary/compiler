use common::scope_map::ScopeMap;
use common::symbol::Symbol;
use core::panic;
use diagnostics::result::Result;
use lexer::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use std::vec;
use syntax::{ast::*, Precedence, Span, Token, TokenKind};
use vfs::FileSystem;

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

    fn visit_enum(&mut self, enum_: &mut Arc<Enum>) -> Result<()> {
        debug!("Visiting enum: {:#?}", enum_);
        Ok(())
    }

    fn visit_struct(&mut self, struct_: &mut Arc<Struct>) -> Result<()> {
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
    is_async_context: bool,
    scope_map: ScopeMap<Symbol, Binding>,
    type_scope_map: ScopeMap<Symbol, TypeBinding>,
    allow_effect_reference: bool,
}

impl<'s> ParserImpl<'s> {
    /// Create a new instance of `ParseImpl`
    pub fn new(source: &'s str) -> Self {
        let lexer = Lexer::new(source);
        let scope_map = ScopeMap::default();
        let type_scope_map = ScopeMap::default();
        let span = Span::new(0, 0);
        Self {
            lexer,
            span,
            is_newline_significant: false,
            is_async_context: false,
            scope_map,
            type_scope_map,
            allow_effect_reference: false,
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
        let mut imports = vec![];
        loop {
            match self.peek()?.kind {
                TokenKind::Import => {
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
            TokenKind::Async => {
                self.eat(TokenKind::Async)?;
                match self.peek()?.kind {
                    TokenKind::Fn => {
                        let function = self.function(true)?;
                        DefinitionKind::Function(function)
                        // ...
                    }
                    TokenKind::Component => {
                        let component = self.component(true)?;
                        DefinitionKind::Component(component)
                    }
                    _ => {
                        panic!("Invalid async definition")
                    }
                }
            }
            TokenKind::Fn => {
                let function = self.function(false)?;
                DefinitionKind::Function(function)
            }
            TokenKind::Component => {
                let component = self.component(false)?;
                DefinitionKind::Component(component)
            }
            TokenKind::Enum => {
                let enum_ = self.enum_()?;
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
            TokenKind::Type => {
                let type_def = self.type_def()?;
                DefinitionKind::Type(type_def)
            }
            TokenKind::Effect => {
                let effect = self.effect_def()?;
                DefinitionKind::Effect(effect)
            }
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

    fn type_(&mut self) -> Result<TypeExpression> {
        // Parse function parameters for types like (a: string, b: int) => int
        if self.eat(TokenKind::LParen)? {
            let span = self.span;
            let mut parameters = vec![];
            loop {
                if self.peek()?.kind == TokenKind::RParen {
                    break;
                }
                let type_ = self.type_()?;
                parameters.push(type_);
                if self.eat(TokenKind::Comma)? {
                    continue;
                } else {
                    break;
                }
            }
            self.expect(TokenKind::RParen)?;
            self.expect(TokenKind::Arrow)?;
            let return_type = self.type_()?.into();
            let span = span.merge(self.span);
            let kind = TypeExpressionKind::Function {
                parameters,
                return_type,
            };
            return Ok(TypeExpression {
                kind,
                span,
                type_: None,
            });
        }
        let type_ = self.reference_type()?;
        let span = type_.span;
        // Parse a function type
        if self.eat(TokenKind::Arrow)? {
            let parameters = vec![type_];
            let return_type = self.type_()?.into();
            let kind = TypeExpressionKind::Function {
                parameters,
                return_type,
            };
            let span = self.span.merge(span);
            let type_ = TypeExpression {
                kind,
                span,
                type_: None,
            };
            Ok(type_)
        } else {
            Ok(type_)
        }
    }

    /// Parses a type reference, like what you would see in type
    /// annotations. It does not parse type _definitions_.
    fn reference_type(&mut self) -> Result<TypeExpression> {
        let name = self.identifier()?;
        let span = self.span;
        // How can we represent user types at this point? We don't want the types crate to depend on the AST
        let type_ = match self.type_scope_map.resolve(&name.symbol) {
            None => match name.symbol.to_string().as_str() {
                "bool" => Type::Boolean,
                "number" => Type::Number,
                "string" => Type::String,
                "void" => Type::Unit,
                _ => {
                    use diagnostics::error::unknown_type;
                    return unknown_type(span, &name.symbol);
                }
            },
            Some((binding, _)) => match binding {
                TypeBinding::Struct(struct_) => Type::Struct(struct_.clone()),
                TypeBinding::Effect(effect) => {
                    if self.allow_effect_reference {
                        Type::Effect(effect.clone())
                    } else {
                        use diagnostics::error::invalid_effect_reference;
                        return invalid_effect_reference(span, effect.name.symbol);
                    }
                }
            },
        };
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
        let span = self.span.merge(span);
        let kind = TypeExpressionKind::Reference { name, arguments };
        let type_ = TypeExpression {
            kind,
            span,
            type_: Some(type_),
        };
        Ok(type_)
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

    fn parameters(&mut self) -> Result<Option<Vec<Arc<Parameter>>>> {
        use TokenKind::{Comma, LParen, RParen};
        if self.eat(LParen)? {
            let mut parameters = vec![];
            loop {
                if let TokenKind::Identifier(symbol) = self.peek()?.kind {
                    let parameter = self.parameter()?;
                    let parameter = Arc::new(parameter);
                    self.scope_map
                        .define(symbol, Binding::Parameter(parameter.clone()));
                    parameters.push(parameter);
                    if self.eat(Comma)? {
                        // Another parameter, continue
                        continue;
                    } else {
                        // Expect the end of the params list
                        break;
                    }
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
        match self.peek()?.kind {
            TokenKind::Number(symbol) => {
                let token = self.next()?;
                Ok(Expression {
                    kind: ExpressionKind::Number {
                        raw: symbol,
                        value: None,
                    },
                    span: token.span,
                    type_: Some(Type::Number),
                })
            }
            TokenKind::String(symbol) => {
                let token = self.next()?;
                Ok(Expression {
                    kind: ExpressionKind::String { raw: symbol },
                    span: token.span,
                    type_: Some(Type::String),
                })
            }
            TokenKind::True | TokenKind::False => {
                let token = self.next()?;
                Ok(Expression {
                    kind: ExpressionKind::Boolean(token.kind == TokenKind::True),
                    span: token.span,
                    type_: Some(Type::Boolean),
                })
            }
            TokenKind::Identifier(symbol) => {
                let token = self.next()?;
                match self.scope_map.resolve(&symbol) {
                    Some((binding, _unique_reference)) => Ok(Expression {
                        kind: ExpressionKind::Reference(binding.clone()),
                        span: token.span,
                        type_: None,
                    }),
                    None => diagnostics::error::unknown_reference_error(token.span, symbol),
                }
            }
            TokenKind::LBracket => self.array_expression(),
            TokenKind::LBrace => self.block_expression(),
            TokenKind::Match => self.match_expression(),
            TokenKind::LParen => {
                self.expect(TokenKind::LParen)?;
                let expression = self.expression(Precedence::None)?;
                self.expect(TokenKind::RParen)?;
                Ok(expression)
            }
            TokenKind::Await => {
                self.expect(TokenKind::Await)?;
                if !self.is_async_context {
                    use diagnostics::error::invalid_await;
                    return invalid_await(self.span);
                }
                let span = self.span;
                let expression = self.expression(Precedence::None)?;
                let span = span.merge(expression.span);
                let kind = ExpressionKind::Await(expression.into());
                Ok(Expression {
                    kind,
                    span,
                    type_: None,
                })
            }
            _ => {
                let token = self.next()?;
                panic!("Unknown token for expression, {:#?}", token.kind);
            }
        }
    }

    fn match_expression(&mut self) -> Result<Expression> {
        self.expect(TokenKind::Match)?;
        let span = self.span;
        let value = self.expression(Precedence::None)?;
        let cases = self.match_cases()?;
        let span = self.span.merge(span);
        Ok(Expression {
            kind: ExpressionKind::Match {
                value: value.into(),
                cases,
            },
            span,
            type_: None,
        })
    }

    fn match_cases(&mut self) -> Result<Vec<MatchCase>> {
        self.expect(TokenKind::LBrace)?;
        let mut cases = vec![];
        let mut wildcard_span = None;
        loop {
            if let TokenKind::RBrace = self.peek()?.kind {
                break;
            }
            let pattern = match self.peek()?.kind {
                TokenKind::Underscore => {
                    self.skip()?;
                    if let Some(span) = wildcard_span {
                        use diagnostics::error::duplicate_wildcard_error;
                        return duplicate_wildcard_error(span, self.span);
                    }
                    wildcard_span = Some(self.span);
                    MatchPattern::Wildcard
                }
                _ => {
                    let expression = self.expression(Precedence::None)?;
                    // Check if a wildcard has already been used and warn
                    // about it since further cases will be unreachable.
                    if let Some(span) = wildcard_span {
                        use diagnostics::error::unreachable_match_case;
                        return unreachable_match_case(self.span, span);
                    }
                    MatchPattern::Expression(expression.into())
                }
            };
            self.expect(TokenKind::Arrow)?;
            let body = self.expression(Precedence::None)?.into();
            cases.push(MatchCase { pattern, body });
        }
        self.expect(TokenKind::RBrace)?;
        Ok(cases)
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

    fn member_expression(&mut self, left: Expression) -> Result<Expression> {
        self.expect(TokenKind::Dot)?;
        let name = self.identifier()?;
        let span = left.span.merge(name.span);
        Ok(Expression {
            span,
            kind: ExpressionKind::Member {
                object: left.into(),
                property: name,
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

    fn arguments(&mut self) -> Result<Vec<Expression>> {
        let mut arguments = vec![];
        loop {
            match self.peek()?.kind {
                // End of argument list
                TokenKind::RParen => break,
                _ => {
                    let argument = self.expression(Precedence::None)?;
                    arguments.push(argument);
                    if self.eat(TokenKind::Comma)? {
                        continue;
                    } else {
                        break;
                    }
                }
            }
        }
        Ok(arguments)
    }

    fn call_expression(&mut self, left: Expression) -> Result<Expression> {
        let span = left.span;
        self.expect(TokenKind::LParen)?;
        match left.kind {
            // Function call with a reference
            ExpressionKind::Reference(_) | ExpressionKind::Member { .. } => {
                let arguments = self.arguments()?;
                let kind = ExpressionKind::Call {
                    callee: left.into(),
                    arguments,
                };
                // TODO arguments
                self.expect(TokenKind::RParen)?;
                let span = self.span.merge(span);
                Ok(Expression {
                    kind,
                    span,
                    type_: None,
                })
            }
            _ => {
                use diagnostics::error::illegal_function_callee;
                illegal_function_callee(left.span)
            }
        }
    }

    fn infix_expression(&mut self, prefix: Expression) -> Result<Expression> {
        use TokenKind::*;
        match self.peek()?.kind {
            Plus | Minus | Star | Slash | LessThan | GreaterThan | DoubleEquals | And | BinAnd => {
                self.binary_expression(prefix)
            }
            Equals => self.assignment_expression(prefix),
            LParen => self.call_expression(prefix),
            Dot => self.member_expression(prefix),
            Range => self.range_expression(prefix),
            _ => Ok(prefix),
        }
    }

    fn block_expression(&mut self) -> Result<Expression> {
        let span = self.span;
        let block = self.block()?;
        let kind = ExpressionKind::Block(block);
        Ok(Expression {
            kind,
            span,
            type_: None,
        })
    }

    fn assignment_expression(&mut self, left: Expression) -> Result<Expression> {
        self.expect(TokenKind::Equals)?;
        match left.kind {
            ExpressionKind::Reference(_) | ExpressionKind::Member { .. } => {
                let right = self.expression(Precedence::Assignment)?;
                let span = left.span.merge(right.span);
                let kind = ExpressionKind::Assignment {
                    left: left.into(),
                    right: right.into(),
                };
                Ok(Expression {
                    kind,
                    span,
                    type_: None,
                })
            }
            _ => {
                use diagnostics::error::illegal_assignment_target;
                illegal_assignment_target(left.span)
            }
        }
    }

    // range_expression parses a range expression like `1..10` or `1..20`
    fn range_expression(&mut self, start: Expression) -> Result<Expression> {
        self.expect(TokenKind::Range)?;
        let end = self.expression(Precedence::None)?;
        let span = start.span.merge(end.span);
        let kind = ExpressionKind::Range {
            start: start.into(),
            end: end.into(),
        };
        Ok(Expression {
            kind,
            span,
            type_: None,
        })
    }

    fn expression(&mut self, precedence: Precedence) -> Result<Expression> {
        let mut expression = self.prefix_expression()?;
        while precedence < self.peek()?.precedence() {
            expression = self.infix_expression(expression)?;
        }
        Ok(expression)
    }

    fn array_expression(&mut self) -> Result<Expression> {
        self.expect(TokenKind::RBracket)?;
        let mut elements = vec![];
        let span = self.span;
        loop {
            match self.peek()?.kind {
                TokenKind::RBracket => break,
                _ => {
                    let element = self.expression(Precedence::None)?;
                    elements.push(element);
                    if self.eat(TokenKind::Comma)? {
                        continue;
                    } else {
                        break;
                    }
                }
            }
        }
        self.expect(TokenKind::RBracket)?;
        let kind = ExpressionKind::Array(elements);
        let span = self.span.merge(span);
        Ok(Expression {
            kind,
            span,
            type_: None,
        })
    }

    fn let_(&mut self) -> Result<Statement> {
        self.expect(TokenKind::Let)?;
        let name = self.identifier()?;
        self.expect(TokenKind::Equals)?;
        let value = self.expression(Precedence::None)?;
        let symbol = name.symbol;
        let span = name.span.merge(value.span);
        // let unique_name = self.scope_map.unique_name();
        let let_ = Let {
            name,
            value,
            // TODO
            unique_name: UniqueName::from(0),
        };
        let let_ = Arc::new(let_);
        let binding = Binding::Let(let_.clone());
        self.scope_map.define(symbol, binding);
        Ok(Statement {
            kind: StatementKind::Let(let_),
            span,
        })
    }

    fn state(&mut self) -> Result<Statement> {
        self.expect(TokenKind::State)?;
        let name = self.identifier()?;
        self.expect(TokenKind::Equals)?;
        let value = self.expression(Precedence::None)?;
        let symbol = name.symbol;
        let span = name.span.merge(value.span);
        // let unique_name = self.scope_map.unique_name();
        let state = State {
            name,
            value,
            // TODO
            unique_name: UniqueName::from(0),
        };
        let state = Arc::new(state);
        let binding = Binding::State(state.clone());
        self.scope_map.define(symbol, binding);
        Ok(Statement {
            kind: StatementKind::State(state),
            span,
        })
    }

    fn return_(&mut self) -> Result<Statement> {
        self.expect(TokenKind::Return)?;
        let span = self.span;
        let value = self.expression(Precedence::None)?;
        let span = span.merge(value.span);
        Ok(Statement {
            kind: StatementKind::Return(value),
            span,
        })
    }

    fn statement(&mut self) -> Result<Statement> {
        match self.peek()?.kind {
            TokenKind::Let => self.let_(),
            TokenKind::State => self.state(),
            TokenKind::Return => self.return_(),
            TokenKind::If => self.if_(),
            TokenKind::For => self.for_(),
            _ => self.expression_statement(),
        }
    }

    fn expression_statement(&mut self) -> Result<Statement> {
        let expression = self.expression(Precedence::None)?;
        let span = expression.span;
        Ok(Statement {
            kind: StatementKind::Expression(expression),
            span,
        })
    }

    // Parse a for-in statement like for x in y { ... }
    fn for_(&mut self) -> Result<Statement> {
        self.expect(TokenKind::For)?;
        let span = self.span;
        let iterator = self.identifier()?;
        let symbol = iterator.symbol;
        self.scope_map
            .define(symbol, Binding::Iterator(iterator.clone()));
        self.expect(TokenKind::In)?;
        let iterable = self.expression(Precedence::None)?;
        let body = self.block()?;
        let for_ = For {
            iterator,
            iterable,
            body,
        };
        let span = self.span.merge(span);
        Ok(Statement {
            kind: StatementKind::For(for_),
            span,
        })
    }

    fn if_(&mut self) -> Result<Statement> {
        self.expect(TokenKind::If)?;
        let span = self.span;
        // self.expect(TokenKind::LParen)?;
        let condition = self.expression(Precedence::None)?;
        // self.expect(TokenKind::RParen)?;
        let body = self.block()?;
        let _if = If { body, condition };
        let span = self.span.merge(span);
        Ok(Statement {
            span,
            kind: StatementKind::If(_if),
        })
    }

    fn block(&mut self) -> Result<Block> {
        self.expect(TokenKind::LBrace)?;
        let mut statements = vec![];
        self.scope_map.extend();
        while !self.peek()?.follows_statement() {
            let statement = self.statement()?;
            statements.push(statement);
        }
        self.expect(TokenKind::RBrace)?;
        self.scope_map.pop();
        Ok(Block { statements })
    }

    /// Functions and components can be annotated with a return value type as well
    /// as an effect type.
    fn type_and_effect_annotation(&mut self) -> Result<(Option<TypeExpression>, Option<Effect>)> {
        let annotations = if self.eat(TokenKind::Colon)? {
            let type_ = self.type_()?;
            let effect = if self.eat(TokenKind::Plus)? {
                let allow_effect_reference = self.allow_effect_reference;
                self.allow_effect_reference = true;
                let effect_type = self.type_()?;
                self.allow_effect_reference = allow_effect_reference;
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

    fn function(&mut self, is_async: bool) -> Result<Arc<Function>> {
        let prev_is_async_context = self.is_async_context;
        self.is_async_context = is_async;
        self.expect(TokenKind::Fn)?;
        self.scope_map.extend();
        let name = self.identifier()?;
        let symbol = name.symbol;
        let type_parameters = self.type_parameters()?;
        let parameters = self.parameters()?;
        let (return_type, effect_type) = self.type_and_effect_annotation()?;
        let body = self.block()?;
        self.scope_map.pop();
        self.is_async_context = prev_is_async_context;
        let function = Function {
            name,
            is_async,
            type_parameters,
            parameters,
            return_type,
            effect_type,
            body,
        };
        let function = Arc::new(function);
        self.scope_map
            .define(symbol, Binding::Function(function.clone()));
        Ok(function)
    }

    fn component(&mut self, is_async: bool) -> Result<Component> {
        let prev_is_async_context = self.is_async_context;
        self.is_async_context = is_async;
        self.expect(TokenKind::Component)?;
        self.scope_map.extend();
        let name = self.identifier()?;
        let type_parameters = self.type_parameters()?;
        let parameters = self.parameters()?;
        let (return_type, effect_type) = self.type_and_effect_annotation()?;
        let body = self.block()?;
        self.scope_map.pop();
        self.is_async_context = prev_is_async_context;
        Ok(Component {
            name,
            is_async,
            type_parameters,
            parameters,
            return_type,
            effect_type,
            body,
        })
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

    fn enum_(&mut self) -> Result<Arc<Enum>> {
        use TokenKind::{LBrace, RBrace};
        self.expect(TokenKind::Enum)?;
        let name = self.identifier()?;
        let symbol = name.symbol;
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
        let enum_ = Enum {
            name,
            type_parameters,
            variants,
        };
        let enum_ = Arc::new(enum_);
        self.scope_map.define(symbol, Binding::Enum(enum_.clone()));
        Ok(enum_)
    }

    fn struct_(&mut self) -> Result<Arc<Struct>> {
        self.expect(TokenKind::Struct)?;
        let name = self.identifier()?;
        let symbol = name.symbol;
        let type_parameters = self.type_parameters()?;
        self.expect(TokenKind::LBrace)?;
        let fields = self.struct_fields()?;
        self.expect(TokenKind::RBrace)?;
        let struct_ = Struct {
            name,
            type_parameters,
            fields,
        };
        let struct_ = Arc::new(struct_);
        self.type_scope_map
            .define(symbol, TypeBinding::Struct(struct_.clone()));
        Ok(struct_)
    }

    fn const_(&mut self) -> Result<Arc<Const>> {
        self.expect(TokenKind::Const)?;
        let name = self.identifier()?;
        let symbol = name.symbol;
        let type_ = if self.eat(TokenKind::Colon)? {
            Some(self.type_()?)
        } else {
            None
        };
        self.expect(TokenKind::Equals)?;
        let value = self.expression(Precedence::None)?;
        let const_ = Arc::new(Const { name, type_, value });
        self.scope_map
            .define(symbol, Binding::Const(const_.clone()));
        Ok(const_)
    }

    fn type_def(&mut self) -> Result<Arc<TypeDef>> {
        debug!("type_def");
        self.expect(TokenKind::Type)?;
        let span = self.span;
        let name = self.identifier()?;
        debug!("type_def {:?}", name);
        self.expect(TokenKind::Equals)?;
        let type_ = self.type_()?;
        let span = span.merge(self.span);
        let type_def = Arc::new(TypeDef { name, type_, span });
        Ok(type_def)
    }

    fn effect_def(&mut self) -> Result<Arc<EffectDef>> {
        debug!("effect_def");
        self.expect(TokenKind::Effect)?;
        let span = self.span;
        let name = self.identifier()?;
        let symbol = name.symbol;
        debug!("effect_def {:?}", name);
        let effect_def = Arc::new(EffectDef { name, span });
        self.type_scope_map
            .define(symbol, TypeBinding::Effect(effect_def.clone()));
        Ok(effect_def)
    }

    fn struct_fields(&mut self) -> Result<Vec<StructField>> {
        let mut fields = vec![];
        loop {
            if let TokenKind::Identifier(_) = self.peek()?.kind {
                let name = self.identifier()?;
                self.expect(TokenKind::Colon)?;
                let type_ = self.type_()?;
                let field = StructField { name, type_ };
                fields.push(field);
            } else {
                break;
            }
        }
        Ok(fields)
    }
}
