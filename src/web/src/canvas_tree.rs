use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use web_sys::js_sys::{Array, Function, Reflect, JSON};
use web_sys::{Document, Element, HtmlElement};
use yew::prelude::*;

use crate::bindings::{alert, d3_hierarchy, d3_tree};
use crate::js_util::*;
use crate::GraphContext;

const TREE_CONTAINER_ID: &str = "graph";
const SVG_NS_URL: &str = "http://www.w3.org/2000/svg";
const XHTML_NS_URL: &str = "http://www.w3.org/1999/xhtml";

const CARD_WIDTH: f64 = 200.0;
const CARD_HEIGHT: f64 = 100.0;
const NODE_X_GAP: f64 = 300.0;
const NODE_Y_GAP: f64 = 200.0;
const PADDING_TOP: f64 = 240.0;
const PADDING_RIGHT: f64 = 240.0;
const PADDING_BOTTOM: f64 = 240.0;
const PADDING_LEFT: f64 = 240.0;

#[derive(Clone)]
struct RenderNode {
    x: f64,
    y: f64,
    tag: String,
    class_name: String,
    id_attr: String,
    node_index: i32,
}

#[derive(Clone)]
struct RenderLink {
    source_x: f64,
    source_y: f64,
    target_x: f64,
    target_y: f64,
}

#[component]
pub fn CanvasTree() -> Html {
    let ctx = use_context::<GraphContext>().unwrap();

    use_effect_with(ctx.clone(), move |ctx| {
        let json = ctx.graph_data.clone();
        render_tree(&json);
        || ()
    });

    html! {
        <div id={TREE_CONTAINER_ID} class="h-screen w-screen w-full overflow-auto"></div>
    }
}

fn render_tree(raw_json: &str) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(container) = document.get_element_by_id(TREE_CONTAINER_ID) else {
        return;
    };

    let raw_json = if raw_json.trim().is_empty() {
        "{
            \"root_index\": -1,
            \"nodes\": [],
            \"results\": [],
            \"selected_nodes\": []
        }"
    } else {
        raw_json
    };

    clear_children(&container);

    let d3_available = Reflect::get(&window, &JsValue::from_str("d3"))
        .ok()
        .map(|v| !v.is_undefined() && !v.is_null())
        .unwrap_or(false);

    if !d3_available {
        alert("D3 is not loaded.");
        return;
    }

    let parsed = match JSON::parse(raw_json) {
        Ok(value) => value,
        Err(_) => {
            alert("Invalid tree JSON.");
            return;
        }
    };

    let layout = match compute_layout_with_d3(&parsed, &container) {
        Ok(value) => value,
        Err(msg) => {
            alert(&msg);
            return;
        }
    };

    if let Err(msg) = draw_svg(&document, &container, &layout) {
        alert(&msg);
    }
}

struct LayoutResult {
    nodes: Vec<RenderNode>,
    links: Vec<RenderLink>,
    width: f64,
    height: f64,
    offset_x: f64,
    offset_y: f64,
}

