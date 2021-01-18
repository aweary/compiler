use std::ops::{Deref, DerefMut, Range};
use std::fmt::Debug;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    start: u32,
    end: u32
}

impl From<usize> for Span {
    fn from(n: usize) -> Self {
        Span::new(n as u32, n as u32)
    }
}

impl Debug for Span {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}..{}", self.start, self.end)
    }

}



impl Into<Range<usize>> for Span {
    fn into(self) -> Range<usize> {
        self.start as usize..self.end as usize
    }
}

impl Span {

    pub fn new(start: u32, end: u32) -> Span {
        Span { start, end }
    }

    pub fn merge(self, other: Span) -> Span {
        use std::cmp::{min, max};
        let start = min(self.start, other.start);
        let end = max(self.end, other.end);
        Span::new(start, end)
    }
}

pub struct Spanned<T> {
    value: T,
    span: Span,
}

impl<T> Spanned<T> {
    #[inline]
    pub fn span(&self) -> Span {
        self.span
    }
}


impl<T> Deref for Spanned<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

impl<T> DerefMut for Spanned<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
       &mut self.value 
    }
}