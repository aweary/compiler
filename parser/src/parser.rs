use core::panic;
use diagnostics::result::Result;
use lexer::Lexer;
use std::path::PathBuf;
use std::sync::Arc;
use std::vec;
use syntax::arena::{alloc_expression, alloc_function, with_mut_function};
use syntax::{
    arena::{AstArena, FunctionId, StatementId},
    ast::*,
    visit::Visitor,
    Precedence, Span, Token, TokenKind,
};
use vfs::FileSystem;

use crate::control_flow::ControlFlowAnalysis;
use common::scope_map::ScopeMap;
use common::symbol::Symbol;
use log::debug;
use types::Type;

#[salsa::query_group(ParserDatabase)]
pub trait Parser: FileSystem {
    fn parse(&self, path: PathBuf) -> Result<Module>;
}

/// Database query for parsing a path.
fn parse(db: &dyn Parser, path: PathBuf) -> Result<Module> {
    let source = db.file_text(path);
    let mut ast_arena = AstArena::default();
    let parser = ParserImpl::new(&source, &mut ast_arena);
    let module = parser.parse_module()?;
    // let mut cfg_analysis = ControlFlowAnalysis::new(&mut ast_arena);
    // cfg_analysis.visit_module(&mut module)?;
    // let cfg_map = cfg_analysis.finish();
    Ok(module)
}

/// Core data structure for the parser, creates the `Lexer` instance
/// and lazily creates and consumes tokens as it parses.
pub struct ParserImpl<'s> {
    lexer: Lexer<'s>,
    prev_span: Span,
    span: Span,
    is_newline_significant: bool,
    is_async_context: bool,
    is_component_context: bool,
    scope_map: ScopeMap<Symbol, Binding>,
    type_scope_map: ScopeMap<Symbol, TypeBinding>,
    allow_effect_reference: bool,
    pub ast_arena: &'s mut AstArena,
    reference_tracker: std::collections::HashSet<FunctionId>,
}

