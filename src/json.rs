use crate::context::Context;
use crate::error::Error;
use crate::error::Result;
use crate::string::String;
use crate::value::{FromLua, Nil, ToLua, Value};

use serde_json::{Map as JsonMap, Number as JsonNumber, Value as JsonValue};

impl<'lua> ToLua<'lua> for &JsonValue {
    fn to_lua(self, lua: Context<'lua>) -> Result<Value<'lua>> {
        Ok(match self {
            JsonValue::Null => Value::Nil,
            JsonValue::Bool(b) => Value::Boolean(*b),
            JsonValue::Number(n) => {
                if let Some(n) = n.as_i64() {
                    Value::Integer(n)
                } else if let Some(n) = n.as_f64() {
                    Value::Number(n)
                } else {
                    Err(Error::ToLuaConversionError {
                        from: "serde_json::Number",
                        to: "Integer",
                        message: Some(format!("value {} too large", n)),
                    })?
                }
            }
            JsonValue::String(s) => lua.create_string(s).map(Value::String)?,
            JsonValue::Array(values) => Value::Table(lua.create_sequence_from(values.iter())?),
            JsonValue::Object(items) => {
                let table = lua.create_table()?;

                for (key, value) in items {
                    let key = lua.create_string(key)?;
                    table.set(key, value)?;
                }

                Value::Table(table)
            }
        })
    }
}

impl<'lua> FromLua<'lua> for JsonValue {
    fn from_lua(lua_value: Value<'lua>, lua: Context<'lua>) -> Result<Self> {
        Ok(match lua_value {
            Value::Nil => JsonValue::Null,
            Value::Boolean(b) => JsonValue::Bool(b),
            Value::LightUserData(_) => Err(Error::FromLuaConversionError {
                from: "LightUserData",
                to: "serde_json::Value",
                message: Some("not supported".to_string()),
            })?,
            Value::Integer(i) => JsonValue::Number(i.into()),
            Value::Number(n) => JsonValue::Number(JsonNumber::from_f64(n).ok_or_else(|| {
                Error::FromLuaConversionError {
                    from: "Number",
                    to: "serde_json::Number",
                    message: Some(format!("value {} not supported", n)),
                }
            })?),
            Value::String(s) => JsonValue::String(s.to_str()?.to_string()),
            Value::Table(t) => {
                if t.len()? == 0 {
                    // There's no way to know whether it's supposed to be an
                    // object or an array.
                    JsonValue::Object(JsonMap::new())
                } else if let Ok(Nil) = t.get(1) {
                    // It's probably a sequence.
                    let values = t
                        .sequence_values()
                        .map(|r: Result<Value>| r.and_then(|v| JsonValue::from_lua(v, lua)))
                        .collect::<Result<_>>()?;

                    JsonValue::Array(values)
                } else {
                    // XXX: maybe call a metamethod here?
                    let items = t
                        .pairs()
                        .map(|r: Result<(String, Value)>| {
                            r.and_then(|(k, v)| {
                                Ok((k.to_str()?.to_string(), JsonValue::from_lua(v, lua)?))
                            })
                        })
                        .collect::<Result<_>>()?;

                    JsonValue::Object(items)
                }
            }
            Value::Function(_) => Err(Error::FromLuaConversionError {
                from: "Function",
                to: "serde_json::Value",
                message: Some("not supported".to_string()),
            })?,
            Value::Thread(_) => Err(Error::FromLuaConversionError {
                from: "Thread",
                to: "serde_json::Value",
                message: Some("not supported".to_string()),
            })?,

            Value::UserData(_) => Err(Error::FromLuaConversionError {
                from: "AnyUserData",
                to: "serde_json::Value",
                message: Some("not supported".to_string()),
            })?,
            Value::Error(e) => Err(e)?,
        })
    }
}
