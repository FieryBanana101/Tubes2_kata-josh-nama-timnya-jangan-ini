use wasm_bindgen::prelude::*;
use wasm_bindgen::JsValue;
use web_sys::js_sys::Function;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_name = alert)]
    pub fn alert(message: &str);

    #[wasm_bindgen(js_namespace = d3, js_name = hierarchy)]
    pub fn d3_hierarchy(data: &JsValue, children_accessor: &Function) -> JsValue;

    #[wasm_bindgen(js_namespace = d3, js_name = tree)]
    pub fn d3_tree() -> JsValue;
}
