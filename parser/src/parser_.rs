use common::{scope_map::ScopeMap, symbol::Symbol};
use diagnostics::result::Result;
use lexer::{Lexer, LexingMode};
use log::debug;
use syntax::{ast::BinOp, ast_::*, visit_::Visitor, Precedence, Span, Token, TokenKind};

use std::{collections::HashMap, path::PathBuf};
use vfs::FileSystem;

use crate::evaluate::ExpressionEvaluator;

use crate::control_flow::{CFGKey, ControlFlowAnalysis};

use codegen::Codegen;

#[salsa::query_group(ParserDatabase)]
pub trait Parser: FileSystem {
    fn parse(&self, path: PathBuf) -> Result<()>;
}

/// Database query for parsing a path.
fn parse(db: &dyn Parser, path: PathBuf) -> Result<()> {
    let source = db.file_text(path);
    let mut arena = AstArena::default();
    let mut parser = ParserImpl::new(&source, &mut arena);
    let module_id = parser.parse_module()?;
    // Evaluate step
    {
        let evaluate = ExpressionEvaluator::new(&mut arena);

        evaluate.visit_module(module_id)?;
        // We want to do constant propagation before we do control flow analysis.
        // That way we can populate known values in call expressions and generate
        // control flow graphs that have annotated return value data.
        // That way we support constant functions, where we can statically determine
        // the return value of a function and inline.

        let cfg_analysis = ControlFlowAnalysis::new(&mut arena);
        cfg_analysis.visit_module(module_id)?;
        let cfg_map = cfg_analysis.finish();
        let mut codegen = Codegen::new("main".to_string(), &mut arena);

        for (key, cfg) in cfg_map.iter() {
            match key {
                CFGKey::Function(function_id) => {
                    codegen.codegen_function(*function_id, cfg)?;
                }
                CFGKey::Component(component_id) => {
                    codegen.codegen_component(*component_id, cfg)?;
                    // ...
                }
            }
        }

        // Path should be fixtures/output.js from the project root, absolute
        let path = PathBuf::from("fixtures/output.js");

        println!("Writing to {:?}", path);
        codegen.write(path)?;
    }
    Ok(())
}

pub struct ParserImpl<'source, 'ctx> {
    lexer: Lexer<'source>,
    ctx: &'ctx mut AstArena,
    span: Span,
    prev_span: Span,
    spans: HashMap<ExpressionId, Span>,
    scope_map: ScopeMap<Symbol, Binding>,
}

impl<'source, 'ctx> ParserImpl<'source, 'ctx> {
    pub fn new(source: &'source str, ctx: &'ctx mut AstArena) -> Self {
        let start_span = Span::new(0, 0);
        Self {
            lexer: Lexer::new(source),
            ctx,
            span: start_span,
            prev_span: start_span,
            spans: HashMap::default(),
            scope_map: ScopeMap::default(),
        }
    }

    pub fn parse_module(&mut self) -> Result<ModuleId> {
        let mut definitions = vec![];

        while self.peek()?.kind != TokenKind::EOF {
            let definition = self.parse_definition()?;
            definitions.push(definition);
        }

        let module = Module { definitions };
        let module_id = self.ctx.modules.alloc(module);
        Ok(module_id)
    }

