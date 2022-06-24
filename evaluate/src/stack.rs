use crate::value::Value;

pub struct StackFrame {
    pub locals: Vec<Value>,
}

pub struct Stack {
    pub frames: Vec<StackFrame>,
}
