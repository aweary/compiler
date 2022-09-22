use crate::TokenStream;
use common::symbol::Symbol;
use diagnostics::error::{invalid_character, multiple_decimal_in_number};
use diagnostics::result::Result;
use std::collections::VecDeque;
use std::iter::{Iterator, Peekable};
use std::str::CharIndices;
use syntax::span::Span;
use syntax::token::{Token, TokenKind};
use unicode_xid::UnicodeXID;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LexingMode {
    Normal,
    TemplateTag,
    TemplateText,
}

pub struct Lexer<'s> {
    source: &'s str,
    chars: Peekable<CharIndices<'s>>,
    lookahead: VecDeque<Token>,
    mode: LexingMode,
}

impl<'s> Lexer<'s> {
    pub fn new(source: &'s str) -> Self {
        let chars = source.char_indices().peekable();
        Lexer {
            chars,
            source,
            lookahead: VecDeque::with_capacity(2),
            mode: LexingMode::Normal,
        }
    }

    pub fn set_mode(&mut self, mode: LexingMode) {
        self.mode = mode;
    }

    pub fn lex(mut self) -> Result<TokenStream> {
        let mut tokens = TokenStream::for_source(&self.source);
        loop {
            let token = self.next_token()?;
            if token.kind == TokenKind::EOF {
                break;
            } else {
                tokens.push(token)
            }
        }
        Ok(tokens)
    }

    fn skip(&mut self) {
        self.chars.next();
    }

    fn skip_while<F>(&mut self, pred: F)
    where
        F: Fn(&char) -> bool,
    {
        loop {
            match self.chars.peek() {
                Some((_, ch)) if pred(ch) => {
                    self.skip();
                }
                _ => return,
            }
        }
    }

    fn skip_whitespace(&mut self) {
        self.skip_while(|char| char != &'\n' && char.is_whitespace());
    }

    fn skip_newlines(&mut self) {
        self.skip_while(|char| char.is_whitespace());
    }

    pub fn next_token(&mut self) -> Result<Token> {
        use TokenKind::*;
        // Read from the lookahead if its populated.
        if let Some(token) = self.lookahead.pop_front() {
            return Ok(token);
        }
        if self.mode == LexingMode::TemplateText {
            self.skip_newlines();
            return self.template_text();
        }
        self.skip_whitespace();
        let char = self.chars.peek();
        match char {
            Some((_, ch)) if ch.is_digit(10) => self.number(),
            Some((_, ch)) if ch.is_xid_start() => self.identifier(),
            Some((_, '#')) => self.comment(),
            Some((_, '"')) => self.string(),
            Some((_, '.')) => self.dot(),
            Some((_, '&')) => self.and(),
            Some((_, ',')) => self.punc(Comma),
            Some((_, '=')) => self.equals(),
            Some((_, '(')) => self.punc(LParen),
            Some((_, ')')) => self.punc(RParen),
            Some((_, '{')) => self.punc(LBrace),
            Some((_, '}')) => self.punc(RBrace),
            Some((_, '[')) => self.punc(LBracket),
            Some((_, ']')) => self.punc(RBracket),
            Some((_, '+')) => self.punc(Plus),
            Some((_, '-')) => self.punc(Minus),
            Some((_, '/')) => self.punc(Slash),
            Some((_, '*')) => self.punc(Star),
            Some((_, ':')) => self.punc(Colon),
            Some((_, '<')) => self.less_than(),
            Some((_, '>')) => self.greater_than(),
            Some((_, '|')) => self.punc(Pipe),
            Some((_, '_')) => self.punc(Underscore),
            Some((_, '\n')) => self.punc(Newline),
            None => {
                let index = self.source.len() - 1;
                let span = Span::new(index as u32, index as u32);
                Ok(Token {
                    span,
                    kind: TokenKind::EOF,
                })
            }
            Some((i, _)) => {
                let span = Span::from(*i);
                invalid_character(span)
            }
        }
    }

    pub fn peek(&mut self) -> Result<&Token> {
        if self.lookahead.is_empty() {
            let token = self.next_token()?;
            self.lookahead.push_front(token);
        }
        Ok(self.lookahead.front().unwrap())
    }

    fn template_text(&mut self) -> Result<Token> {
        match self.chars.peek() {
            Some((_, '<')) => self.punc(TokenKind::LessThan),
            Some((_, '>')) => self.punc(TokenKind::GreaterThan),
            Some((_, '{')) => self.punc(TokenKind::LBrace),
            Some((_, '}')) => self.punc(TokenKind::RBrace),
            _ => {
                let (start, _) = self.chars.next().unwrap();
                let mut end = start;
                while let Some((i, ch)) = self.chars.peek() {
                    match ch {
                        '{' | '}' | '<' | '>' => {
                            break;
                        }
                        _ => {
                            end = *i;
                            self.skip();
                        }
                    }
                }
                let span = Span::new(start as u32, end as u32);
                let word = &self.source[start..end + 1];
                // TODO dont think this is the right way to handle whitespace
                // let word = word.trim();
                let symbol = Symbol::intern(word);
                let kind = TokenKind::TemplateString(symbol);
                let token = Token::new(kind, span);
                Ok(token)
            }
        }
    }

    /// We don't create tokens for comments at the moment. This
    /// method will just skip all the characters it sees until it encounters
    /// a newline and then attempt to return the next token
    fn comment(&mut self) -> Result<Token> {
        self.skip_while(|ch| ch != &'\n');
        self.next_token()
    }

