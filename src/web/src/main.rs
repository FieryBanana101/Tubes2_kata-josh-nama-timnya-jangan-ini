use yew::prelude::*;

mod bindings;
mod canvas_tree;
mod html_input;
mod js_util;
mod left_section;
mod right_section;

use crate::canvas_tree::CanvasTree;
use crate::left_section::LeftSection;
use crate::right_section::RightSection;

#[component]
fn App() -> Html {
    html! {
        <div class="relative h-[100vh] w-full">
            <LeftSection/>
            <CanvasTree/>
            <RightSection/>
        </div>
    }
}

pub fn main() {
    yew::Renderer::<App>::new().render();
}
