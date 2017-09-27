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

use chrono::{DateTime, Utc};
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq)]
pub enum Operation {
    Equal(Box<Expr>, Box<Expr>),
    LessThan(Box<Expr>, Box<Expr>),
    GreaterThan(Box<Expr>, Box<Expr>),

    And(Box<Expr>, Box<Expr>),
    Or(Box<Expr>, Box<Expr>),
    Xor(Box<Expr>, Box<Expr>),
    Not(Box<Expr>),

    All(Box<Expr>, Box<Expr>),
    Any(Box<Expr>, Box<Expr>),
    Filter(Box<Expr>, Box<Expr>),
    Map(Box<Expr>, Box<Expr>),
    Length(Box<Expr>),

    Test(Box<Expr>, Box<Expr>),
    Lines(Box<Expr>),

    Context(String),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Numeral(usize),
    Boolean(bool),
    String(String),
    List(Vec<Expr>),
    Dictionary(HashMap<String, Value>),
}

#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Value(Value),
    Operation(Operation),
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<Option<String>> for Value {
    fn from(s: Option<String>) -> Self {
        match s {
            Some(s) => s.into(),
            None => String::new().into(),
        }
    }
}

impl From<DateTime<Utc>> for Value {
    fn from(t: DateTime<Utc>) -> Self {
        Value::String(t.to_rfc3339())
    }
}

impl<T: Into<Value>> From<Vec<T>> for Value {
    fn from(v: Vec<T>) -> Self {
        Value::List(v.into_iter().map(|e| Expr::Value(e.into())).collect())
    }
}
