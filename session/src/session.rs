use typed_arena::{Arena};
use syntax::token::Token;

#[derive(Default)]
pub struct Session {
    /// Arena for allocating tokens when lexing
    tokens: Arena<Token>,
}

impl Session {

    pub fn new() -> Self {
        Session::default()
    }

    /// Allocate a new token for the lexer.
    pub fn alloc_token(&self, token: Token) -> &Token {
        self.tokens.alloc(token)
    }
}