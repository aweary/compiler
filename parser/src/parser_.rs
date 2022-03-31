use common::{scope_map::ScopeMap, symbol::Symbol};
use diagnostics::result::Result;
use lexer::Lexer;
use syntax::{ast::BinOp, ast_::*, visit_::Visitor, Precedence, Span, Token, TokenKind};

use std::{collections::HashMap, path::PathBuf};
use vfs::FileSystem;

use crate::evaluate::ExpressionEvaluator;

use crate::control_flow::ControlFlowAnalysis;

use codegen::codegen_from_cfg;

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
    let evaluate = ExpressionEvaluator::new(&mut arena);

    evaluate.visit_module(module_id)?;

    let cfg_analysis = ControlFlowAnalysis::new(&mut arena);

    cfg_analysis.visit_module(module_id)?;

    let map = cfg_analysis.finish();

    for (_function_id, cfg) in map {
        codegen_from_cfg(&cfg)?;
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
        let symbol = name.name;
        self.expect(TokenKind::Equals)?;
        let value = self.parse_expression(Precedence::None)?;
        let const_ = Const { name, value };
        let const_ = self.ctx.consts.alloc(const_);
        self.scope_map.define(symbol, Binding::Const(const_));
        Ok(const_)
    }

    fn parse_function(&mut self) -> Result<FunctionId> {
        self.expect(TokenKind::Fn)?;
        let name = self.identifier()?;
        self.expect(TokenKind::LParen)?;
        self.expect(TokenKind::RParen)?;
        let body = self.parse_block()?;
        let function = Function { body, name };
        let function_id = self.ctx.alloc_function(function);
        Ok(function_id)
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
        let symbol = name.name;
        self.expect(TokenKind::Equals)?;
        let value = self.parse_expression(Precedence::None)?;
        let let_ = Statement::Let { name, value };
        let let_id = self.ctx.statements.alloc(let_);
        self.scope_map.define(symbol, Binding::Let(let_id));
        Ok(let_id)
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

    fn call_expression(&mut self, callee: ExpressionId) -> Result<ExpressionId> {
        todo!()
    }

    fn parse_infix_expression(&mut self, prefix: ExpressionId) -> Result<ExpressionId> {
        use TokenKind::*;
        match self.peek()?.kind {
            Plus | Minus | Star | Slash | LessThan | GreaterThan | DoubleEquals | And | BinAnd => {
                self.binary_expression(prefix)
            }
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
            // References
            TokenKind::Identifier(symbol) => {
                let token = self.next()?;
                self.parse_expression_from_identifier(symbol, token.span)
            }
            _ => {
                println!("cant {:?}", self.peek()?);
                todo!()
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
            Ok(expression_id)
        } else {
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
                name: symbol,
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
