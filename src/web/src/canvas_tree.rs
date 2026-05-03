use wasm_bindgen::{closure::Closure, JsCast, JsValue};
use web_sys::js_sys::{Array, Reflect, JSON};
use web_sys::{Document, Element, WheelEvent};
use yew::prelude::*;

use crate::bindings::{alert, d3_hierarchy, d3_select, d3_tree, d3_zoom};
use crate::js_util::*;
use crate::GraphContext;

const TREE_CONTAINER_ID: &str = "graph";
const SVG_NS_URL: &str = "http://www.w3.org/2000/svg";
const XHTML_NS_URL: &str = "http://www.w3.org/1999/xhtml";

const CARD_WIDTH: f64 = 140.0;
const CARD_HEIGHT: f64 = 60.0;
const NODE_X_GAP: f64 = 160.0; // Horizontal gap between siblings
const NODE_Y_GAP: f64 = 100.0; // Vertical gap between levels
const PADDING_TOP: f64 = 50.0;
const PADDING_LEFT: f64 = 50.0;
const PADDING_RIGHT: f64 = 50.0;

#[derive(Clone)]
struct RenderNode {
    x: f64,
    y: f64,
    tag: String,
    class_name: String,
    id_attr: String,
    node_index: i32,
    attributes: Vec<(String, String)>,
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

    let onwheel = Callback::from(|e: WheelEvent| {
        e.prevent_default();
    });

    html! {
        <div
            id={TREE_CONTAINER_ID}
            class="h-screen w-screen w-full overflow-hidden"
            onwheel={onwheel}
        ></div>
    }
}

fn render_tree(raw_json: &str) {
    web_sys::console::log_1(&JsValue::from_str(&format!("Raw JSON: {}", raw_json)));
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
        "{\"root_index\": -1, \"nodes\": {}}"
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

    if let Err(msg) = setup_d3_zoom(&document, &container, layout.width, layout.height) {
        web_sys::console::error_1(&JsValue::from_str(&msg));
    }
}

struct LayoutResult {
    nodes: Vec<RenderNode>,
    links: Vec<RenderLink>,
    width: f64,
    height: f64,
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
        });
    }

    let nodes_value = Reflect::get(parsed, &JsValue::from_str("nodes"))
        .map_err(|_| "Missing nodes object.".to_string())?;

    let root_node_data = Reflect::get(&nodes_value, &JsValue::from_str(&root_index.to_string()))
        .map_err(|_| format!("Root node with index {} not found.", root_index))?;

    let nodes_for_children = nodes_value.clone();
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
                let child = Reflect::get(&nodes_for_children, &JsValue::from_str(&node_index.to_string()))
                    .unwrap_or(JsValue::UNDEFINED);
                if !child.is_undefined() && !child.is_null() {
                    resolved_children.push(&child);
                }
            }
        }
        resolved_children.into()
    });

    let hierarchy_root = d3_hierarchy(
        &root_node_data,
        children_accessor
            .as_ref()
            .unchecked_ref::<web_sys::js_sys::Function>(),
    );

    let tree_layout = d3_tree();
    // In vertical layout, we swap the gaps to d3.tree()
    let node_size = Array::new();
    node_size.push(&JsValue::from_f64(NODE_X_GAP));
    node_size.push(&JsValue::from_f64(NODE_Y_GAP));
    let _ = call_method1(&tree_layout, "nodeSize", &node_size.into())?;

    let tree_function = tree_layout
        .dyn_ref::<web_sys::js_sys::Function>()
        .ok_or_else(|| "d3.tree() did not return a callable".to_string())?;
    let _ = tree_function.call1(&JsValue::NULL, &hierarchy_root);

    let descendants_value = call_method0(&hierarchy_root, "descendants")?;
    let descendants = Array::from(&descendants_value);
    
    let links_value = call_method0(&hierarchy_root, "links")?;
    let links_array = Array::from(&links_value);

    let mut nodes: Vec<RenderNode> = Vec::new();
    let mut x_min = f64::INFINITY;
    let mut x_max = f64::NEG_INFINITY;
    let mut y_min = f64::INFINITY;
    let mut y_max = f64::NEG_INFINITY;

    for i in 0..descendants.length() {
        let item = descendants.get(i);
        // Vertical tree: d3-tree 'x' is horizontal position, 'y' is vertical depth
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

        let attrs_value = Reflect::get(&data, &JsValue::from_str("attributes")).unwrap_or(JsValue::NULL);
        let attrs_array = Array::from(&attrs_value);
        let mut attributes: Vec<(String, String)> = Vec::new();
        
        // Handle Map-like attributes from new JSON
        if attrs_value.is_object() {
            let keys = Reflect::own_keys(&attrs_value).unwrap_or(Array::new());
            for j in 0..keys.length() {
                let key = keys.get(j).as_string().unwrap_or_default();
                let val = Reflect::get(&attrs_value, &JsValue::from_str(&key))
                    .ok()
                    .and_then(|v| v.as_string())
                    .unwrap_or_default();
                if !key.is_empty() {
                    attributes.push((key, val));
                }
            }
        }

        nodes.push(RenderNode { x, y, tag, class_name, id_attr, node_index, attributes });
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

    let container_width = container.dyn_ref::<web_sys::HtmlElement>().map(|v| v.client_width() as f64).unwrap_or(0.0);
    let container_height = container.dyn_ref::<web_sys::HtmlElement>().map(|v| v.client_height() as f64).unwrap_or(0.0);

    let graph_width = (x_max - x_min) + PADDING_LEFT + PADDING_RIGHT + CARD_WIDTH;
    let graph_height = (y_max - y_min) + PADDING_TOP + 150.0 + CARD_HEIGHT;

    Ok(LayoutResult {
        nodes,
        links,
        width: container_width.max(graph_width),
        height: container_height.max(graph_height),
    })
}

