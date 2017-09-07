// Copyright 2017 CoreOS, Inc.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

pub mod ast;

use errors::*;
use regex::Regex;
use self::ast::{Expr, Operation, Value};

macro_rules! expr {
    ( $expr:expr, $context:expr, $type:path  ) => {
        match match $expr {
            Expr::Value(v) => v,
            Expr::Operation(o) => eval_expr(Expr::Operation(o), $context)?,
        } {
            $type(v) => v,
            _ => Err("Invalid type")?,
        }
    };
}

pub fn eval(expression: &str, input: &Value) -> Result<bool> {
    match eval_expr(
        ast::parse(expression).chain_err(
            || "Failed to parse expression",
        )?,
        input,
    ).chain_err(|| "Failed to evaluate expression")? {
        Value::Boolean(b) => Ok(b),
        _ => Err("Invalid result".into()),
    }
}

fn eval_expr(expr: Expr, context: &Value) -> Result<Value> {
    match expr {
        Expr::Value(val) => Ok(val),
        Expr::Operation(Operation::Equal(a, b)) => Ok(Value::Boolean(
            eval_expr(*a, context)? ==
                eval_expr(*b, context)?,
        )),
        Expr::Operation(Operation::LessThan(a, b)) => Ok(Value::Boolean(
            expr!(*a, context, Value::Numeral) <
                expr!(*b, context, Value::Numeral),
        )),
        Expr::Operation(Operation::GreaterThan(a, b)) => Ok(Value::Boolean(
            expr!(*a, context, Value::Numeral) >
                expr!(*b, context, Value::Numeral),
        )),
        Expr::Operation(Operation::And(a, b)) => Ok(Value::Boolean(
            expr!(*a, context, Value::Boolean) &&
                expr!(*b, context, Value::Boolean),
        )),
        Expr::Operation(Operation::Or(a, b)) => Ok(Value::Boolean(
            expr!(*a, context, Value::Boolean) ||
                expr!(*b, context, Value::Boolean),
        )),
        Expr::Operation(Operation::Xor(a, b)) => Ok(Value::Boolean(
            expr!(*a, context, Value::Boolean) ^
                expr!(*b, context, Value::Boolean),
        )),
        Expr::Operation(Operation::Not(a)) => Ok(
            Value::Boolean(!expr!(*a, context, Value::Boolean)),
        ),
        Expr::Operation(Operation::All(list, condition)) => {
            for elem in expr!(*list, context, Value::List) {
                if !expr!(
                    *condition.clone(),
                    &eval_expr(elem, context)?,
                    Value::Boolean
                )
                {
                    return Ok(Value::Boolean(false));
                }
            }
            Ok(Value::Boolean(true))
        }
        Expr::Operation(Operation::Any(list, condition)) => {
            for elem in expr!(*list, context, Value::List) {
                if expr!(
                    *condition.clone(),
                    &eval_expr(elem, context)?,
                    Value::Boolean
                )
                {
                    return Ok(Value::Boolean(true));
                }
            }
            Ok(Value::Boolean(false))
        }
        Expr::Operation(Operation::Filter(list, condition)) => {
            let mut result = Vec::new();
            for elem in expr!(*list, context, Value::List) {
                let res = eval_expr(elem, context)?;
                if expr!(*condition.clone(), &res, Value::Boolean) {
                    result.push(Expr::Value(res))
                }
            }
            Ok(Value::List(result))
        }
        Expr::Operation(Operation::Map(list, transform)) => {
            let mut result = Vec::new();
            for elem in expr!(*list, context, Value::List) {
                result.push(Expr::Value(
                    eval_expr(*transform.clone(), &eval_expr(elem, context)?)?,
                ));
            }
            Ok(Value::List(result))
        }
        Expr::Operation(Operation::Length(a)) => Ok(Value::Numeral(
            expr!(*a, context, Value::List).len(),
        )),
        Expr::Operation(Operation::Test(term, pattern)) => {
            Ok(Value::Boolean(
                Regex::new(&expr!(*pattern, context, Value::String))?
                    .is_match(&expr!(*term, context, Value::String)),
            ))
        }
        Expr::Operation(Operation::Context(path)) => {
            let mut context = context;
            for elem in path.split('.') {
                match (elem, context) {
                    (path, &Value::Dictionary(ref map)) => {
                        match map.get(path) {
                            Some(val) => context = val,
                            None => Err("No such key")?,
                        }
                    }
                    ("", _) => break,
                    _ => Err("Invalid type")?,
                }
            }
            Ok(context.clone())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_eval() {
        assert_eq!(eval_pr("true").unwrap(), true);
        assert_eq!(eval_pr("true and false").unwrap(), false);
        assert_eq!(eval_pr("(true and false) or true").unwrap(), true);
        assert_eq!(eval_pr("true and (false or true)").unwrap(), true);
        assert_eq!(eval_pr("true not").unwrap(), false);
        assert_eq!(eval_pr("false = false").unwrap(), true);
        assert_eq!(eval_pr("7 = 7").unwrap(), true);
        assert_eq!(eval_pr("7 = true").unwrap(), false);
        assert_eq!(eval_pr("[1 2 3] length = 3").unwrap(), true);
        assert_eq!(eval_pr("[true true true] all .").unwrap(), true);
        assert_eq!(eval_pr("[true true false] all .").unwrap(), false);
        assert_eq!(eval_pr("[false true false] any .").unwrap(), true);
        assert_eq!(eval_pr("[false] any .").unwrap(), false);
        assert_eq!(
            eval_pr("[false true false] filter . length = 1").unwrap(),
            true
        );
        assert_eq!(eval_pr("[false false] map(. not) all .").unwrap(), true);
        assert_eq!(eval_pr(".commits length = 2").unwrap(), true);
        assert_eq!(eval_pr(r#""hello" test "h""#).unwrap(), true);
        assert_eq!(eval_pr(r#""hello" test "z""#).unwrap(), false);
        //assert_eq!(eval_pr(".commits all(.title length) < 50", true));

        //assert_eq!(eval_pr("true length").unwrap(), Err(String::from("Invalid type")));
    }

    fn eval_pr(expression: &str) -> Result<bool> {
        let mut map = HashMap::new();
        map.insert(
            String::from("commits"),
            Value::List(vec![
                Expr::Value(Value::Dictionary(HashMap::new())),
                Expr::Value(Value::Dictionary(HashMap::new())),
            ]),
        );
        eval(expression, &Value::Dictionary(map))
    }
}
