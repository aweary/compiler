#[derive(Debug, PartialOrd, Ord, PartialEq, Eq, Clone, Copy)]
pub enum Precedence {
    None = 0,
    Assignment = 1,
    Conditional = 2,
    Sum = 3,
    Product = 4,
    Compare = 5,
    Prefix = 6,
}
