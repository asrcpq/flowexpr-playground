use std::cell::RefCell;
use std::rc::Rc;

use indexmap::IndexMap;
use reearth_flow_expr::{compile, default_env, eval, NativeFn, Value};
use wasm_bindgen::prelude::*;

fn json_to_value(v: &serde_json::Value) -> Value {
    match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Bool(*b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() {
                Value::Int(i)
            } else {
                Value::Float(n.as_f64().unwrap_or(f64::NAN))
            }
        }
        serde_json::Value::String(s) => Value::String(s.clone()),
        serde_json::Value::Array(arr) => Value::array(arr.iter().map(json_to_value).collect()),
        serde_json::Value::Object(obj) => {
            let mut map = IndexMap::new();
            for (k, v) in obj {
                map.insert(k.clone(), json_to_value(v));
            }
            Value::map(map)
        }
    }
}

/// Evaluate `expr` against the variables in `env_json` (a JSON object).
///
/// Returns a JSON string: `{"ok":true,"result":"...","output":[...]}` on
/// success, or `{"ok":false,"error":"...","output":[...]}` on failure.
#[wasm_bindgen]
pub fn eval_expr(expr: &str, env_json: &str) -> String {
    let captured: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    let mut env = default_env();

    // Replace print to capture output instead of writing to stdout.
    let cap = captured.clone();
    env.insert(
        "print".into(),
        Value::Fn(NativeFn::new(move |args| {
            let line = args
                .iter()
                .map(|v| match v {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                })
                .collect::<Vec<_>>()
                .join(" ");
            cap.borrow_mut().push(line);
            Ok(Value::Null)
        })),
    );

    let env_str = if env_json.trim().is_empty() { "{}" } else { env_json };
    match serde_json::from_str::<serde_json::Value>(env_str) {
        Ok(serde_json::Value::Object(obj)) => {
            for (k, v) in obj {
                env.insert(k, json_to_value(&v));
            }
        }
        Ok(_) => {
            return serde_json::json!({
                "ok": false,
                "error": "environment must be a JSON object",
                "output": []
            })
            .to_string();
        }
        Err(e) => {
            return serde_json::json!({
                "ok": false,
                "error": format!("invalid environment JSON: {e}"),
                "output": []
            })
            .to_string();
        }
    }

    let output = || captured.borrow().clone();

    let compiled = match compile(expr) {
        Ok(c) => c,
        Err(e) => {
            return serde_json::json!({
                "ok": false,
                "error": e.to_string(),
                "output": output()
            })
            .to_string();
        }
    };

    match eval(&compiled, &mut env) {
        Ok(v) => serde_json::json!({
            "ok": true,
            "result": v.to_string(),
            "output": output()
        })
        .to_string(),
        Err(e) => serde_json::json!({
            "ok": false,
            "error": e.to_string(),
            "output": output()
        })
        .to_string(),
    }
}
