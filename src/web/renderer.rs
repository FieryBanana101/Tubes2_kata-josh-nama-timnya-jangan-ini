use yew::prelude::*;

use crate::web::left_section::LeftSection;
use crate::web::right_section::RightSection;

#[component]
fn App() -> Html {
    html! {
        <div class="w-full h-[100vh]">
            <LeftSection/>
            <canvas/>
            <RightSection/>
        </div>
    }
}

pub fn render() {
    yew::Renderer::<App>::new().render();
}