fn compute_layout_with_d3(parsed: &JsValue, container: &Element) -> Result<LayoutResult, String> {
    let root_index_value = Reflect::get(parsed, &JsValue::from_str("root_index"))
        .map_err(|_| "Missing root_index.".to_string())?;
    let root_index = root_index_value
        .as_f64()
        .ok_or_else(|| "root_index must be a number.".to_string())? as i32;

    if root_index == -1i32 {
        return Ok(LayoutResult {
            nodes: vec![],
            links: vec![],
            width: 0.0,
            height: 0.0,
            offset_x: 0.0,
            offset_y: 0.0,
        });
    }

    let nodes_value = Reflect::get(parsed, &JsValue::from_str("nodes"))
        .map_err(|_| "Missing nodes array.".to_string())?;
    let nodes_array = Array::from(&nodes_value);
    if nodes_array.length() == 0 {
        return Err("nodes must not be empty.".to_string());
    }

    if root_index >= nodes_array.length().try_into().unwrap() || root_index < 0 {
        return Err("root_index is out of range.".to_string());
    }

    let root_node_data = nodes_array.get(root_index.try_into().unwrap());
    let nodes_for_children = nodes_array.clone();
    let children_accessor = Closure::<dyn FnMut(JsValue) -> JsValue>::new(move |node: JsValue| {
        let children_value =
            Reflect::get(&node, &JsValue::from_str("children")).unwrap_or(JsValue::UNDEFINED);
        if children_value.is_undefined() || children_value.is_null() {
            return Array::new().into();
        }

        let child_indexes = Array::from(&children_value);
        let resolved_children = Array::new();
        for idx in 0..child_indexes.length() {
            let node_index = child_indexes.get(idx).as_f64().unwrap_or(-1.0);
            if node_index >= 0.0 {
                let child = nodes_for_children.get(node_index as u32);
                if !child.is_undefined() && !child.is_null() {
                    resolved_children.push(&child);
                }
            }
        }

        resolved_children.into()
    });

    let hierarchy_root = d3_hierarchy(
        &root_node_data,
        children_accessor.as_ref().unchecked_ref::<Function>(),
    );

    let tree_layout = d3_tree();
    let node_size = Array::new();
    node_size.push(&JsValue::from_f64(NODE_Y_GAP));
    node_size.push(&JsValue::from_f64(NODE_X_GAP));
    let _ = call_method1(&tree_layout, "nodeSize", &node_size.into())?;

    let tree_function = tree_layout
        .dyn_ref::<Function>()
        .ok_or_else(|| "d3.tree() did not return a callable".to_string())?;
    let _ = tree_function.call1(&JsValue::NULL, &hierarchy_root);

    let descendants_value = call_method0(&hierarchy_root, "descendants")?;
    let descendants = Array::from(&descendants_value);
    if descendants.length() == 0 {
        return Err("Tree has no descendants".to_string());
    }

    let links_value = call_method0(&hierarchy_root, "links")?;
    let links_array = Array::from(&links_value);

    let mut nodes: Vec<RenderNode> = Vec::new();
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;

    for i in 0..descendants.length() {
        let item = descendants.get(i);
        let x = get_number_field(&item, "x").unwrap_or(0.0);
        let y = get_number_field(&item, "y").unwrap_or(0.0);
        x_min = x_min.min(x);
        x_max = x_max.max(x);
        y_min = y_min.min(y);
        y_max = y_max.max(y);

        let data = Reflect::get(&item, &JsValue::from_str("data")).unwrap_or(JsValue::NULL);
        let tag = get_string_field(&data, "tag").unwrap_or_else(|| "div".to_string());
        let class_name = get_string_field(&data, "class").unwrap_or_default();
        let id_attr = get_string_field(&data, "id").unwrap_or_default();
        let node_index = get_number_field(&data, "index").unwrap_or(i as f64) as i32;

        nodes.push(RenderNode {
            x,
            y,
            tag,
            class_name,
            id_attr,
            node_index,
        });
    }

    let mut links: Vec<RenderLink> = Vec::new();
    for i in 0..links_array.length() {
        let link = links_array.get(i);
        let source = Reflect::get(&link, &JsValue::from_str("source")).unwrap_or(JsValue::NULL);
        let target = Reflect::get(&link, &JsValue::from_str("target")).unwrap_or(JsValue::NULL);
        links.push(RenderLink {
            source_x: get_number_field(&source, "x").unwrap_or(0.0),
            source_y: get_number_field(&source, "y").unwrap_or(0.0),
            target_x: get_number_field(&target, "x").unwrap_or(0.0),
            target_y: get_number_field(&target, "y").unwrap_or(0.0),
        });
    }

    let container_width = container
        .dyn_ref::<HtmlElement>()
        .map(|v| v.client_width() as f64)
        .unwrap_or(0.0);
    let container_height = container
        .dyn_ref::<HtmlElement>()
        .map(|v| v.client_height() as f64)
        .unwrap_or(0.0);

    let graph_width = (y_max - y_min) + PADDING_LEFT + PADDING_RIGHT + CARD_WIDTH + 20.0;
    let graph_height = (x_max - x_min) + PADDING_TOP + PADDING_BOTTOM + CARD_HEIGHT;

    Ok(LayoutResult {
        nodes,
        links,
        width: container_width.max(graph_width),
        height: container_height.max(graph_height),
        offset_x: PADDING_LEFT - y_min,
        offset_y: PADDING_TOP - x_min,
    })
}

