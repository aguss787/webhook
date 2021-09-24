use std::collections::HashMap;

use serde::Deserialize;

use crate::event::process::{Identifier, Item, State, Value};
use crate::event::process;
use crate::event::sender::Payload;

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Op {
    SetEnv { set_env: SetEnv },
    ToPayload { to_payload: ToPayload },
}

impl Op {
    pub fn execute(&self, payload: Payload, state: State) -> process::Result<(Payload, State)> {
        match self {
            Op::SetEnv { set_env } => {
                let (value, payload, mut new_state) = set_env.value.evaluate(payload, state)?;
                let idx = set_env.target.clone();
                log::debug!("setting env with key {} as {:?}", idx, value);
                new_state.set(idx, value)?;
                Ok((payload, new_state))
            }
            Op::ToPayload { to_payload } => {
                let (item, _, state) = to_payload.value.evaluate(payload, state)?;

                let item_bytes = to_payload.format.to_vec(&item)?;
                let payload = Payload::new(item_bytes);

                Ok((payload, state))
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
        let _ = state.set(Identifier::from("o"), Item::Value(Value::None));

        let key = Identifier::from("key");
        let item = Item::Value(Value::IntValue(123));
        let value = Box::new(Expression::Item(item.clone()));

        let op = Op::SetEnv { set_env: SetEnv { target: key.clone().into(), value } };
        let payload = crate::event::sender::Payload::new(vec![]);

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
    FromPayload { from_payload: PayloadFormat },
    Item(Item),
    AsMap { as_map: HashMap<String, Expression> },
}

impl Expression {
    pub fn evaluate(&self, payload: Payload, state: State) -> process::Result<(Item, Payload, State)> {
        match self {
            Expression::SetEnv { set_env } => {
                let (value, payload, mut new_state) = set_env.value.evaluate(payload, state)?;
                let idx = set_env.target.clone();
                log::trace!("setting env with key {} as {:?}", idx, value);
                new_state.set(idx, value.clone())?;
                Ok((value, payload, new_state))
            }
            Expression::GetEnv { get_env } => {
                let value = state.get(&get_env);
                let item = value
                    .and_then(|o| Some(o.clone()))
                    .unwrap_or(Item::Value(Value::None));
                Ok((item, payload, state))
            }
            Expression::FromPayload { from_payload: format } => {
                let item = format.parse_payload(&payload)?;
                Ok((item, payload, state))
            }
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
        let _ = state.set(Identifier::from("o"), Item::Value(Value::None));

        let key = Identifier::from("key");
        let item = Item::Value(Value::IntValue(123));
        let value = Box::new(Expression::Item(item.clone()));

        let exp = Expression::SetEnv { set_env: SetEnv { target: key.clone().into(), value } };
        let payload = crate::event::sender::Payload::new(vec![]);

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
        let key = Identifier::from("key");
        let item = Item::Value(Value::IntValue(123));

        let _ = state.set(key.clone(), item.clone());

        let exp = Expression::GetEnv { get_env: key.clone().into() };
        let payload = crate::event::sender::Payload::new(vec![]);

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
        let payload = crate::event::sender::Payload::new(vec![]);

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
pub struct ToPayload {
    format: PayloadFormat,
    value: Box<Expression>,
}

#[derive(Deserialize, Debug, Clone)]
#[serde(rename_all = "lowercase")]
pub enum PayloadFormat {
    Yaml,
    Json,
}

impl PayloadFormat {
    pub fn to_vec(&self, i: &Item) -> super::Result<Vec<u8>> {
        Ok(match self {
            PayloadFormat::Yaml => { serde_yaml::to_vec(&i)? }
            PayloadFormat::Json => { serde_json::to_vec(&i)? }
        })
    }

    pub fn parse_payload(&self, payload: &Payload) -> super::Result<Item> {
        Ok(match self {
            PayloadFormat::Yaml => { serde_yaml::from_slice(payload.content.as_slice().clone())? }
            PayloadFormat::Json => { serde_json::from_slice(payload.content.as_slice().clone())? }
        })
    }
}

impl From<serde_json::Error> for super::Error {
    fn from(_: serde_json::Error) -> Self {
        unimplemented!()
    }
}

impl From<serde_yaml::Error> for super::Error {
    fn from(_: serde_yaml::Error) -> Self {
        unimplemented!()
    }
}