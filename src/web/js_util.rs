use web_sys::js_sys::{Function, Reflect};
use wasm_bindgen::{JsCast, JsValue};

pub fn call_method0(target: &JsValue, method: &str) -> Result<JsValue, String> {
    let method_value = Reflect::get(target, &JsValue::from_str(method))
        .map_err(|_| format!("Missing method {method}."))?;
    let function = method_value
        .dyn_ref::<Function>()
        .ok_or_else(|| format!("{method} is not callable."))?;
    function
        .call0(target)
        .map_err(|_| format!("Failed to call {method}."))
}

pub fn call_method1(target: &JsValue, method: &str, arg: &JsValue) -> Result<JsValue, String> {
    let method_value = Reflect::get(target, &JsValue::from_str(method))
        .map_err(|_| format!("Missing method {method}."))?;
    let function = method_value
        .dyn_ref::<Function>()
        .ok_or_else(|| format!("{method} is not callable."))?;
    function
        .call1(target, arg)
        .map_err(|_| format!("Failed to call {method}."))
}

pub fn get_string_field(target: &JsValue, key: &str) -> Option<String> {
    Reflect::get(target, &JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_string())
}

pub fn get_number_field(target: &JsValue, key: &str) -> Option<f64> {
    Reflect::get(target, &JsValue::from_str(key))
        .ok()
        .and_then(|v| v.as_f64())
}