fn draw_svg(document: &Document, container: &Element, layout: &LayoutResult) -> Result<(), String> {
    let svg = document.create_element_ns(Some(SVG_NS_URL), "svg").map_err(|_| "Failed to create svg".to_string())?;
    svg.set_attribute("width", &layout.width.to_string()).map_err(|_| "Failed to set width.".to_string())?;
    svg.set_attribute("height", &layout.height.to_string()).map_err(|_| "Failed to set height.".to_string())?;
    svg.set_attribute("id", "graph-svg").map_err(|_| "Failed to set svg id.".to_string())?;

    let g = document.create_element_ns(Some(SVG_NS_URL), "g").map_err(|_| "Failed to create group".to_string())?;
    g.set_attribute("class", "zoom-group").map_err(|_| "Failed to set group class".to_string())?;
    let _ = svg.append_child(&g);

    // Normalize X so the leftmost node is at PADDING_LEFT
    let mut min_x = f64::INFINITY;
    for node in &layout.nodes { min_x = min_x.min(node.x); }
    let x_offset = PADDING_LEFT - min_x;

    for link in &layout.links {
        let start_x = link.source_x + x_offset + (CARD_WIDTH / 2.0);
        let start_y = link.source_y + PADDING_TOP + CARD_HEIGHT;
        let end_x = link.target_x + x_offset + (CARD_WIDTH / 2.0);
        let end_y = link.target_y + PADDING_TOP;
        
        let control_y = (start_y + end_y) / 2.0;
        let d = format!(
            "M {:.2} {:.2} C {:.2} {:.2}, {:.2} {:.2}, {:.2} {:.2}",
            start_x, start_y, start_x, control_y, end_x, control_y, end_x, end_y
        );

        let path = document.create_element_ns(Some(SVG_NS_URL), "path").map_err(|_| "Failed to create link path.".to_string())?;
        path.set_attribute("d", &d).map_err(|_| "Failed to set link path data.".to_string())?;
        path.set_attribute("fill", "none").map_err(|_| "Failed to set link fill.".to_string())?;
        path.set_attribute("stroke", "#aaaaaa").map_err(|_| "Failed to set link stroke.".to_string())?;
        path.set_attribute("stroke-width", "2").map_err(|_| "Failed to set link width.".to_string())?;
        let _ = g.append_child(&path);
    }

    for node in &layout.nodes {
        let x = node.x + x_offset;
        let y = node.y + PADDING_TOP;

        let foreign_object = document.create_element_ns(Some(SVG_NS_URL), "foreignObject").map_err(|_| "Failed to create node card.".to_string())?;
        foreign_object.set_attribute("x", &x.to_string()).map_err(|_| "Failed to set node x.".to_string())?;
        foreign_object.set_attribute("y", &y.to_string()).map_err(|_| "Failed to set node y.".to_string())?;
        foreign_object.set_attribute("width", &CARD_WIDTH.to_string()).map_err(|_| "Failed to set node width.".to_string())?;
        foreign_object.set_attribute("height", &CARD_HEIGHT.to_string()).map_err(|_| "Failed to set node height.".to_string())?;
        
        let card = document.create_element_ns(Some(XHTML_NS_URL), "div").map_err(|_| "Failed to create card html.".to_string())?;
        let node_id = format!("graph-node-{}", node.node_index);

        let id_html = if !node.id_attr.is_empty() && node.id_attr != "-" {
            format!("<div class='text-[10px] leading-tight truncate'><b>id:</b> {}</div>", node.id_attr)
        } else {
            String::new()
        };

        let class_html = if !node.class_name.is_empty() && node.class_name != "-" {
            format!("<div class='text-[10px] leading-tight truncate'><b>class:</b> {}</div>", node.class_name)
        } else {
            String::new()
        };

        let html = format!(
            "<div id='{node_id}' class='graph-node border-2 rounded bg-white p-1.5 cursor-pointer hover:bg-blue-50 \
                  data-[state=selected]:bg-green-400 data-[state=selected]:border-green-600 \
                  data-[state=visited]:bg-yellow-200 data-[state=visited]:border-yellow-400 \
                  data-[state=current]:bg-blue-400 data-[state=current]:border-blue-600 \
                  data-[state=lca]:border-red-500' \
                  style='width:{w}px;height:{h}px;display:flex;flex-direction:column;justify-content:center;text-align:center;' \
                  onclick='window.selectNode({idx})' \
                  oncontextmenu='event.preventDefault(); window.setLCATarget({idx})'>\
               <div class='font-bold text-[13px] text-blue-900 data-[state=selected]:text-green-900 mb-0.5'>&lt;{tag}&gt;</div>\
               {id_html}\
               {class_html}\
             </div>",
            node_id = node_id,
            idx = node.node_index,
            tag = node.tag,
            w = CARD_WIDTH,
            h = CARD_HEIGHT,
            id_html = id_html,
            class_html = class_html,
        );

        card.set_inner_html(&html);
        let _ = foreign_object.append_child(&card);
        let _ = g.append_child(&foreign_object);
    }

    let _ = container.append_child(&svg);
    Ok(())
}

fn setup_d3_zoom(document: &Document, _container: &Element, width: f64, height: f64) -> Result<(), String> {
    let Some(window) = web_sys::window() else { return Err("No window".to_string()); };
    let svg_element = document.get_element_by_id("graph-svg").ok_or("SVG not found")?;
    let zoom_obj = d3_zoom();

    let scale_extent = Array::new();
    scale_extent.push(&JsValue::from_f64(0.1));
    scale_extent.push(&JsValue::from_f64(3.0));
    let zoom_obj = call_method1(&zoom_obj, "scaleExtent", &scale_extent.into()).map_err(|_| "Zoom error".to_string())?;

    let zoomed = Reflect::get(&window, &JsValue::from_str("zoomed")).map_err(|_| "zoomed missing".to_string())?;
    let zoom_obj = call_method2(&zoom_obj, "on", &JsValue::from_str("zoom"), &zoomed).map_err(|_| "Zoom on error".to_string())?;

    let selection = d3_select(&svg_element);
    let _ = call_method1(&selection, "call", &zoom_obj.into()).map_err(|_| "Zoom apply error".to_string())?;

    Ok(())
}

fn clear_children(container: &Element) {
    while let Some(child) = container.first_child() {
        let _ = container.remove_child(&child);
    }
}
