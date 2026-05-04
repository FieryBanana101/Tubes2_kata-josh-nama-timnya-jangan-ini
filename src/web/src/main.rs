use yew::prelude::*;
use serde_json;
use wasm_bindgen::prelude::*;

mod bindings;
mod canvas_tree;
mod html_input;
mod js_util;
mod left_section;
mod right_section;
mod result_player;

use std::rc::Rc;

use crate::canvas_tree::CanvasTree;
use crate::left_section::LeftSection;
use crate::right_section::RightSection;

#[derive(Debug, Clone, PartialEq)]
pub struct SelectedNode {
    pub node_index: i32,
    pub tag: String,
    pub class: String,
    pub id: String,
    pub attributes: Vec<(String, String)>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AnimationType {
    None,
    CSS,
    LCA,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DetailNode {
    pub x: f64,
    pub y: f64,
    pub node_index: i32,
    pub tag: String,
    pub class: String,
    pub id: String,
    pub attributes: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    pub graph_data: String,
    pub selected_node: Option<SelectedNode>,
    pub lca_selected: Vec<usize>,
    pub animation_type: AnimationType,
    pub animation_active: bool,
    pub detail_node: Option<DetailNode>,
}

impl Reducible for Graph {
    type Action = GraphAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match action {
            GraphAction::SetGraphData(data) => Graph { 
                graph_data: data, 
                selected_node: self.selected_node.clone(),
                lca_selected: Vec::new(),
                animation_type: AnimationType::None,
                animation_active: false,
                detail_node: None,
            }.into(),
            GraphAction::SelectNode(node) => Graph { 
                graph_data: self.graph_data.clone(), 
                selected_node: Some(node),
                lca_selected: self.lca_selected.clone(),
                animation_type: self.animation_type,
                animation_active: self.animation_active,
                detail_node: self.detail_node.clone(),
            }.into(),
            GraphAction::ClearSelection => Graph { 
                graph_data: self.graph_data.clone(), 
                selected_node: None,
                lca_selected: self.lca_selected.clone(),
                animation_type: self.animation_type,
                animation_active: self.animation_active,
                detail_node: self.detail_node.clone(),
            }.into(),
            GraphAction::ToggleLCANode(idx) => {
                if self.animation_active {
                    return self;
                }
                let mut new_lca = self.lca_selected.clone();
                if let Some(pos) = new_lca.iter().position(|&x| x == idx) {
                    new_lca.remove(pos);
                } else {
                    new_lca.push(idx);
                }
                Graph {
                    graph_data: self.graph_data.clone(),
                    selected_node: self.selected_node.clone(),
                    lca_selected: new_lca,
                    animation_type: self.animation_type,
                    animation_active: self.animation_active,
                    detail_node: self.detail_node.clone(),
                }.into()
            },
            GraphAction::ClearLCA => Graph {
                graph_data: self.graph_data.clone(),
                selected_node: self.selected_node.clone(),
                lca_selected: Vec::new(),
                animation_type: self.animation_type,
                animation_active: self.animation_active,
                detail_node: self.detail_node.clone(),
            }.into(),
            GraphAction::SetAnimationType(anim_type) => Graph {
                graph_data: self.graph_data.clone(),
                selected_node: self.selected_node.clone(),
                lca_selected: self.lca_selected.clone(),
                animation_type: anim_type,
                animation_active: false,
                detail_node: self.detail_node.clone(),
            }.into(),
            GraphAction::SetAnimationActive(active) => Graph {
                graph_data: self.graph_data.clone(),
                selected_node: self.selected_node.clone(),
                lca_selected: self.lca_selected.clone(),
                animation_type: self.animation_type,
                animation_active: active,
                detail_node: self.detail_node.clone(),
            }.into(),
            GraphAction::ShowDetail(detail) => Graph {
                graph_data: self.graph_data.clone(),
                selected_node: self.selected_node.clone(),
                lca_selected: self.lca_selected.clone(),
                animation_type: self.animation_type,
                animation_active: self.animation_active,
                detail_node: Some(detail),
            }.into(),
            GraphAction::HideDetail => Graph {
                graph_data: self.graph_data.clone(),
                selected_node: self.selected_node.clone(),
                lca_selected: self.lca_selected.clone(),
                animation_type: self.animation_type,
                animation_active: self.animation_active,
                detail_node: None,
            }.into(),
        }
    }
}

pub enum GraphAction {
    SetGraphData(String),
    SelectNode(SelectedNode),
    ClearSelection,
    ToggleLCANode(usize),
    ClearLCA,
    SetAnimationType(AnimationType),
    SetAnimationActive(bool),
    ShowDetail(DetailNode),
    HideDetail,
}

pub type GraphContext = UseReducerHandle<Graph>;

#[component]
fn App() -> Html {
    let graph = use_reducer(|| Graph { 
        graph_data: String::new(), 
        selected_node: None,
        lca_selected: Vec::new(),
        animation_type: AnimationType::None,
        animation_active: false,
        detail_node: None,
    });

    // Expose window.selectNode to JS
    {
        let graph_handle = graph.clone();
        use_effect_with(graph.animation_active, move |&anim_active| {
            let inner_graph = graph_handle.clone();
            let closure = Closure::<dyn Fn(usize)>::new(move |idx: usize| {
                if !anim_active {
                    inner_graph.dispatch(GraphAction::ToggleLCANode(idx));
                }
            });

            let window = web_sys::window().unwrap();
            let _ = web_sys::js_sys::Reflect::set(
                &window,
                &JsValue::from_str("selectNode"),
                closure.as_ref().unchecked_ref(),
            );

            closure.forget();
            || ()
        });
    }

    let on_close_modal = {
        let graph = graph.clone();
        Callback::from(move |_| graph.dispatch(GraphAction::HideDetail))
    };

    html! {
        <ContextProvider<GraphContext> context={graph.clone()}>
            <div class="relative h-[100vh] w-full">
                <LeftSection/>
                <CanvasTree />
                <RightSection/>
                { if let Some(node) = &graph.detail_node {
                    let attrs_str = node.attributes.iter()
                        .map(|(k, v)| format!("{}=\"{}\"", k, v))
                        .collect::<Vec<_>>()
                        .join(" ");
                    let tag_html = format!("<{} {}>", node.tag, attrs_str);

                    html! {
                        <div class="fixed inset-0 z-[100] flex items-center justify-center bg-black bg-opacity-50">
                            <div class="bg-white p-6 rounded shadow-xl w-96 relative">
                                <button onclick={on_close_modal.clone()} class="absolute top-2 right-2 text-gray-500 hover:text-black text-xl font-bold">{"×"}</button>
                                <h2 class="text-lg font-bold mb-4">{"Node Details"}</h2>
                                <div class="space-y-2 text-sm font-mono text-gray-700">
                                    <p><strong>{"Node Index: "}</strong>{node.node_index}</p>
                                    <p><strong>{"Position (x,y): "}</strong>{format!("({}, {})", node.x, node.y)}</p>
                                    <div class="mt-4 p-2 bg-gray-100 rounded break-words whitespace-pre-wrap">{tag_html}</div>
                                </div>
                            </div>
                        </div>
                    }
                } else { html! {} } }
            </div>
        </ContextProvider<GraphContext>>
    }
}

pub fn main() {
    yew::Renderer::<App>::new().render();
}
