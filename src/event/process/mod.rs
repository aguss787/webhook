use std::collections::HashMap;
use serde::Deserialize;
use thiserror::Error;

pub mod operation;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {}

pub type State = HashMap<String, Item>;

#[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(untagged)]
pub enum Item {
    Value(Value),
    Vec(Vec<Value>),
    Map(HashMap<String, Item>),
}

#[derive(Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(untagged)]
pub enum Value {
    None,
    IntValue(i64),
    StringValue(String),
}