fn draw_svg(document: &Document, container: &Element, layout: &LayoutResult) -> Result<(), String> {
    let svg = document
        .create_element_ns(Some(SVG_NS_URL), "svg")
        .map_err(|_| "Failed to create svg".to_string())?;
    svg.set_attribute("width", &layout.width.to_string())
        .map_err(|_| "Failed to set attribute.".to_string())?;
    svg.set_attribute("height", &layout.height.to_string())
        .map_err(|_| "Failed to set attribute.".to_string())?;
    svg.set_attribute("style", "display:block;")
        .map_err(|_| "Failed to set attribute.".to_string())?;

    for link in &layout.links {
        let start_x = layout.offset_x + link.source_y + (CARD_WIDTH / 2.0);
        let start_y = layout.offset_y + link.source_x;
        let end_x = layout.offset_x + link.target_y + (CARD_WIDTH / 2.0);
        let end_y = layout.offset_y + link.target_x;
        let control_x = (start_x + end_x) / 2.0;
        let d = format!(
            "M {:.2} {:.2} C {:.2} {:.2}, {:.2} {:.2}, {:.2} {:.2}",
            start_x, start_y, control_x, start_y, control_x, end_y, end_x, end_y
        );

        let path = document
            .create_element_ns(Some(SVG_NS_URL), "path")
            .map_err(|_| "Failed to create link path.".to_string())?;
        path.set_attribute("d", &d)
            .map_err(|_| "Failed to set link path data.".to_string())?;
        path.set_attribute("fill", "none")
            .map_err(|_| "Failed to set link fill.".to_string())?;
        path.set_attribute("stroke", "#aaaaaa")
            .map_err(|_| "Failed to set link stroke.".to_string())?;
        path.set_attribute("stroke-width", "2")
            .map_err(|_| "Failed to set link width.".to_string())?;
        path.set_attribute("stroke-opacity", "0.75")
            .map_err(|_| "Failed to set link opacity.".to_string())?;
        let _ = svg.append_child(&path);
    }

    for node in &layout.nodes {
        let x = layout.offset_x + node.y;
        let y = layout.offset_y + node.x - (CARD_HEIGHT / 2.0);

        let foreign_object = document
            .create_element_ns(Some(SVG_NS_URL), "foreignObject")
            .map_err(|_| "Failed to create node card.".to_string())?;
        foreign_object
            .set_attribute("x", &x.to_string())
            .map_err(|_| "Failed to set node x.".to_string())?;
        foreign_object
            .set_attribute("y", &y.to_string())
            .map_err(|_| "Failed to set node y.".to_string())?;
        foreign_object
            .set_attribute("width", &CARD_WIDTH.to_string())
            .map_err(|_| "Failed to set node width.".to_string())?;
        foreign_object
            .set_attribute("height", &CARD_HEIGHT.to_string())
            .map_err(|_| "Failed to set node height.".to_string())?;
        let card = document
            .create_element_ns(Some(XHTML_NS_URL), "div")
            .map_err(|_| "Failed to create card html.".to_string())?;
        let class_name = (!node.class_name.is_empty())
            .then(|| node.class_name.clone())
            .unwrap_or("-".into());
        let id_attr = (!node.id_attr.is_empty())
            .then(|| node.id_attr.clone())
            .unwrap_or("-".into());

        let html = format!(
            "<div data-state='not-visited' id='{id}' class='graph-node data-[state=visited]:bg-gray-200 data-[state=intermediate]:bg-yellow-400 data-[state=selected]:bg-green-400 data-[state=current]:bg-blue-400 border-box border-2 rounded-lg bg-white p-2' style='width:{w}px;min-height:{h}px;'>\
               <div class='flex justify-between items-center'>\
                 <span>&lt;{tag}&gt;</span>\
               </div>\
               <div class='overflow-scroll'>\
                 <div style='white-space:nowrap;'><b>class</b>: {class_name}</div>\
                 <div style='white-space:nowrap;'><b>id</b>: {id_attr}</div>\
               </div>\
             </div>",
            id = "graph-node-".to_string() + &node.node_index.to_string(),
            w = CARD_WIDTH,
            h = CARD_HEIGHT,
            tag = node.tag,
            class_name = class_name,
            id_attr = id_attr,
        );

        card.set_inner_html(&html);
        let _ = foreign_object.append_child(&card);
        let _ = svg.append_child(&foreign_object);
    }

    let _ = container.append_child(&svg);
    Ok(())
}

fn clear_children(container: &Element) {
    while let Some(child) = container.first_child() {
        let _ = container.remove_child(&child);
    }
}