impl<'s> ParserImpl<'s> {
    /// Create a new instance of `ParseImpl`
    pub fn new(source: &'s str, ast_arena: &'s mut AstArena) -> Self {
        let lexer = Lexer::new(source);
        let scope_map = ScopeMap::default();
        let type_scope_map = ScopeMap::default();
        let span = Span::new(0, 0);
        Self {
            lexer,
            prev_span: span,
            span,
            is_newline_significant: false,
            is_async_context: false,
            is_component_context: false,
            scope_map,
            type_scope_map,
            allow_effect_reference: false,
            ast_arena,
            reference_tracker: std::collections::HashSet::new(),
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
        let prev_span = self.prev_span;
        if token != kind {
            use diagnostics::error::unexpected_token_error;
            unexpected_token_error(span, prev_span, kind, token.kind)
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
            self.prev_span = self.span;
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
                    }
                    self.expect(TokenKind::RBrace)?;
                    if self.eat(TokenKind::Dot)? {
                        let next_token = self.next()?;
                        use diagnostics::error::dot_after_import_list;
                        return dot_after_import_list(self.span.merge(next_token.span));
                    }
                    let part = ImportPart::Collection(collection);
                    parts.push(part);
                    // arly here as collections must occur at the end
                    // of an import path.
                    break;
                }
                _ => break,
            }
            if !self.eat(TokenKind::Dot)? {
                break;
            }
        }
        debug!("import parts {:#?}", parts);
        // Add the imported values to the local scope
        match parts.last().unwrap() {
            ImportPart::Module(ident) => {
                let binding = Binding::Import(ident.span);
                self.scope_map.define(ident.symbol, binding);
            }
            ImportPart::Collection(idents) => {
                for ident in idents {
                    let binding = Binding::Import(ident.span);
                    self.scope_map.define(ident.symbol, binding);
                }
            }
        };
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
                        let function = self.parse_function(true)?;
                        DefinitionKind::Function(function)
                        // ...
                    }
                    TokenKind::Component => {
                        let component = self.component(true)?;
                        DefinitionKind::Component(component)
                    }
                    _ => {
                        let token = self.next()?;
                        use diagnostics::error::unexpected_token_error;
                        return unexpected_token_error(
                            self.span,
                            self.prev_span,
                            TokenKind::Fn,
                            token.kind,
                        );
                    }
                }
            }
            TokenKind::Fn => {
                let function = self.parse_function(false)?;
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
                        let smybol = identifier.symbol;
                        let type_param = TypeParameter { name: identifier };
                        let type_param = Arc::new(type_param);
                        self.type_scope_map
                            .define(smybol, TypeBinding::TypeParameter(type_param.clone()));
                        identifiers.push(type_param);
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
                    use edit_distance::edit_distance;
                    let symbol_str = format!("{}", name.symbol);
                    let mut maybe_reference_span: Option<Span> = None;
                    for scope in self.type_scope_map.scope_iter() {
                        for (binding_symbol, (binding, _)) in &scope.bindings {
                            let binding_str = format!("{}", binding_symbol);
                            let distance = edit_distance(&binding_str, &symbol_str);
                            if distance <= 3 {
                                maybe_reference_span = Some(binding.span());
                            }
                        }
                    }
                    return unknown_type(span, &name.symbol, maybe_reference_span);
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
                TypeBinding::TypeParameter(type_param) => Type::Parameter(type_param.clone()),
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

    fn parse_expression_from_identifier(
        &mut self,
        symbol: Symbol,
        span: Span,
    ) -> Result<Expression> {
        match self.scope_map.resolve(&symbol) {
            Some((binding, _unique_reference)) => {
                if let Binding::Function(function_id) = binding {
                    self.reference_tracker.remove(&function_id);
                }
                let expr = Expression {
                    kind: ExpressionKind::Reference(binding.clone()),
                    span: span,
                    type_: None,
                };
                self.infix_expression(expr)
            }
            None => {
                // TODO move edit distance check into scope_map
                use edit_distance::edit_distance;
                let symbol_str = format!("{}", symbol);
                let mut maybe_reference_span: Option<Span> = None;
                let max_edit_distance = 2;
                for scope in self.scope_map.scope_iter() {
                    for (binding_symbol, (binding, _)) in &scope.bindings {
                        let binding_str = format!("{}", binding_symbol);
                        let distance = edit_distance(&binding_str, &symbol_str);
                        if distance <= max_edit_distance {
                            maybe_reference_span = Some(binding.span(&self.ast_arena));
                        }
                    }
                }
                return diagnostics::error::unknown_reference_error(
                    span,
                    symbol,
                    maybe_reference_span,
                );
            }
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
                self.parse_expression_from_identifier(symbol, token.span)
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
                use diagnostics::error::unexpected_token_for_expression;
                return unexpected_token_for_expression(token.span, self.prev_span);
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

    fn _dot(&mut self, left: Expression) -> Result<Expression> {
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

    fn arguments(&mut self) -> Result<Vec<Argument>> {
        debug!("Arguments");
        // Arguments can be positional like foo(bar) or named
        // like foo(bar: baz).
        #[derive(Debug, PartialEq, Eq)]
        enum CallFormat {
            Unknown,
            Named,
            Positional,
        }
        let mut arguments = vec![];
        let mut call_format = CallFormat::Unknown;

        if self.eat(TokenKind::RParen)? {
            return Ok(arguments);
        }

        loop {
            // End of arguments
            if let TokenKind::RParen = self.peek()?.kind {
                break;
            }
            // TODO can't parse as expression because we do name resolution here
            if let TokenKind::Identifier(_) = self.peek()?.kind {
                let name = self.identifier()?;
                if self.eat(TokenKind::Colon)? {
                    // Named argument
                    if call_format == CallFormat::Positional {
                        use diagnostics::error::named_argument_after_positional;
                        // Parse the next expression to include it in the error reporting
                        let expr = self.expression(Precedence::None)?;
                        let span = name.span.merge(expr.span);
                        return named_argument_after_positional(
                            span,
                            arguments.last().unwrap().span,
                        );
                    }
                    call_format = CallFormat::Named;
                    let value = self.expression(Precedence::None)?;
                    let span = name.span.merge(self.span);
                    let argument = Argument {
                        span,
                        name: Some(name),
                        value,
                    };
                    arguments.push(argument);
                } else {
                    // Positional argument
                    let expr = self.parse_expression_from_identifier(name.symbol, name.span)?;
                    if call_format == CallFormat::Named {
                        use diagnostics::error::positional_argument_after_named;
                        return positional_argument_after_named(
                            expr.span,
                            arguments.last().unwrap().span,
                        );
                    }
                    call_format = CallFormat::Positional;
                    let expr = self.parse_expression_from_identifier(name.symbol, name.span)?;
                    let argument = Argument {
                        span: expr.span,
                        name: None,
                        value: expr,
                    };
                    arguments.push(argument);
                }
            } else {
                let expr = self.expression(Precedence::None)?;
                if call_format == CallFormat::Named {
                    use diagnostics::error::positional_argument_after_named;
                    return positional_argument_after_named(
                        expr.span,
                        arguments.last().unwrap().span,
                    );
                }
                call_format = CallFormat::Positional;
                let argument = Argument {
                    span: expr.span,
                    name: None,
                    value: expr,
                };
                arguments.push(argument);
            }
            self.eat(TokenKind::Comma)?;
        }
        self.expect(TokenKind::RParen)?;
        Ok(arguments)
    }

    fn call_expression(&mut self, left: Expression) -> Result<Expression> {
        let span = left.span;
        self.expect(TokenKind::LParen)?;
        match left.kind {
            // Function call with a reference
            ExpressionKind::Reference(_) | ExpressionKind::Member { .. } => {
                let arguments = self.arguments()?;
                let span = span.merge(self.span);
                let call = Call {
                    callee: left.into(),
                    arguments,
                };
                if let TokenKind::LBrace = self.peek()?.kind {
                    // A bracket right after a function call is parsed as a view expression
                    let block = self.block()?;
                    let view = View {
                        constructor: call,
                        body: block,
                    };
                    Ok(Expression {
                        span,
                        kind: ExpressionKind::View(view.into()),
                        type_: None,
                    })
                } else {
                    let kind = ExpressionKind::Call(call);
                    let span = self.span.merge(span);
                    Ok(Expression {
                        kind,
                        span,
                        type_: None,
                    })
                }
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
        let span = self.span;
        let name = self.identifier()?;
        self.expect(TokenKind::Equals)?;
        let value = self.expression(Precedence::None)?;
        let symbol = name.symbol;
        let span = span.merge(value.span);
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

    fn statement(&mut self) -> Result<StatementId> {
        let statement = match self.peek()?.kind {
            TokenKind::Let => self.let_(),
            TokenKind::State => self.state(),
            TokenKind::Return => self.return_(),
            TokenKind::If => self.if_(),
            TokenKind::For => self.for_(),
            TokenKind::While => self.while_(),
            _ => self.expression_statement(),
        }?;
        let statement_id = self.ast_arena.statements.alloc(statement);
        Ok(statement_id)
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

    fn if_impl(&mut self) -> Result<If> {
        self.expect(TokenKind::If)?;
        let span = self.span;
        let condition = self.expression(Precedence::None)?;
        let body = self.block()?;
        let alternate = if self.eat(TokenKind::Else)? {
            if TokenKind::If == self.peek()?.kind {
                let if_ = self.if_impl()?;
                let alternate = Else::If(if_);
                Some(alternate.into())
            } else {
                let block = self.block()?;
                let alternate = Else::Block(block);
                Some(alternate.into())
            }
        } else {
            None
        };
        let span = self.span.merge(span);
        let condition = alloc_expression(condition);
        let if_ = If {
            span,
            condition,
            body,
            alternate,
        };
        Ok(if_)
    }

    fn if_(&mut self) -> Result<Statement> {
        let if_ = self.if_impl()?;
        Ok(Statement {
            span: if_.span,
            kind: StatementKind::If(if_),
        })
    }

    fn while_(&mut self) -> Result<Statement> {
        self.expect(TokenKind::While)?;
        let span = self.span;
        let condition = self.expression(Precedence::None)?;
        let condition = self.ast_arena.expressions.alloc(condition);
        let body = self.block()?;
        let span = self.span.merge(span);
        let while_ = While { condition, body };
        Statement::new(StatementKind::While(while_), span)
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

    fn parse_function(&mut self, is_async: bool) -> Result<FunctionId> {
        let prev_is_async_context = self.is_async_context;
        self.is_async_context = is_async;
        self.expect(TokenKind::Fn)?;
        self.scope_map.extend();
        let name = self.identifier()?;
        let symbol = name.symbol;

        let type_parameters = self.type_parameters()?;
        let parameters = self.parameters()?;
        let (return_type, effect_type) = self.type_and_effect_annotation()?;

        let function = Function {
            name,
            is_async,
            type_parameters,
            parameters,
            return_type,
            effect_type,
            body: None,
        };

        let function_id = self.ast_arena.functions.alloc(function);

        self.scope_map
            .define(symbol, Binding::Function(function_id));

        self.reference_tracker.insert(function_id);

        let body = self.block()?;

        self.ast_arena.functions.get_mut(function_id).unwrap().body = Some(body);

        self.scope_map.pop();
        self.is_async_context = prev_is_async_context;

        Ok(function_id)
    }

    fn component(&mut self, is_async: bool) -> Result<Arc<Component>> {
        self.is_component_context = true;
        let prev_is_async_context = self.is_async_context;
        self.is_async_context = is_async;
        self.expect(TokenKind::Component)?;
        self.scope_map.extend();
        let name = self.identifier()?;
        let symbol = name.symbol;
        let type_parameters = self.type_parameters()?;
        let parameters = self.parameters()?;
        let (return_type, effect_type) = self.type_and_effect_annotation()?;
        let body = self.block()?;
        self.scope_map.pop();
        self.is_async_context = prev_is_async_context;
        let component = Component {
            name,
            is_async,
            type_parameters,
            parameters,
            return_type,
            effect_type,
            body,
        };
        let component = Arc::new(component);
        self.scope_map
            .define(symbol, Binding::Component(component.clone()));
        self.is_component_context = false;
        Ok(component)
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
