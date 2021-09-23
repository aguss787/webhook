use serde::Deserialize;

use crate::event::process::{Item, State, Value};
use crate::event::process;
use crate::event::sender::Payload;
use std::fmt::Formatter;
use std::collections::HashMap;

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Op {
    SetEnv { set_env: SetEnv },
}

impl Op {
    pub fn execute(&self, payload: Payload, state: State) -> process::Result<(Payload, State)> {
        match self {
            Op::SetEnv { set_env } => {
                let (value, payload, mut new_state) = set_env.value.evaluate(payload, state)?;
                let idx = set_env.target.clone();
                log::debug!("setting env with key {} as {:?}", idx, value);
                new_state.entry(idx.to_string())
                    .and_modify(|i| { *i = value.clone(); })
                    .or_insert(value.clone());
                Ok((payload, new_state))
            }
        }
    }
}

#[cfg(test)]
mod op_tests {
    use crate::event::process::*;
    use crate::event::process::operation::{Op, SetEnv};
    use super::*;

    #[test]
    fn test_set_env_ok() {
        let mut state = State::new();
        state.insert(String::from("o"), Item::Value(Value::None));

        let key = String::from("key");
        let item = Item::Value(Value::IntValue(123));
        let value = Box::new(Expression::Item(item.clone()));

        let op = Op::SetEnv { set_env: SetEnv { target: key.clone().into(), value } };
        let payload = crate::event::sender::Payload::new();

        let res = op.execute(payload, state);
        assert!(res.is_ok());

        let (_, state) = res.unwrap();

        assert_eq!(state.len(), 2);
        assert!(state.get(&key).is_some());
        assert_eq!(state.get(&key).unwrap(), &item);
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Expression {
    SetEnv { set_env: SetEnv },
    GetEnv { get_env: Identifier },
    FromJson { from_json: String },
    FromPayload { from_payload: FromPayloadOp },
    ToPayload { to_payload: Identifier },
    Item(Item),
    AsMap { as_map: HashMap<String, Expression> }
}

impl Expression {
    pub fn evaluate(&self, payload: Payload, state: State) -> process::Result<(Item, Payload, State)> {
        match self {
            Expression::SetEnv { set_env } => {
                let (value, payload, mut new_state) = set_env.value.evaluate(payload, state)?;
                let idx = set_env.target.clone();
                log::trace!("setting env with key {} as {:?}", idx, value);
                new_state.entry(idx.to_string())
                    .and_modify(|i| { *i = value.clone(); })
                    .or_insert(value.clone());
                Ok((value, payload, new_state))
            }
            Expression::GetEnv { get_env } => {
                let value = state.get(&get_env.to_string());
                let item = value
                    .and_then(|o| Some(o.clone()))
                    .unwrap_or(Item::Value(Value::None));
                Ok((item, payload, state))
            }
            Expression::FromPayload { .. } => { unimplemented!() }
            Expression::ToPayload { .. } => { unimplemented!() }
            Expression::Item(i) => { Ok((i.clone(), payload, state)) }
            Expression::FromJson { .. } => { unimplemented!() }
            Expression::AsMap { .. } => { unimplemented!() }
        }
    }
}


#[cfg(test)]
mod expression_tests {
    use crate::event::process::*;
    use crate::event::process::operation::SetEnv;
    use super::*;

    #[test]
    fn test_set_env_ok() {
        let mut state = State::new();
        state.insert(String::from("o"), Item::Value(Value::None));

        let key = String::from("key");
        let item = Item::Value(Value::IntValue(123));
        let value = Box::new(Expression::Item(item.clone()));

        let exp = Expression::SetEnv { set_env: SetEnv { target: key.clone().into(), value } };
        let payload = crate::event::sender::Payload::new();

        let res = exp.evaluate(payload, state);
        assert!(res.is_ok());

        let (ret_item, _, state) = res.unwrap();

        assert_eq!(state.len(), 2);
        assert!(state.get(&key).is_some());
        assert_eq!(state.get(&key).unwrap(), &item);

        assert_eq!(ret_item, item);
    }

    #[test]
    fn test_get_env_ok() {
        let mut state = State::new();
        let key = String::from("key");
        let item = Item::Value(Value::IntValue(123));

        state.insert(key.clone(), item.clone());

        let exp = Expression::GetEnv { get_env: key.clone().into() };
        let payload = crate::event::sender::Payload::new();

        let res = exp.evaluate(payload, state);
        assert!(res.is_ok());

        let (ret_item, _, state) = res.unwrap();

        assert_eq!(state.len(), 1);
        assert!(state.get(&key).is_some());
        assert_eq!(state.get(&key).unwrap(), &item);

        assert_eq!(ret_item, item);
    }

    #[test]
    fn test_item_ok() {
        let state = State::new();

        let item = Item::Value(Value::IntValue(123));
        let exp = Expression::Item(item.clone());
        let payload = crate::event::sender::Payload::new();

        let res = exp.evaluate(payload, state);
        assert!(res.is_ok());

        let (ret_item, _, state) = res.unwrap();

        assert_eq!(state.len(), 0);
        assert_eq!(ret_item, item);
    }
}

#[derive(Deserialize, Debug, Clone)]
pub struct SetEnv {
    target: Identifier,
    value: Box<Expression>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum FromPayloadOp {
    Json { json: Identifier },
}

#[derive(Deserialize, Debug, Clone)]
pub struct Identifier(String);

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
