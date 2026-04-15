use yew::prelude::*;

use crate::web::canvas_tree::CanvasTree;
use crate::web::left_section::LeftSection;
use crate::web::right_section::RightSection;

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

pub fn render() {
    yew::Renderer::<App>::new().render();
}
