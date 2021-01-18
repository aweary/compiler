use syntax::token::Token;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenStream {
    tokens: Vec<Token>,
}

impl TokenStream {
    /// Allocate a TokenStream with an estimated capacity
    /// for the source string.
    pub fn for_source(string: &str) -> Self {
        let len = string.len();
        let capacity = (len / 3).next_power_of_two();
        let tokens = Vec::with_capacity(capacity);
        TokenStream { tokens }
    }

    pub fn push(&mut self, token: Token) {
        self.tokens.push(token)
    }
}

impl IntoIterator for TokenStream {
    type Item = Token;
    type IntoIter = std::vec::IntoIter<Self::Item>;
    fn into_iter(self) -> Self::IntoIter {
        self.tokens.into_iter()
    }
}