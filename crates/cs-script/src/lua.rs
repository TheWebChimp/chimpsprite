//! Lua 5.4: cada operación es una función global que recibe una tabla (`draw_line{...}`) o nada.

use std::cell::RefCell;
use std::rc::Rc;

use mlua::{HookTriggers, Lua, LuaSerdeExt, Value, VmState};

use crate::host::Host;
use crate::{MAX_OPERATIONS, Outcome, ScriptError};

/// Cada cuántas instrucciones se cuenta (para el tope de `MAX_OPERATIONS`).
const HOOK_EVERY: u32 = 10_000;

pub(crate) fn run(source: &str, host: Host) -> Result<Outcome, ScriptError> {
    let host = Rc::new(RefCell::new(host));
    let result = (|| -> mlua::Result<()> {
        let lua = Lua::new();
        let globals = lua.globals();
        for &name in cs_agent::OPERATIONS.iter().chain(crate::host::HELPERS) {
            let host = host.clone();
            let f = lua.create_function(move |lua, params: Value| {
                let params: serde_json::Value = match params {
                    Value::Nil => serde_json::Value::Null,
                    other => lua.from_value(other)?,
                };
                let result = host
                    .borrow_mut()
                    .call(name, params)
                    .map_err(mlua::Error::runtime)?;
                lua.to_value_with(&result, nil_for_null())
            })?;
            globals.set(name, f)?;
        }
        {
            let host = host.clone();
            let op = lua.create_function(move |lua, (name, params): (String, Value)| {
                let params: serde_json::Value = match params {
                    Value::Nil => serde_json::Value::Null,
                    other => lua.from_value(other)?,
                };
                let result = host
                    .borrow_mut()
                    .call(&name, params)
                    .map_err(mlua::Error::runtime)?;
                lua.to_value_with(&result, nil_for_null())
            })?;
            globals.set("op", op)?;
        }
        let context = host.borrow().context.clone();
        {
            let host = host.clone();
            let random = lua.create_function(move |_, (a, b): (Option<i64>, Option<i64>)| {
                let mut host = host.borrow_mut();
                Ok(match (a, b) {
                    (Some(a), Some(b)) => mlua::Value::Integer(host.random_between(a, b)),
                    _ => mlua::Value::Number(host.random()),
                })
            })?;
            globals.set("random", random)?;
        }
        globals.set(
            "current_layer",
            lua.create_function(move |_, ()| Ok(context.layer))?,
        )?;
        globals.set(
            "current_frame",
            lua.create_function(move |_, ()| Ok(context.frame))?,
        )?;
        globals.set(
            "current_artboard",
            lua.create_function(move |_, ()| Ok(context.artboard))?,
        )?;
        {
            let host = host.clone();
            let print = lua.create_function(move |_, args: mlua::Variadic<Value>| {
                let line = args
                    .iter()
                    .map(|v| v.to_string().unwrap_or_else(|_| format!("{v:?}")))
                    .collect::<Vec<_>>()
                    .join("\t");
                host.borrow_mut().print(line);
                Ok(())
            })?;
            globals.set("print", print)?;
        }
        // Tope de instrucciones.
        let count = std::cell::Cell::new(0u64);
        lua.set_hook(
            HookTriggers::new().every_nth_instruction(HOOK_EVERY),
            move |_, _| {
                count.set(count.get() + HOOK_EVERY as u64);
                if count.get() > MAX_OPERATIONS {
                    Err(mlua::Error::runtime("the script ran for too long"))
                } else {
                    Ok(VmState::Continue)
                }
            },
        )?;
        lua.load(source).set_name("script").exec()
    })();
    let host = Rc::try_unwrap(host)
        .ok()
        .expect("the interpreter is gone")
        .into_inner();
    match result {
        Ok(()) => host.finish(),
        Err(e) => Err(host.fail(e.to_string())),
    }
}

/// El `null` de JSON es `nil` en Lua (si no, sería un valor "verdadero" y `if x then` fallaría).
fn nil_for_null() -> mlua::SerializeOptions {
    mlua::SerializeOptions::new()
        .serialize_none_to_null(false)
        .serialize_unit_to_null(false)
}
