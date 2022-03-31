use diagnostics::result::Result;
use syntax::{ast::BinOp, ast_::*, visit_::Visitor};

pub struct ExpressionEvaluator<'a> {
    arena: &'a mut AstArena,
}

impl<'a> ExpressionEvaluator<'a> {
    pub fn new(arena: &'a mut AstArena) -> Self {
        Self { arena }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Value {
    Boolean(bool),
    Number(f64),
}

fn evaluate_expression(arena: &AstArena, expression: &Expression) -> Option<Value> {
    match expression {
        Expression::Binary { left, right, op } => {
            let left_expr = {
                let left_expr_cell = arena.expressions.get(*left).unwrap();
                left_expr_cell.borrow()
            };
            let right_expr = {
                let right_expr_cell = arena.expressions.get(*right).unwrap();
                right_expr_cell.borrow()
            };

            let left_value = evaluate_expression(arena, &left_expr);
            let right_value = evaluate_expression(arena, &right_expr);

            match (left_value, right_value) {
                (Some(left_value), Some(right_value)) => match (left_value, right_value) {
                    // Two numeric values!
                    (Value::Number(left_value), Value::Number(right_value)) => match op {
                        BinOp::DoubleEquals => Some(Value::Boolean(left_value == right_value)),
                        BinOp::Add => Some(Value::Number(left_value + right_value)),
                        BinOp::Sub => Some(Value::Number(left_value - right_value)),
                        BinOp::Sum => Some(Value::Number(left_value + right_value)),
                        BinOp::Mul => Some(Value::Number(left_value * right_value)),
                        BinOp::Div => Some(Value::Number(left_value / right_value)),
                        BinOp::Mod => Some(Value::Number(left_value % right_value)),
                        BinOp::GreaterThan => Some(Value::Boolean(left_value > right_value)),
                        BinOp::LessThan => Some(Value::Boolean(left_value < right_value)),
                        _ => None,
                    },
                    // Two boolean values
                    (Value::Boolean(left_value), Value::Boolean(right_value)) => match op {
                        BinOp::And => Some(Value::Boolean(left_value && right_value)),
                        BinOp::Or => Some(Value::Boolean(left_value || right_value)),
                        BinOp::DoubleEquals => Some(Value::Boolean(left_value == right_value)),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            }
        }
        Expression::Number(value) => Some(Value::Number(*value)),
        Expression::Boolean(value) => Some(Value::Boolean(*value)),
        Expression::Reference(binding) => match binding {
            Binding::Let(statement_id) => {
                let statement = arena.statements.get(*statement_id).unwrap();
                match statement {
                    Statement::Let { value, .. } => {
                        let expression = arena.expressions.get(*value).unwrap().borrow();
                        evaluate_expression(arena, &expression)
                    }
                    _ => None,
                }
            }
            Binding::Const(const_id) => {
                let const_ = arena.consts.get(*const_id).unwrap();
                let expression = arena.expressions.get(const_.value).unwrap().borrow();
                evaluate_expression(arena, &expression)
            }
            _ => None
            // Binding::Function(function_id) => todo!(),
        },
        _ => None,
    }
}

impl<'a> Visitor for ExpressionEvaluator<'a> {
    fn context_mut(&mut self) -> &mut AstArena {
        self.arena
    }

    fn context(&self) -> &AstArena {
        self.arena
    }

    fn visit_expression(&self, expression: &mut Expression) -> Result<()> {
        println!("evaluate {:?}", expression);
        if let Some(value) = evaluate_expression(self.arena, expression) {
            println!("value {:?}\n", value);
            match value {
                Value::Boolean(value) => {
                    *expression = Expression::Boolean(value);
                }
                Value::Number(value) => {
                    *expression = Expression::Number(value);
                }
            }
        } else {
            println!("No value\n")
        }
        Ok(())
    }
}

#[test]
fn evaluate_simple_expr_test() {
    let mut arena = AstArena::default();

    let mut expression = {
        let left = arena.alloc_expression(Expression::Number(5.0));
        let right = arena.alloc_expression(Expression::Number(10.0));
        let op = BinOp::Add;
        Expression::Binary { left, right, op }
    };

    let evaluate = ExpressionEvaluator::new(&mut arena);

    evaluate.visit_expression(&mut expression).unwrap();

    assert!(expression == Expression::Number(15.0));
}
