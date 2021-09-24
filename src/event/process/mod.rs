use std::collections::HashMap;
use std::fmt::Formatter;
use std::str::FromStr;

use serde::{Serialize, Deserialize};
use thiserror::Error;
use std::num::ParseIntError;

pub mod operation;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Error, Debug)]
pub enum Error {
    #[error("unable to access field {field} from type {t}")]
    NonMapAccess { field: String, t: String },

    #[error("index {index} out of bound in array with length {len}")]
    IndexOutOfBound { index: usize, len: usize },

    #[error("invalid index: {reason}")]
    InvalidIndex { reason: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct State(HashMap<String, Item>);

impl State {
    pub fn new() -> Self {
        State(HashMap::new())
    }

    pub fn get(&self, key: &Identifier) -> Option<&Item> {
        Self::get_from_map(&self.0, key)
    }

    fn get_from_map<'a>(map: &'a HashMap<String, Item>, key: &Identifier) -> Option<&'a Item> {
        let (key, path) = key.split();

        match key {
            None => { None }
            Some(key) => {
                let value = map.get(&key);

                State::get_from_child(path, value)
            }
        }
    }

    fn get_from_vec<'a>(vec: &'a Vec<Item>, key: &Identifier) -> Option<&'a Item> {
        let (key, path) = key.split();

        match key {
            None => { None }
            Some(key) => {
                let value = usize::from_str(key.as_str())
                    .map(|idx| vec.get(idx))
                    .unwrap_or(None);

                State::get_from_child(path, value)
            }
        }
    }

    fn get_from_child(path: Option<Identifier>, value: Option<&Item>) -> Option<&Item> {
        match path {
            None => { value }
            Some(recursive_key) => {
                value.and_then(|v| {
                    match v {
                        Item::Map(v) => { Self::get_from_map(v, &recursive_key) }
                        Item::Vec(v) => { Self::get_from_vec(v, &recursive_key) }
                        _ => None
                    }
                })
            }
        }
    }

    pub fn set(&mut self, key: Identifier, value: Item) -> Result<Option<Item>> {
        Self::set_map(&mut self.0, key, value)
    }

    fn set_map(map: &mut HashMap<String, Item>, key: Identifier, value: Item) -> Result<Option<Item>> {
        let (key, path) = key.split();
        log::trace!("setting internal state with key {:?} . {:?}, with value {:?}", key, path, value);

        match key {
            None => { Ok(None) }
            Some(key) => {
                match path {
                    None => {
                        Ok(map.insert(key, value))
                    }
                    Some(recursive_key) => {
                        let rec = map.get_mut(&key);

                        let rec = if rec.is_none() {
                            drop(rec);
                            map.insert(key.clone(), Item::Map(HashMap::new()));
                            map.get_mut(&key).unwrap()
                        } else {
                            rec.unwrap()
                        };

                        match rec {
                            Item::Map(map) => {
                                Self::set_map(map, recursive_key, value)
                            }
                            Item::Vec(v) => {
                                Self::set_vec(v, recursive_key, value)
                            }
                            i => Err(Error::NonMapAccess { field: key, t: i.type_name().into() })
                        }
                    }
                }
            }
        }
    }

    fn set_vec(vec: &mut Vec<Item>, key: Identifier, value: Item) -> Result<Option<Item>> {
        let (key, path) = key.split();
        log::trace!("setting internal state with key {:?} . {:?}, with value {:?}", key, path, value);

        match key {
            None => { Ok(None) }
            Some(key) => {
                match path {
                    None => {
                        let mut value = value;
                        Ok(usize::from_str(key.as_str())
                            .map(|idx| vec.get_mut(idx))?
                            .map(|val| {
                                std::mem::swap(val, &mut value);
                                value
                            }))
                    }
                    Some(recursive_key) => {
                        let idx = usize::from_str(key.as_str());
                        let rec = idx
                            .clone()
                            .map(|idx| vec.get_mut(idx))?;

                        match rec {
                            None => Err(Error::IndexOutOfBound { index: idx?, len: vec.len() }),
                            Some(Item::Map(map)) => {
                                Self::set_map(map, recursive_key, value)
                            }
                            Some(Item::Vec(v)) => {
                                Self::set_vec(v, recursive_key, value)
                            }
                            Some(i) => Err(Error::NonMapAccess { field: key, t: i.type_name().into() })
                        }
                    }
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[cfg(test)]
mod state_tests {
    use super::*;

    #[test]
    fn set_ok() {
        let mut state = State::new();

        let key: Identifier = "key".into();
        let value = Item::Value(Value::StringValue("123".into()));

        let returned_item = state.set(key.clone(), value.clone());
        assert!(returned_item.is_ok());
        let returned_item = returned_item.unwrap();
        assert!(returned_item.is_none());

        drop(returned_item);

        assert_eq!(state.0.len(), 1);

        let item = state.0.get(&key.to_string());
        assert!(item.is_some());
        assert_eq!(item.unwrap(), &value);
    }

    #[test]
    fn set_replace_ok() {
        let mut state = State::new();

        let key: Identifier = "key".into();
        let value = Item::Value(Value::StringValue("123".into()));
        let other_value = Item::Value(Value::StringValue("321".into()));

        let returned_item = state.set(key.clone(), other_value.clone());
        assert!(returned_item.is_ok());
        let returned_item = returned_item.unwrap();
        assert!(returned_item.is_none());

        let returned_item = state.set(key.clone(), value.clone());
        assert!(returned_item.is_ok());
        let returned_item = returned_item.unwrap();
        assert!(returned_item.is_some());
        assert_eq!(returned_item.unwrap(), other_value);

        assert_eq!(state.0.len(), 1);

        let item = state.0.get(&key.to_string());
        assert!(item.is_some());
        assert_eq!(item.unwrap(), &value);
    }

    #[test]
    fn set_recursive_ok() {
        let mut state = State::new();

        let key: Identifier = "key.other".into();
        let value = Item::Value(Value::StringValue("123".into()));

        let returned_item = state.set(key.clone(), value.clone());
        assert!(returned_item.is_ok());
        let returned_item = returned_item.unwrap();
        assert!(returned_item.is_none());

        drop(returned_item);

        assert_eq!(state.0.len(), 1);

        let item = state.0.get(&String::from("key"));
        assert!(item.is_some());

        let item = item.unwrap();
        assert!(matches!(item, Item::Map(_)));

        let map = match item {
            Item::Map(map) => map,
            _ => unreachable!()
        };
        let item = map.get("other");
        assert!(item.is_some());

        let item = item.unwrap();
        assert_eq!(item, &value);
    }

    #[test]
    fn set_array_ok() {
        let mut state = State::new();

        let key: Identifier = "key".into();
        let old_value = Item::Value(Value::IntValue(123));
        let vec = Item::Vec(vec!(
            old_value.clone(),
        ));
        let value = Item::Value(Value::StringValue("123".into()));

        let returned_value = state.set(key.clone(), vec.clone());
        assert!(returned_value.is_ok());

        let returned_value = state.set("key.0".into(), value.clone());
        assert!(returned_value.is_ok());
        let returned_value = returned_value.unwrap();
        assert!(returned_value.is_some());
        assert_eq!(returned_value.unwrap(), old_value);

        assert_eq!(state.0.len(), 1);

        let item = state.0.get(&String::from("key"));
        assert!(item.is_some());

        let item = item.unwrap();
        assert!(matches!(item, Item::Vec(_)));

        let map = match item {
            Item::Vec(map) => map,
            _ => unreachable!()
        };
        let item = map.get(0);
        assert!(item.is_some());

        let item = item.unwrap();
        assert_eq!(item, &value);
    }


    #[test]
    fn set_recursive_replace_ok() {
        let mut state = State::new();

        let key: Identifier = "key.other".into();
        let value = Item::Value(Value::StringValue("123".into()));
        let other_value = Item::Value(Value::StringValue("321".into()));

        let returned_item = state.set(key.clone(), other_value.clone());
        assert!(returned_item.is_ok());
        let returned_item = returned_item.unwrap();
        assert!(returned_item.is_none());

        let returned_item = state.set(key.clone(), value.clone());
        assert!(returned_item.is_ok());
        let returned_item = returned_item.unwrap();
        assert!(returned_item.is_some());
        assert_eq!(returned_item.unwrap(), other_value);

        assert_eq!(state.0.len(), 1);

        let item = state.0.get(&String::from("key"));
        assert!(item.is_some());

        let item = item.unwrap();
        assert!(matches!(item, Item::Map(_)));

        let map = match item {
            Item::Map(map) => map,
            _ => unreachable!()
        };
        let item = map.get("other");
        assert!(item.is_some());

        let item = item.unwrap();
        assert_eq!(item, &value);
    }

    #[test]
    fn get_some_ok() {
        let mut state = State::new();

        let key: Identifier = "key".into();
        let value = Item::Value(Value::StringValue("123".into()));

        let _ = state.set(key.clone(), value.clone());

        let result = state.get(&key);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &value);
    }

    #[test]
    fn get_none_ok() {
        let state = State::new();

        let key: Identifier = "key".into();

        let result = state.get(&key);
        assert!(result.is_none());
    }

    #[test]
    fn get_some_recursive_ok() {
        let mut state = State::new();

        let key: Identifier = "key.other".into();
        let value = Item::Value(Value::StringValue("123".into()));

        let _ = state.set(key.clone(), value.clone());

        let result = state.get(&key);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &value);
    }

    #[test]
    fn get_none_recursive_ok() {
        let mut state = State::new();

        let key: Identifier = "key.other".into();
        let value = Item::Value(Value::StringValue("123".into()));

        let _ = state.set("key.cat".into(), value.clone());

        let result = state.get(&key);
        assert!(result.is_none());
    }

    #[test]
    fn get_some_partial_recursive_ok() {
        let mut state = State::new();

        let key: Identifier = "key".into();
        let value = Item::Value(Value::StringValue("123".into()));

        let _ = state.set("key.other".into(), value.clone());

        let result = state.get(&key);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), {
            let mut map = HashMap::new();
            map.insert("other".into(), value.clone());
            &Item::Map(map)
        })
    }

    #[test]
    fn get_array_element_ok() {
        let mut state = State::new();

        let key: Identifier = "key.1".into();
        let target = Item::Value(Value::StringValue("321".into()));
        let value = Item::Vec(vec!(
            Item::Value(Value::StringValue("123".into())),
            target.clone(),
        ));

        let _ = state.set("key".into(), value.clone());

        let result = state.get(&key);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &target)
    }

    #[test]
    fn get_array_element_nested_ok() {
        let mut state = State::new();

        let key: Identifier = "key.0.0".into();
        let target = Item::Value(Value::StringValue("321".into()));
        let value = Item::Vec(vec!(
            Item::Vec(vec!(
                target.clone(),
            ))
        ));

        let _ = state.set("key".into(), value.clone());

        let result = state.get(&key);
        assert!(result.is_some());
        assert_eq!(result.unwrap(), &target)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(untagged)]