    fn string(&mut self) -> Result<Token> {
        let (start, _) = self.chars.next().unwrap();
        let mut end = start;
        while let Some((i, ch)) = self.chars.next() {
            if ch == '"' {
                end = i;
                break;
            } else if ch == '\n' {
                break;
            }
        }
        if start == end {
            use diagnostics::error::unterminated_string;
            let span = Span::from(start);
            return unterminated_string(span);
        }
        let span = Span::new(start as u32, end as u32);
        let word = &self.source[start + 1..end];
        let symbol = Symbol::intern(word);
        let kind = TokenKind::String(symbol);
        let token = Token::new(kind, span);
        Ok(token)
    }

    // Equals can be either the '=' or '=>' operators.
    fn equals(&mut self) -> Result<Token> {
        let (start, _) = self.chars.next().unwrap();
        let (span, kind) = match self.chars.peek() {
            Some((_, '>')) => {
                let (end, _) = self.chars.next().unwrap();
                let span = Span::new(start as u32, end as u32);
                (span, TokenKind::Arrow)
            }
            // Support == as well
            Some((_, '=')) => {
                let (end, _) = self.chars.next().unwrap();
                let span = Span::new(start as u32, end as u32);
                (span, TokenKind::DoubleEquals)
            }
            _ => {
                let end = start;
                (Span::new(start as u32, end as u32), TokenKind::Equals)
            }
        };
        let token = Token::new(kind, span);
        Ok(token)
    }

    fn dot(&mut self) -> Result<Token> {
        let (start, _) = self.chars.next().unwrap();
        let (span, kind) = match self.chars.peek() {
            // Range
            Some((_, '.')) => {
                let (end, _) = self.chars.next().unwrap();
                (Span::new(start as u32, end as u32), TokenKind::Range)
            }
            // Decimal
            _ => {
                let end = start;
                (Span::new(start as u32, end as u32), TokenKind::Dot)
            }
        };
        let token = Token::new(kind, span);
        Ok(token)
    }

    fn and(&mut self) -> Result<Token> {
        let (start, _) = self.chars.next().unwrap();
        let (span, kind) = match self.chars.peek() {
            Some((_, '&')) => {
                let (end, _) = self.chars.next().unwrap();
                (Span::new(start as u32, end as u32), TokenKind::BinAnd)
            }
            _ => {
                let end = start;
                (Span::new(start as u32, end as u32), TokenKind::And)
            }
        };
        let token = Token::new(kind, span);
        Ok(token)
    }

    fn greater_than(&mut self) -> Result<Token> {
        let (start, _) = self.chars.next().unwrap();
        let (span, kind) = match self.chars.peek() {
            Some((_, '=')) => {
                let (end, _) = self.chars.next().unwrap();
                (
                    Span::new(start as u32, end as u32),
                    TokenKind::GreaterThanEquals,
                )
            }
            _ => {
                let end = start;
                (Span::new(start as u32, end as u32), TokenKind::GreaterThan)
            }
        };
        let token = Token::new(kind, span);
        Ok(token)
    }

    fn less_than(&mut self) -> Result<Token> {
        let (start, _) = self.chars.next().unwrap();
        let (span, kind) = match self.chars.peek() {
            Some((_, '=')) => {
                let (end, _) = self.chars.next().unwrap();
                (
                    Span::new(start as u32, end as u32),
                    TokenKind::LessThanEquals,
                )
            }
            _ => {
                let end = start;
                (Span::new(start as u32, end as u32), TokenKind::LessThan)
            }
        };
        let token = Token::new(kind, span);
        Ok(token)
    }

    fn number(&mut self) -> Result<Token> {
        let (start, _) = self.chars.next().unwrap();
        let mut end = start;
        let mut is_float = false;
        loop {
            match self.chars.peek() {
                Some((i, ch)) => {
                    if ch.is_digit(10) || ch == &'_' {
                        end = *i;
                    } else if ch == &'.' {
                        if is_float {
                            // Check if the next char is a
                            return multiple_decimal_in_number(Span::new(start as u32, end as u32));
                        }
                        is_float = true;
                        end = *i;
                    } else {
                        break;
                    }
                }
                _ => break,
            }
            self.chars.next();
        }
        let span = Span::new(start as u32, end as u32);
        let word = &self.source[start..end + 1];
        let symbol = Symbol::intern(word);
        let kind = TokenKind::Number(symbol);
        let token = Token::new(kind, span);
        Ok(token)
    }

    fn identifier(&mut self) -> Result<Token> {
        let (start, _) = self.chars.next().unwrap();
        let mut end = start;
        while let Some((i, ch)) = self.chars.peek() {
            if ch.is_xid_continue() {
                end = *i;
                self.chars.next();
                continue;
            } else {
                break;
            }
        }
        let span = Span::new(start as u32, end as u32);
        let word = &self.source[start..end + 1];
        let kind = {
            use TokenKind::*;
            match word {
                "import" => Import,
                "if" => If,
                "else" => Else,
                "fn" => Fn,
                "in" => In,
                "while" => While,
                "for" => For,
                "await" => Await,
                "async" => Async,
                "true" => True,
                "false" => False,
                "let" => Let,
                "state" => State,
                "component" => Component,
                "enum" => Enum,
                "struct" => Struct,
                "const" => Const,
                "pub" => Pub,
                "return" => Return,
                "type" => Type,
                "and" => And,
                "or" => Or,
                "match" => Match,
                "effect" => Effect,
                "number" => NumberType,
                "string" => StringType,
                "bool" => Boolean,
                _ => {
                    let symbol = Symbol::intern(word);
                    Identifier(symbol)
                }
            }
        };
        Ok(Token::new(kind, span))
    }

    fn punc(&mut self, kind: TokenKind) -> Result<Token> {
        let (index, _) = self.chars.next().unwrap();
        let span = Span::new(index as u32, index as u32);
        Ok(Token { kind, span })
    }
}
