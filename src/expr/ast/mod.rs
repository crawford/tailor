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

pub mod types;

use errors::*;
use nom::{self, IResult};
use std::str::FromStr;
pub use self::types::*;

#[derive(Debug, PartialEq)]
enum PartialOperation {
    Equal(Expr),
    LessThan(Expr),
    GreaterThan(Expr),

    And(Expr),
    Or(Expr),
    Xor(Expr),
    Not,

    All(Expr),
    Any(Expr),
    Filter(Expr),
    Map(Expr),
    Length,

    Test(Expr),
}

#[derive(PartialEq)]
enum InfixOperator {
    Equal,
    LessThan,
    GreaterThan,

    And,
    Or,
    Xor,

    All,
    Any,
    Filter,
    Map,

    Test,
}

fn is_context(c: char) -> bool {
    c.is_alphabetic() || c == '.'
}

named!(value <&str, Expr>, ws!(
    alt!(
        tag!("true")  => { |_| Expr::Value(Value::Boolean(true))  } |
        tag!("false") => { |_| Expr::Value(Value::Boolean(false)) } |
        map!(
            flat_map!(call!(nom::digit), parse_to!(usize)),
            |n| { Expr::Value(Value::Numeral(n)) }
        ) |
        map!(
            delimited!(
                char!('['),
                many0!(value),
                char!(']')
            ),
            |l| { Expr::Value(Value::List(l)) }
        ) |
        map!(
            delimited!(
                char!('"'),
                flat_map!(call!(nom::alpha), parse_to!(String)),
                char!('"')
            ),
            |p| { Expr::Value(Value::String(p)) }
        ) |
        map!(
            map_res!(
                preceded!(
                    char!('.'),
                    take_while!(is_context)
                ),
                FromStr::from_str
            ),
            |s: String| { Expr::Operation(Operation::Context(s)) }
        ) |
        delimited!(
            char!('('),
            expr,
            char!(')')
        )
    )
));

named!(operation0 <&str, PartialOperation>, ws!(
    alt!(
        tag!("not")    => { |_| PartialOperation::Not    } |
        tag!("length") => { |_| PartialOperation::Length }
    )
));

named!(operation1 <&str, PartialOperation>, ws!(
    do_parse!(
        op: alt!(
            char!('=') => { |_| InfixOperator::Equal       } |
            char!('<') => { |_| InfixOperator::LessThan    } |
            char!('>') => { |_| InfixOperator::GreaterThan } |

            tag!("and") => { |_| InfixOperator::And } |
            tag!("or")  => { |_| InfixOperator::Or  } |
            tag!("xor") => { |_| InfixOperator::Xor } |

            tag!("all")    => { |_| InfixOperator::All    } |
            tag!("any")    => { |_| InfixOperator::Any    } |
            tag!("filter") => { |_| InfixOperator::Filter } |
            tag!("map")    => { |_| InfixOperator::Map    } |

            tag!("test")    => { |_| InfixOperator::Test }
        ) >>
        arg: value >>
        (match op {
            InfixOperator::Equal       => PartialOperation::Equal(arg),
            InfixOperator::LessThan    => PartialOperation::LessThan(arg),
            InfixOperator::GreaterThan => PartialOperation::GreaterThan(arg),

            InfixOperator::And => PartialOperation::And(arg),
            InfixOperator::Or  => PartialOperation::Or(arg),
            InfixOperator::Xor => PartialOperation::Xor(arg),

            InfixOperator::All    => PartialOperation::All(arg),
            InfixOperator::Any    => PartialOperation::Any(arg),
            InfixOperator::Filter => PartialOperation::Filter(arg),
            InfixOperator::Map    => PartialOperation::Map(arg),

            InfixOperator::Test => PartialOperation::Test(arg),
        })
    )
));

named!(expr <&str, Expr>, ws!(
    do_parse!(
        init: value >>
        exp: fold_many0!(
            alt!(operation0 | operation1),
            init,
            |ast, part| {
                match part {
                    PartialOperation::Equal(arg)       => Expr::Operation(Operation::Equal(Box::new(ast), Box::new(arg))),
                    PartialOperation::LessThan(arg)    => Expr::Operation(Operation::LessThan(Box::new(ast), Box::new(arg))),
                    PartialOperation::GreaterThan(arg) => Expr::Operation(Operation::GreaterThan(Box::new(ast), Box::new(arg))),

                    PartialOperation::And(arg) => Expr::Operation(Operation::And(Box::new(ast), Box::new(arg))),
                    PartialOperation::Or(arg)  => Expr::Operation(Operation::Or(Box::new(ast), Box::new(arg))),
                    PartialOperation::Xor(arg) => Expr::Operation(Operation::Xor(Box::new(ast), Box::new(arg))),
                    PartialOperation::Not      => Expr::Operation(Operation::Not(Box::new(ast))),

                    PartialOperation::All(arg)    => Expr::Operation(Operation::All(Box::new(ast), Box::new(arg))),
                    PartialOperation::Any(arg)    => Expr::Operation(Operation::Any(Box::new(ast), Box::new(arg))),
                    PartialOperation::Filter(arg) => Expr::Operation(Operation::Filter(Box::new(ast), Box::new(arg))),
                    PartialOperation::Map(arg)    => Expr::Operation(Operation::Map(Box::new(ast), Box::new(arg))),
                    PartialOperation::Length      => Expr::Operation(Operation::Length(Box::new(ast))),

                    PartialOperation::Test(arg) => Expr::Operation(Operation::Test(Box::new(ast), Box::new(arg))),
                }
            }
        ) >>
        (exp)
    )
));

