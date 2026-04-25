use yew::prelude::*;

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

#[derive(Debug, Clone, PartialEq)]
pub struct Graph {
    pub graph_data: String,
    pub selected_node: Option<SelectedNode>,
}

impl Reducible for Graph {
    type Action = GraphAction;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        match action {
            GraphAction::SetGraphData(data) => Graph { graph_data: data, selected_node: self.selected_node.clone() }.into(),
            GraphAction::SelectNode(node) => Graph { graph_data: self.graph_data.clone(), selected_node: Some(node) }.into(),
            GraphAction::ClearSelection => Graph { graph_data: self.graph_data.clone(), selected_node: None }.into(),
        }
    }
}

pub enum GraphAction {
    SetGraphData(String),
    SelectNode(SelectedNode),
    ClearSelection,
}

pub type GraphContext = UseReducerHandle<Graph>;

#[component]
fn App() -> Html {
    let graph = use_reducer(|| Graph { graph_data: String::new(), selected_node: None });

    html! {
        <ContextProvider<GraphContext> context={graph}>
            <div class="relative h-[100vh] w-full">
                <LeftSection/>
                <CanvasTree />
                <RightSection/>
            </div>
        </ContextProvider<GraphContext>>
    }
}

pub fn main() {
    yew::Renderer::<App>::new().render();
}