//! Rhai: cada operación es una función global que recibe un mapa (`#{...}`) o nada.

use std::cell::RefCell;
use std::rc::Rc;

use rhai::{Dynamic, Engine, EvalAltResult, Map};

use crate::host::Host;
use crate::{MAX_OPERATIONS, Outcome, ScriptError};

type Call = Result<Dynamic, Box<EvalAltResult>>;

pub(crate) fn run(source: &str, host: Host) -> Result<Outcome, ScriptError> {
    let host = Rc::new(RefCell::new(host));
    let mut engine = Engine::new();
    engine.set_max_operations(MAX_OPERATIONS);
    let call = {
        let host = host.clone();
        move |name: &str, params: Dynamic| -> Call {
            let params: serde_json::Value = rhai::serde::from_dynamic(&params)?;
            let result = host.borrow_mut().call(name, params).map_err(|e| {
                Box::new(EvalAltResult::ErrorRuntime(e.into(), rhai::Position::NONE))
            })?;
            rhai::serde::to_dynamic(result)
        }
    };
    for &name in cs_agent::OPERATIONS {
        let with = call.clone();
        engine.register_fn(name, move |params: Map| {
            with(name, Dynamic::from_map(params))
        });
        let without = call.clone();
        engine.register_fn(name, move || without(name, Dynamic::UNIT));
    }
    let generic = call.clone();
    engine.register_fn("op", move |name: &str, params: Map| {
        generic(name, Dynamic::from_map(params))
    });
    let generic = call;
    engine.register_fn("op", move |name: &str| generic(name, Dynamic::UNIT));
    let context = host.borrow().context;
    engine.register_fn("current_layer", move || context.layer as i64);
    engine.register_fn("current_frame", move || context.frame as i64);
    engine.register_fn("current_artboard", move || context.artboard as i64);
    {
        let host = host.clone();
        engine.on_print(move |line| host.borrow_mut().print(line.to_string()));
    }
    {
        let host = host.clone();
        engine.on_debug(move |line, _, _| host.borrow_mut().print(line.to_string()));
    }
    let result = engine.run(source);
    drop(engine);
    let host = Rc::try_unwrap(host)
        .ok()
        .expect("the engine is gone")
        .into_inner();
    match result {
        Ok(()) => host.finish(),
        Err(e) => Err(host.fail(e.to_string())),
    }
}