pub enum Item {
    Value(Value),
    Vec(Vec<Item>),
    Map(HashMap<String, Item>),
}

impl Item {
    pub fn type_name(&self) -> &str {
        match self {
            Item::Value(i) => { i.type_name() }
            Item::Vec(_) => { "Array" }
            Item::Map(_) => { "Map" }
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Eq, PartialEq)]
#[serde(untagged)]
pub enum Value {
    None,
    IntValue(i64),
    StringValue(String),
}

impl Value {
    pub fn type_name(&self) -> &str {
        match self {
            Value::None => { "None" }
            Value::IntValue(_) => { "Int" }
            Value::StringValue(_) => { "String" }
        }
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct Identifier(String);

impl Identifier {
    pub fn split(&self) -> (Option<String>, Option<Identifier>) {
        let mut iter = self.0.split(".");
        let current = iter.next().map(|s| String::from(s));
        let rest = iter.collect::<Vec<_>>().join(".");

        (current, if rest.len() == 0 { None } else { Some(rest.into()) })
    }
}

impl std::fmt::Display for Identifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for Identifier {
    fn from(s: String) -> Self {
        Identifier(s)
    }
}

impl From<&str> for Identifier {
    fn from(s: &str) -> Self {
        Identifier(String::from(s))
    }
}

impl From<std::num::ParseIntError> for Error {
    fn from(e: ParseIntError) -> Self {
        Error::InvalidIndex { reason: e.to_string() }
    }
}