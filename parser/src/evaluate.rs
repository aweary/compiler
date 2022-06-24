use std::collections::HashMap;

use diagnostics::result::Result;
use syntax::{ast::BinOp, ast_::*, visit_::Visitor};

use crate::control_flow::constrct_cfg_from_block;

use evaluate::Value;

pub struct ExpressionEvaluator<'a> {
    arena: &'a mut AstArena,
}

impl<'a> ExpressionEvaluator<'a> {
    pub fn new(arena: &'a mut AstArena) -> Self {
        Self { arena }
    }
}

#[derive(Debug, Clone)]
pub struct CallContext {
    pub arguments: HashMap<ParameterId, ExpressionId>,
}

pub fn evaluate_expression(
    arena: &AstArena,
    expression: &Expression,
    call_context: Option<&CallContext>,
) -> Option<Value> {
    match expression {
        Expression::Call { callee, arguments } => {
            let callee_expr = arena.expressions.get(*callee).expect("callee not found");
            if let Expression::Reference(Binding::Function(function_id)) = *callee_expr.borrow() {
                let function_ref = arena
                    .functions
                    .get(function_id)
                    .expect("function not found");
                let function = function_ref.borrow();
                let body = arena
                    .blocks
                    .get(function.body.unwrap())
                    .expect("function body not found");

                let call_context = if let Some(parameters) = &function.parameters {
                    let params_and_arguments = parameters.iter().zip(arguments.iter());
                    let mut arguments = HashMap::new();
                    for (parameter, argument) in params_and_arguments {
                        arguments.insert(*parameter, argument.value);
                    }
                    Some(CallContext { arguments })
                } else {
                    None
                };
                let cfg = constrct_cfg_from_block(body, arena, call_context.as_ref());
                println!(
                    "Call to '{}' expression evaluated to: {:?}",
                    function.name.symbol, cfg.value
                );
                cfg.value
            } else {
                None
            }
        }
        Expression::Binary { left, right, op } => {
            let left_expr = {
                let left_expr_cell = arena.expressions.get(*left).unwrap();
                left_expr_cell.borrow()
            };
            let right_expr = {
                let right_expr_cell = arena.expressions.get(*right).unwrap();
                right_expr_cell.borrow()
            };

            let left_value = evaluate_expression(arena, &left_expr, call_context);
            let right_value = evaluate_expression(arena, &right_expr, call_context);

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
                        evaluate_expression(arena, &expression, call_context)
                    }
                    _ => None,
                }
            }
            Binding::Const(const_id) => {
                let const_ = arena.consts.get(*const_id).unwrap();
                let expression = arena.expressions.get(const_.value).unwrap().borrow();
                evaluate_expression(arena, &expression, call_context)
            }
            Binding::Parameter(parameter_id) => {
                if let Some(call_context) = call_context {
                    if let Some(value) = call_context.arguments.get(parameter_id) {
                        let value_expression = arena.expressions.get(*value).unwrap();
                        let value_expression = value_expression.borrow();
                        evaluate_expression(arena, &value_expression, Some(call_context))
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
            _ => None, // Binding::Function(function_id) => todo!(),
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
        let call_context = if let Expression::Call { callee, arguments } = expression {
            let callee_expr = self
                .arena
                .expressions
                .get(*callee)
                .expect("callee not found");
            let callee_expr = callee_expr.borrow();

            if let Expression::Reference(binding) = *callee_expr {
                if let Binding::Function(function_id) = binding {
                    let function = self.arena.functions.get(function_id).unwrap();
                    let function = function.borrow();
                    match &function.parameters {
                        Some(parameters) => {
                            let params_and_arguments = parameters.iter().zip(arguments.iter());
                            let mut arguments = HashMap::new();
                            for (parameter, argument) in params_and_arguments {
                                arguments.insert(*parameter, argument.value);
                            }
                            let call_context = CallContext { arguments };
                            Some(call_context)
                        }
                        None => Some(CallContext {
                            arguments: HashMap::new(),
                        }),
                    }
                } else {
                    None
                }
            } else {
                None
            }
        } else {
            None
        };

        if let Some(value) = evaluate_expression(self.arena, expression, call_context.as_ref()) {
            *expression = value_to_expression(value);
        } else {
            // ...
        }
        Ok(())
    }
}

pub fn value_to_expression(value: Value) -> Expression {
    match value {
        Value::Boolean(value) => Expression::Boolean(value),
        Value::Number(value) => Expression::Number(value),
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
