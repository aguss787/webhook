use std::collections::HashMap;

use serde::Deserialize;

use crate::event::process;
use crate::event::process::{Identifier, Item, State, Value};
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
    use crate::event::process::operation::{Op, SetEnv};
    use crate::event::process::*;

    use super::*;

    #[test]
    fn test_set_env_ok() {
        let mut state = State::new();
        let _ = state.set(Identifier::from("o"), Item::Value(Value::None));

        let key = Identifier::from("key");
        let item = Item::Value(Value::IntValue(123));
        let value = Box::new(Expression::Item(item.clone()));

        let op = Op::SetEnv {
            set_env: SetEnv {
                target: key.clone().into(),
                value,
            },
        };
        let payload = crate::event::sender::Payload::new(vec![]);

        let res = op.execute(payload, state);
        assert!(res.is_ok());

        let (_, state) = res.unwrap();

        assert_eq!(state.len(), 2);
        assert!(state.get(&key).is_some());
        assert_eq!(state.get(&key).unwrap(), &item);
    }

    #[test]
    fn test_to_payload_ok() {
        let mut state = State::new();
        let item = Item::Value(Value::IntValue(123));
        let value = Box::new(Expression::Item(item.clone()));

        let _ = state.set(Identifier::from("o"), item.clone());

        let op = Op::ToPayload {
            to_payload: ToPayload {
                value,
                format: PayloadFormat::Json,
            },
        };
        let payload = crate::event::sender::Payload::new(vec![]);

        let res = op.execute(payload, state);
        assert!(res.is_ok());

        let (payload, _) = res.unwrap();
        assert!(payload.content.len() > 0);
        assert_eq!(payload.content, "123".as_bytes());
    }
}

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum Expression {
    SetEnv { set_env: SetEnv },
    GetEnv { get_env: Identifier },
    FromJson { from_json: String },
    FromPayload { from_payload: PayloadFormat },
    AsMap { as_map: HashMap<String, Expression> },
    Item(Item),
}

impl Expression {
    pub fn evaluate(
        &self,
        payload: Payload,
        state: State,
    ) -> process::Result<(Item, Payload, State)> {
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
            Expression::FromPayload {
                from_payload: format,
            } => {
                let item = format.parse_payload(&payload)?;
                Ok((item, payload, state))
            }
            Expression::Item(i) => Ok((i.clone(), payload, state)),
            Expression::FromJson { .. } => {
                unimplemented!()
            }
            Expression::AsMap { as_map: map } => {
                let (map, payload, state) = map.iter().fold(
                    Ok((HashMap::new(), payload, state)),
                    |acc: process::Result<_>, (key, expr)| {
                        let (mut acc, payload, state) = acc?;
                        let (item, payload, state) = expr.evaluate(payload, state)?;
                        acc.insert(key.clone(), item);
                        Ok((acc, payload, state))
                    },
                )?;

                Ok((Item::Map(map), payload, state))
            }
        }
    }
}

#[cfg(test)]
mod expression_tests {
    use crate::event::process::operation::SetEnv;
    use crate::event::process::*;

    use super::*;

    #[test]
    fn test_set_env_ok() {
        let mut state = State::new();
        let _ = state.set(Identifier::from("o"), Item::Value(Value::None));

        let key = Identifier::from("key");
        let item = Item::Value(Value::IntValue(123));
        let value = Box::new(Expression::Item(item.clone()));

        let exp = Expression::SetEnv {
            set_env: SetEnv {
                target: key.clone().into(),
                value,
            },
        };
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

        let exp = Expression::GetEnv {
            get_env: key.clone().into(),
        };
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

    #[test]
    fn test_as_map_ok() {
        let env_id = Identifier("id".into());
        let env_value = Item::Value(Value::StringValue("test".into()));
        let state = {
            let mut state = State::new();

            let _ = state.set(env_id.clone(), env_value.clone());

            state
        };

        let new_item = Item::Value(Value::IntValue(123));
        let to_env_id = Identifier("to_id".into());
        let to_env_item = Item::Value(Value::IntValue(123));

        let map = {
            let mut res = HashMap::new();

            res.insert(
                String::from("from_env"),
                Expression::GetEnv {
                    get_env: env_id.clone(),
                },
            );
            res.insert(String::from("value"), Expression::Item(new_item.clone()));
            res.insert(
                String::from("to_env"),
                Expression::SetEnv {
                    set_env: SetEnv {
                        target: to_env_id.clone(),
                        value: Box::new(Expression::Item(to_env_item.clone())),
                    },
                },
            );

            res
        };
        let exp = Expression::AsMap { as_map: map };
        let payload = crate::event::sender::Payload::new(vec![]);

        let exp_res = exp.evaluate(payload, state);
        assert!(exp_res.is_ok());
        let (item, _, state) = exp_res.unwrap();

        assert!(matches!(item, Item::Map(_)));
        let map = match item {
            Item::Map(m) => m,
            _ => unreachable!(),
        };

        assert_eq!(map.len(), 3);

        assert_eq!(map.get(&String::from("from_env")), Some(&env_value));
        assert_eq!(map.get(&String::from("value")), Some(&new_item));
        assert_eq!(map.get(&String::from("to_env")), Some(&to_env_item));

        assert_eq!(state.len(), 2);
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
            PayloadFormat::Yaml => serde_yaml::to_vec(&i)?,
            PayloadFormat::Json => serde_json::to_vec(&i)?,
        })
    }

    pub fn parse_payload(&self, payload: &Payload) -> super::Result<Item> {
        Ok(match self {
            PayloadFormat::Yaml => serde_yaml::from_slice(payload.content.as_slice().clone())?,
            PayloadFormat::Json => serde_json::from_slice(payload.content.as_slice().clone())?,
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
