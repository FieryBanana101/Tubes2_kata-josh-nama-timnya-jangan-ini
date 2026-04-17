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

#[derive(Debug, Eq, Clone, PartialEq)]
pub struct Graph {
    pub graph_data: String,
}

impl Reducible for Graph {
    type Action = String;

    fn reduce(self: Rc<Self>, action: Self::Action) -> Rc<Self> {
        Graph { graph_data: action }.into()
    }
}

pub type GraphContext = UseReducerHandle<Graph>;

#[component]
fn App() -> Html {
    let graph = use_reducer(|| Graph { graph_data: String::new() });

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