pub fn parse(expression: &str) -> Result<Expr> {
    debug!("Parsing expression: {}", expression);
    match expr(expression) {
        IResult::Done("", expr) => {
            trace!("Expression parsed as {:?}", expr);
            Ok(expr)
        }
        IResult::Done(r, _) => {
            warn!("Parsing finished with remaining characters: {}", r);
            Err(format!("input remaining: {}", r).into())
        }
        IResult::Error(err) => {
            warn!("Parsing error occured: {}", err);
            Err(format!("error occurred: {}", err).into())
        }
        IResult::Incomplete(n) => {
            warn!(
                "Parsing finished prematurely. {:?} more characters expected.",
                n
            );
            Err(format!("needed more: {:?}", n).into())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_value() {
        assert_eq!(
            value("true"),
            IResult::Done("", Expr::Value(Value::Boolean(true)))
        );
        assert_eq!(
            value("false"),
            IResult::Done("", Expr::Value(Value::Boolean(false)))
        );
        assert_eq!(
            value(" true  "),
            IResult::Done("", Expr::Value(Value::Boolean(true)))
        );
        assert_eq!(
            value("12"),
            IResult::Done("", Expr::Value(Value::Numeral(12)))
        );
        assert_eq!(
            value("  52 "),
            IResult::Done("", Expr::Value(Value::Numeral(52)))
        );
        assert_eq!(
            value("[]"),
            IResult::Done("", Expr::Value(Value::List(vec![])))
        );
        assert_eq!(
            value("[1 true]"),
            IResult::Done(
                "",
                Expr::Value(Value::List(vec![
                    Expr::Value(Value::Numeral(1)),
                    Expr::Value(Value::Boolean(true)),
                ])),
            )
        );
    }

    #[test]
    fn test_parse() {
        assert_eq!(
            parse("1 < 7").unwrap(),
            Expr::Operation(Operation::LessThan(
                Box::new(Expr::Value(Value::Numeral(1))),
                Box::new(Expr::Value(Value::Numeral(7))),
            ))
        );
        assert_eq!(
            parse("false and true not and true").unwrap(),
            Expr::Operation(Operation::And(
                Box::new(Expr::Operation(
                    Operation::Not(Box::new(Expr::Operation(Operation::And(
                        Box::new(Expr::Value(Value::Boolean(false))),
                        Box::new(Expr::Value(Value::Boolean(true))),
                    )))),
                )),
                Box::new(Expr::Value(Value::Boolean(true))),
            ))
        );
        assert_eq!(
            parse("((1 < 7) or (2 > 9)) and true").unwrap(),
            Expr::Operation(Operation::And(
                Box::new(Expr::Operation(Operation::Or(
                    Box::new(Expr::Operation(Operation::LessThan(
                        Box::new(Expr::Value(Value::Numeral(1))),
                        Box::new(Expr::Value(Value::Numeral(7))),
                    ))),
                    Box::new(Expr::Operation(Operation::GreaterThan(
                        Box::new(Expr::Value(Value::Numeral(2))),
                        Box::new(Expr::Value(Value::Numeral(9))),
                    ))),
                ))),
                Box::new(Expr::Value(Value::Boolean(true))),
            ))
        );
        assert_eq!(
            parse("(.attr) length").unwrap(),
            Expr::Operation(Operation::Length(Box::new(
                Expr::Operation(Operation::Context(String::from("attr"))),
            )))
        );
        assert_eq!(
            parse(".attr length").unwrap(),
            Expr::Operation(Operation::Length(Box::new(
                Expr::Operation(Operation::Context(String::from("attr"))),
            )))
        );
        assert_eq!(
            parse(".attr.sub length").unwrap(),
            Expr::Operation(Operation::Length(Box::new(Expr::Operation(
                Operation::Context(String::from("attr.sub")),
            ))))
        );
    }
}
