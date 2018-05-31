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

pub use self::types::*;
use errors::*;
use nom::{self, types::CompleteStr, Err};
use std::str::FromStr;

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
    Lines,
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

named!(boolean <CompleteStr, Expr>,
    alt!(
        tag!("true")  => { |_| Expr::Value(Value::Boolean(true))  } |
        tag!("false") => { |_| Expr::Value(Value::Boolean(false)) }
    )
);

named!(numeral <CompleteStr, Expr>,
    map!(
        flat_map!(call!(nom::digit), parse_to!(usize)),
        |n| { Expr::Value(Value::Numeral(n)) }
    )
);

named!(list <CompleteStr, Expr>,
    map!(
        delimited!(
            char!('['),
            many0!(value),
            char!(']')
        ),
        |l| { Expr::Value(Value::List(l)) }
    )
);

named!(string <CompleteStr, Expr>,
    map!(
        delimited!(
            char!('"'),
            fold_many0!(
                alt!(
                    map!(tag!(r#"\""#), |_| CompleteStr(r#"""#)) |
                    map!(tag!(r#"\\"#), |_| CompleteStr(r#"\"#)) |
                    is_not!(r#""\"#)
                ),
                String::new(),
                |acc: String, s: CompleteStr| {
                    acc + &s
                }
            ),
            char!('"')
        ),
        |s| { Expr::Value(Value::String(s.to_string())) }
    )
);

named!(context <CompleteStr, Expr>,
    map!(
        map_res!(
            preceded!(
                char!('.'),
                take_while!(|c: char| { c.is_alphabetic() || c == '.' })
            ),
            |c: CompleteStr| { FromStr::from_str(&c) }
        ),
        |s: String| { Expr::Operation(Operation::Context(s)) }
    )
);

named!(nested <CompleteStr, Expr>,
    delimited!(
        char!('('),
        expr,
        char!(')')
    )
);

named!(value <CompleteStr, Expr>, ws!(
    alt!(boolean | numeral | list | string | context | nested)
));

named!(operation0 <CompleteStr, PartialOperation>, ws!(
    alt!(
        tag!("not")    => { |_| PartialOperation::Not    } |
        tag!("length") => { |_| PartialOperation::Length } |
        tag!("lines")  => { |_| PartialOperation::Lines  }
    )
));

named!(operation1 <CompleteStr, PartialOperation>, ws!(
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

named!(expr <CompleteStr, Expr>, ws!(
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
                    PartialOperation::Lines     => Expr::Operation(Operation::Lines(Box::new(ast))),
                }
            }
        ) >>
        (exp)
    )
));

pub fn parse(expression: &str) -> Result<Expr> {
    debug!("Parsing expression: {}", expression);
    match expr(CompleteStr(expression)) {
        Ok((CompleteStr(""), expr)) => {
            trace!("Expression parsed as {:?}", expr);
            Ok(expr)
        }
        Ok((r, _)) => {
            warn!("Parsing finished with remaining characters: {}", r);
            Err(format!("input remaining: {}", r).into())
        }
        Err(Err::Incomplete(n)) => {
            warn!(
                "Parsing finished prematurely. {:?} more characters expected.",
                n
            );
            Err(format!("needed more: {:?}", n).into())
        }
        Err(err) => {
            warn!("Parsing error occured: {}", err);
            Err(format!("error occurred: {}", err).into())
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_value() {
        assert_eq!(
            value(CompleteStr("true")),
            Ok((CompleteStr(""), Expr::Value(Value::Boolean(true))))
        );
        assert_eq!(
            value(CompleteStr("false")),
            Ok((CompleteStr(""), Expr::Value(Value::Boolean(false))))
        );
        assert_eq!(
            value(CompleteStr(" true  ")),
            Ok((CompleteStr(""), Expr::Value(Value::Boolean(true))))
        );
        assert_eq!(
            value(CompleteStr("12")),
            Ok((CompleteStr(""), Expr::Value(Value::Numeral(12))))
        );
        assert_eq!(
            value(CompleteStr("  52 ")),
            Ok((CompleteStr(""), Expr::Value(Value::Numeral(52))))
        );
        assert_eq!(
            value(CompleteStr("[]")),
            Ok((CompleteStr(""), Expr::Value(Value::List(vec![]))))
        );
        assert_eq!(
            value(CompleteStr("[1 true]")),
            Ok((
                CompleteStr(""),
                Expr::Value(Value::List(vec![
                    Expr::Value(Value::Numeral(1)),
                    Expr::Value(Value::Boolean(true)),
                ])),
            ))
        );
        assert_eq!(
            value(CompleteStr(r#""""#)),
            Ok((CompleteStr(""), Expr::Value(Value::String(String::new()))))
        );
        assert_eq!(
            value(CompleteStr(r#""simple string""#)),
            Ok((
                CompleteStr(""),
                Expr::Value(Value::String("simple string".to_string()))
            ))
        );
        assert_eq!(
            value(CompleteStr(r#""^[A-Za-z\":\\]{,100}$""#)),
            Ok((
                CompleteStr(""),
                Expr::Value(Value::String(r#"^[A-Za-z":\]{,100}$"#.to_string())),
            ))
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
                Box::new(Expr::Operation(Operation::Not(Box::new(Expr::Operation(
                    Operation::And(
                        Box::new(Expr::Value(Value::Boolean(false))),
                        Box::new(Expr::Value(Value::Boolean(true))),
                    )
                ))),)),
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
            Expr::Operation(Operation::Length(Box::new(Expr::Operation(
                Operation::Context(String::from("attr"))
            ),)))
        );
        assert_eq!(
            parse(".attr length").unwrap(),
            Expr::Operation(Operation::Length(Box::new(Expr::Operation(
                Operation::Context(String::from("attr"))
            ),)))
        );
        assert_eq!(
            parse(".attr.sub length").unwrap(),
            Expr::Operation(Operation::Length(Box::new(Expr::Operation(
                Operation::Context(String::from("attr.sub")),
            ))))
        );
    }
}