    fn parse_definition(&mut self) -> Result<Definition> {
        // let is_public = self.eat(TokenKind::Pub)?;
        match self.peek()?.kind {
            TokenKind::Fn => {
                let function_id = self.parse_function()?;
                Ok(Definition::Function(function_id))
            }
            TokenKind::Const => {
                let const_id = self.parse_const()?;
                Ok(Definition::Const(const_id))
            }
            TokenKind::Component => {
                let component_id = self.parse_component()?;
                Ok(Definition::Component(component_id))
            }
            TokenKind::Enum => {
                todo!("enum")
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

    fn parse_const(&mut self) -> Result<ConstId> {
        self.expect(TokenKind::Const)?;
        let name = self.identifier()?;
        let symbol = name.symbol;
        self.expect(TokenKind::Equals)?;
        let value = self.parse_expression(Precedence::None)?;
        let const_ = Const { name, value };
        let const_ = self.ctx.consts.alloc(const_);
        self.scope_map.define(symbol, Binding::Const(const_));
        Ok(const_)
    }

    fn parse_type(&mut self) -> Result<Type> {
        // Parse function parameters for types like (a: string, b: int) => int
        if self.eat(TokenKind::LParen)? {
            let mut parameters = vec![];
            loop {
                if self.peek()?.kind == TokenKind::RParen {
                    break;
                }
                let type_ = self.parse_type()?;
                parameters.push(type_);
                if self.eat(TokenKind::Comma)? {
                    continue;
                } else {
                    break;
                }
            }
            self.expect(TokenKind::RParen)?;
            self.expect(TokenKind::Arrow)?;
            let return_type = self.parse_type()?.into();
            return Ok(Type::Function {
                parameters,
                return_type,
            });
        }

        match self.peek()?.kind {
            TokenKind::Boolean => {
                self.expect(TokenKind::Boolean)?;
                return Ok(Type::Boolean);
            }
            TokenKind::NumberType => {
                self.expect(TokenKind::NumberType)?;
                return Ok(Type::Number);
            }
            TokenKind::StringType => {
                self.expect(TokenKind::StringType)?;
                return Ok(Type::String);
            }
            _ => {
                todo!()
            }
        }
    }

    fn parameter(&mut self) -> Result<Parameter> {
        let name = self.identifier()?;
        let type_ = if self.eat(TokenKind::Colon)? {
            Some(self.parse_type()?)
        } else {
            None
        };
        Ok(Parameter { name, type_ })
    }

    fn parse_parameters(&mut self) -> Result<Option<Vec<ParameterId>>> {
        use TokenKind::{Comma, LParen, RParen};
        if self.eat(LParen)? {
            let mut parameters = vec![];
            loop {
                if let TokenKind::Identifier(symbol) = self.peek()?.kind {
                    let parameter = self.parameter()?;
                    let parameter_id = self.ctx.parameters.alloc(parameter);
                    self.scope_map
                        .define(symbol, Binding::Parameter(parameter_id));
                    parameters.push(parameter_id);
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

    fn parse_function(&mut self) -> Result<FunctionId> {
        self.expect(TokenKind::Fn)?;
        let name = self.identifier()?;
        let symbol = name.symbol;
        let parameters = self.parse_parameters()?;
        let function = Function {
            body: None,
            name,
            parameters,
        };
        let function_id = self.ctx.alloc_function(function);
        self.scope_map
            .define(symbol, Binding::Function(function_id));
        let body = self.parse_block()?;
        let function = self.ctx.functions.get_mut(function_id).unwrap();
        let mut function = function.borrow_mut();
        function.body = Some(body);

        Ok(function_id)
    }

    fn parse_component(&mut self) -> Result<ComponentId> {
        self.expect(TokenKind::Component)?;
        let name = self.identifier()?;
        let symbol = name.symbol;
        let parameters = self.parse_parameters()?;
        let component = Component {
            body: None,
            name,
            parameters,
        };
        let component_id = self.ctx.alloc_component(component);
        self.scope_map
            .define(symbol, Binding::Component(component_id));
        let body = self.parse_block()?;
        let component = self.ctx.components.get_mut(component_id).unwrap();
        let mut component = component.borrow_mut();
        component.body = Some(body);
        Ok(component_id)
    }

    fn parse_block(&mut self) -> Result<BlockId> {
        self.expect(TokenKind::LBrace)?;
        let mut statements = vec![];
        self.scope_map.extend();
        while !self.peek()?.follows_statement() {
            let statement = self.parse_statement()?;
            statements.push(statement);
        }

        self.expect(TokenKind::RBrace)?;
        self.scope_map.pop();
        let block = Block { statements };
        let block_id = self.ctx.blocks.alloc(block);
        Ok(block_id)
    }

    fn parse_statement(&mut self) -> Result<StatementId> {
        match self.peek()?.kind {
            TokenKind::Let => self.parse_let(),
            TokenKind::State => self.parse_state(),
            TokenKind::Return => self.parse_return(),
            TokenKind::If => self.parse_if(),
            TokenKind::While => self.parse_while(),
            _ => todo!(),
        }
    }

    fn parse_while(&mut self) -> Result<StatementId> {
        self.expect(TokenKind::While)?;
        let condition = self.parse_expression(Precedence::None)?;
        let body = self.parse_block()?;
        let statement_id = self
            .ctx
            .statements
            .alloc(Statement::While { condition, body });
        Ok(statement_id)
    }

    fn parse_if(&mut self) -> Result<StatementId> {
        let if_ = self.parse_if_impl()?;
        let statement = Statement::If(if_);
        let statement_id = self.ctx.statements.alloc(statement);
        Ok(statement_id)
    }

    fn parse_if_impl(&mut self) -> Result<If> {
        self.expect(TokenKind::If)?;
        let condition = self.parse_expression(Precedence::None)?;
        let body = self.parse_block()?;
        let alternate = if self.eat(TokenKind::Else)? {
            if TokenKind::If == self.peek()?.kind {
                let if_ = self.parse_if_impl()?;
                let alternate = Else::If(if_);
                Some(alternate.into())
            } else {
                let block = self.parse_block()?;
                let alternate = Else::Block(block);
                Some(alternate.into())
            }
        } else {
            None
        };
        let if_ = If {
            condition,
            body,
            alternate,
        };
        Ok(if_)
    }

    fn parse_return(&mut self) -> Result<StatementId> {
        self.expect(TokenKind::Return)?;
        let value = self.parse_expression(Precedence::None)?;
        let return_ = Statement::Return(value);
        let return_id = self.ctx.statements.alloc(return_);
        Ok(return_id)
    }

    fn parse_let(&mut self) -> Result<StatementId> {
        self.expect(TokenKind::Let)?;
        let name = self.identifier()?;
        let symbol = name.symbol;
        self.expect(TokenKind::Equals)?;
        let value = self.parse_expression(Precedence::None)?;
        let let_ = Statement::Let { name, value };
        let let_id = self.ctx.statements.alloc(let_);
        self.scope_map.define(symbol, Binding::Let(let_id));
        Ok(let_id)
    }

    fn parse_state(&mut self) -> Result<StatementId> {
        self.expect(TokenKind::State)?;
        let name = self.identifier()?;
        let symbol = name.symbol;
        self.expect(TokenKind::Equals)?;
        let value = self.parse_expression(Precedence::None)?;
        let state = Statement::State { name, value };
        let state_id = self.ctx.statements.alloc(state);
        self.scope_map.define(symbol, Binding::State(state_id));
        Ok(state_id)
    }

    fn parse_expression(&mut self, precedence: Precedence) -> Result<ExpressionId> {
        let mut expression = self.parse_prefix_expression()?;
        while precedence < self.peek()?.precedence() {
            expression = self.parse_infix_expression(expression)?;
        }
        Ok(expression)
    }

    fn binary_expression(&mut self, left: ExpressionId) -> Result<ExpressionId> {
        let (op, precedence) = {
            let token = self.next()?;
            let precedence = token.precedence();
            let op: BinOp = token.into();
            (op, precedence)
        };
        let right = self.parse_expression(precedence)?;
        let expression = Expression::Binary { left, op, right };
        Ok(self.ctx.alloc_expression(expression))
    }

    fn call_expression(&mut self, callee_id: ExpressionId) -> Result<ExpressionId> {
        let callee = self.ctx.expressions.get(callee_id).unwrap();
        let callee = callee.borrow();
        match *callee {
            Expression::Reference(_) => {
                std::mem::drop(callee);
                let arguments = self.parse_arguments()?;
                let expression = Expression::Call {
                    callee: callee_id,
                    arguments,
                };
                let expression_id = self.ctx.alloc_expression(expression);
                Ok(expression_id)
            }
            _ => {
                todo!()
            }
        }
        // TODO
        // - call graph
        // - evaluate to see if we can inline
    }

    fn parse_arguments(&mut self) -> Result<Vec<Argument>> {
        self.expect(TokenKind::LParen)?;
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
                        // use diagnostics::error::named_argument_after_positional;
                        // Parse the next expression to include it in the error reporting
                        // let expr = self.parse_expression(Precedence::None)?;
                        panic!("TODO");
                        // let span = name.span.merge(expr.span);
                        // return named_argument_after_positional(
                        //     span,
                        //     arguments.last().unwrap().span,
                        // );
                    }
                    call_format = CallFormat::Named;
                    let value = self.parse_expression_from_identifier(name.symbol, name.span)?;
                    // let span = name.span.merge(self.span);
                    let argument = Argument {
                        name: Some(name),
                        value,
                    };
                    arguments.push(argument);
                } else {
                    // Positional argument
                    let _expr = self.parse_expression_from_identifier(name.symbol, name.span)?;
                    if call_format == CallFormat::Named {
                        todo!()
                        // use diagnostics::error::positional_argument_after_named;
                        // return positional_argument_after_named(
                        //     expr.span,
                        //     arguments.last().unwrap().span,
                        // );
                    }
                    call_format = CallFormat::Positional;
                    let expr = self.parse_expression_from_identifier(name.symbol, name.span)?;
                    let argument = Argument {
                        name: None,
                        value: expr,
                    };
                    arguments.push(argument);
                }
            } else {
                let expr = self.parse_expression(Precedence::None)?;
                if call_format == CallFormat::Named {
                    // use diagnostics::error::positional_argument_after_named;
                    todo!()
                    // return positional_argument_after_named(
                    //     expr.span,
                    //     arguments.last().unwrap().span,
                    // );
                }
                call_format = CallFormat::Positional;
                let argument = Argument {
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

    fn parse_infix_expression(&mut self, prefix: ExpressionId) -> Result<ExpressionId> {
        use TokenKind::*;
        match self.peek()?.kind {
            Plus | Minus | Star | Slash | LessThan | LessThanEquals | GreaterThan
            | GreaterThanEquals | DoubleEquals | And | BinAnd => self.binary_expression(prefix),
            LParen => self.call_expression(prefix),
            // Equals => self.assignment_expression(prefix),
            // Dot => self.member_expression(prefix),
            // Range => self.range_expression(prefix),
            _ => Ok(prefix),
        }
    }

    fn parse_prefix_expression(&mut self) -> Result<ExpressionId> {
        match self.peek()?.kind {
            // Boolean expressions
            TokenKind::True | TokenKind::False => {
                let token = self.next()?;
                let value = TokenKind::True == token.kind;
                let expression_id = self.ctx.alloc_expression(Expression::Boolean(value));
                self.spans.insert(expression_id, token.span);
                Ok(expression_id)
            }
            // Numeric expressions
            TokenKind::Number(raw_value) => {
                self.next()?;
                let value: f64 = raw_value.into();
                let expression_id = self.ctx.alloc_expression(Expression::Number(value));
                self.spans.insert(expression_id, self.prev_span);
                Ok(expression_id)
            }
            TokenKind::String(symbol) => {
                self.next()?;
                let expression_id = self.ctx.alloc_expression(Expression::String(symbol));
                self.spans.insert(expression_id, self.prev_span);
                Ok(expression_id)
            }
            // References
            TokenKind::Identifier(symbol) => {
                let token = self.next()?;
                self.parse_expression_from_identifier(symbol, token.span)
            }
            TokenKind::LessThan => {
                self.expect(TokenKind::LessThan)?;
                let template = self.parse_template()?;
                let expression_id = self.ctx.alloc_expression(Expression::Template(template));
                Ok(expression_id)
            }
            TokenKind::LParen => {
                self.next()?;
                let span = self.span;
                let expression_id = self.parse_expression(Precedence::None)?;
                self.expect(TokenKind::RParen)?;
                let span = span.merge(self.prev_span);
                self.spans.insert(expression_id, span);
                Ok(expression_id)
            }
            _ => {
                println!("NOPE {:?}", self.peek()?);
                todo!()
            }
        }
    }

    fn parse_template(&mut self) -> Result<TemplateId> {
        let open_tag = self.parse_template_open_tag()?;
        debug!("parse_template: open_tag = {:#?}", open_tag);
        if self.peek()?.kind == TokenKind::Slash {
            debug!("parse_template: self-closing tag");
            self.next()?;
            self.expect(TokenKind::GreaterThan)?;
            let template = Template {
                open_tag,
                close_tag: None,
                children: None,
            };
            let template_id = self.ctx.alloc_template(template);
            return Ok(template_id);
        }
        self.expect(TokenKind::GreaterThan)?;
        self.lexer.set_mode(LexingMode::TemplateText);
        let (template_children, close_tag) = self.parse_template_children_and_close_tag()?;
        debug!(
            "parse_template: template_children = {:#?}",
            template_children
        );
        debug!("parse_template: close_tag = {:#?}", close_tag);
        let template = Template {
            open_tag,
            close_tag: Some(close_tag),
            children: Some(template_children),
        };
        let template_id = self.ctx.alloc_template(template);
        Ok(template_id)
    }

    fn parse_template_children_and_close_tag(
        &mut self,
    ) -> Result<(Vec<TemplateChild>, TemplateCloseTag)> {
        let mut children = Vec::new();
        let mut close_tag = None;
        loop {
            match self.peek()?.kind {
                TokenKind::TemplateString(symbol) => {
                    debug!(
                        "parse_template_children_and_close_tag: TemplateString({})",
                        symbol
                    );
                    self.skip()?;
                    // TODO don't think this is the right way to handle whitespace
                    if symbol.to_string().is_empty() {
                        continue;
                    }
                    let child = TemplateChild::String(symbol);
                    children.push(child);
                }
                TokenKind::LBrace => {
                    self.expect(TokenKind::LBrace)?;
                    self.lexer.set_mode(LexingMode::Normal);
                    let expression = self.parse_expression(Precedence::None)?;
                    self.lexer.set_mode(LexingMode::TemplateText);
                    self.expect(TokenKind::RBrace)?;
                    let child = TemplateChild::Expression(expression);
                    children.push(child);
                }
                TokenKind::LessThan => {
                    self.lexer.set_mode(LexingMode::Normal);
                    self.expect(TokenKind::LessThan)?;
                    if self.eat(TokenKind::Slash)? {
                        // This is a close tag, not a nested template
                        let name = self.identifier()?;
                        debug!(
                            "parse_template_children_and_close_tag: closing tag for </{}>",
                            name.symbol
                        );

                        self.expect(TokenKind::GreaterThan)?;
                        close_tag = Some(TemplateCloseTag { name });
                        break;
                    } else {
                        debug!("parse_template_children_and_close_tag: nested template");
                        let template_id = self.parse_template()?;
                        self.lexer.set_mode(LexingMode::TemplateText);
                        let child = TemplateChild::Template(template_id);
                        children.push(child);
                    }
                }
                _ => {
                    break;
                }
            }
        }
        debug!(
            "parse_template_children_and_close_tag: children = {:#?}",
            children
        );
        debug!(
            "parse_template_children_and_close_tag: close_tag = {:#?}",
            close_tag
        );
        // TODO better error message for missing close tag
        Ok((children, close_tag.expect("should have parsed close tag")))
    }

    fn parse_template_open_tag(&mut self) -> Result<TemplateOpenTag> {
        let name = self.identifier()?;
        let attributes = self.parse_template_attributes()?;
        let open_tag = TemplateOpenTag { name, attributes };
        Ok(open_tag)
    }

    fn parse_template_attributes(&mut self) -> Result<Vec<TemplateAttribute>> {
        let mut attributes = vec![];
        loop {
            if self.peek()?.kind == TokenKind::GreaterThan || self.peek()?.kind == TokenKind::Slash
            {
                break;
            }
            let template_attribute = self.parse_template_attribute()?;
            attributes.push(template_attribute);
        }
        Ok(attributes)
    }

    fn parse_template_attribute(&mut self) -> Result<TemplateAttribute> {
        // We allow keywords here
        let name = self.identifier_loose()?;
        self.expect(TokenKind::Equals)?;
        // TODO I don't think this is the right precedence
        match self.peek()?.kind {
            TokenKind::String(_) | TokenKind::True | TokenKind::False => {
                let value = self.parse_expression(Precedence::Prefix)?;
                let template_attribute = TemplateAttribute { name, value: value };
                Ok(template_attribute)
            }
            _ => {
                self.expect(TokenKind::LBrace)?;
                let value = self.parse_expression(Precedence::None)?;
                self.expect(TokenKind::RBrace)?;
                let template_attribute = TemplateAttribute { name, value };
                Ok(template_attribute)
            }
        }
    }

    fn parse_expression_from_identifier(
        &mut self,
        symbol: Symbol,
        span: Span,
    ) -> Result<ExpressionId> {
        if let Some((binding, _)) = self.scope_map.resolve(&symbol) {
            let expression = Expression::Reference(*binding);
            let expression_id = self.ctx.alloc_expression(expression);
            self.parse_infix_expression(expression_id)
        } else {
            // TODO move edit distance check into scope_map
            use edit_distance::edit_distance;
            let symbol_str = format!("{}", symbol);
            let maybe_reference_span: Option<Span> = None;
            let max_edit_distance = 2;
            for scope in self.scope_map.scope_iter() {
                for (binding_symbol, (_, _)) in &scope.bindings {
                    let binding_str = format!("{}", binding_symbol);
                    let distance = edit_distance(&binding_str, &symbol_str);
                    if distance <= max_edit_distance {
                        // maybe_reference_span = match binding {
                        // }
                    }
                }
            }
            return diagnostics::error::unknown_reference_error(span, symbol, maybe_reference_span);
        }
    }

    /// Parse an identifier
    fn identifier(&mut self) -> Result<Identifier> {
        let token = self.next()?;
        match token.kind {
            TokenKind::Identifier(symbol) => Ok(Identifier {
                symbol,
                span: token.span,
            }),
            _ => {
                use diagnostics::error::expected_identifier;
                expected_identifier(token.span, token.kind)
            }
        }
    }

    // Allows identifiers that are keywords
    fn identifier_loose(&mut self) -> Result<Identifier> {
        let token = self.next()?;
        match token.kind {
            TokenKind::Identifier(symbol) => Ok(Identifier {
                symbol,
                span: token.span,
            }),
            TokenKind::Type => Ok(Identifier {
                symbol: Symbol::intern("type"),
                span: token.span,
            }),
            _ => {
                use diagnostics::error::expected_identifier;
                expected_identifier(token.span, token.kind)
            }
        }
    }

    /// Consume a token if it matches the provided `kind`,
    /// otherwise do nothing. Returns whether the token was consumed.
    fn eat(&mut self, kind: TokenKind) -> Result<bool> {
        if self.peek()? == &kind {
            self.skip()?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    /// Skip the next token and do nothing with it.
    fn skip(&mut self) -> Result<()> {
        self.next()?;
        Ok(())
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

    /// Look at the next token without consuming it
    fn peek(&mut self) -> Result<&Token> {
        let token_kind = &self.lexer.peek()?.kind;
        // Ignore newlines when they are not considered significant
        if token_kind == &TokenKind::Newline {
            self.lexer.next_token()?;
            self.peek()
        } else {
            self.lexer.peek()
        }
    }

    /// Consume the next token from the lexer
    fn next(&mut self) -> Result<Token> {
        let token = self.lexer.next_token()?;
        // Ignore newlines when they are not considered significant
        if token.is_newline() {
            self.next()
        } else {
            self.prev_span = self.span;
            self.span = token.span;
            Ok(token)
        }
    }
}
//
