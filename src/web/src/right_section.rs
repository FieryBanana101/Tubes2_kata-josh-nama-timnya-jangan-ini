use crate::{GraphContext};
use yew::prelude::*;

#[component]
pub fn RightSection() -> Html {
    let ctx = use_context::<GraphContext>().unwrap();
    let selected_node = ctx.selected_node.clone();

    // Toggle Right Section
    let is_open = use_state(|| false);
    let onclick = {
        let is_open = is_open.clone();
        move |_| {
            is_open.set(!(*is_open));
        }
    };

    // Toggle between node details and traversal logs
    let view_mode = use_state(|| "details".to_string());
    let toggle_view = {
        let view_mode: UseStateHandle<String> = view_mode.clone();
        move |_| {
            let new_mode = if *view_mode == "details" { "logs" } else { "details" };
            view_mode.set(new_mode.to_string());
        }
    };

    html! {
       <div class="absolute right-0 top-[10vh]">
            <button class="absolute h-[2em] -top-[2em] bg-gray-500 px-4 right-0"
                {onclick}>
                {if *is_open {"Close"} else {"Open"} }</button>
            <div
                data-open={(*is_open).to_string()}
                class="flex flex-col justify-between w-80 h-[80vh] bg-gray-200 data-[open=false]:hidden border border-black rounded-l-xl overflow-y-scroll overflow-x-hidden">
                <div class="flex flex-col gap-4">
                    <div class="flex flex-row gap-2 justify-center bg-gray-300">
                        <button class="text-white px-4 py-2" onclick={toggle_view}>
                            {if *view_mode == "details" {"Show Logs"} else {"Show Details"} }
                        </button>
                    </div>
                    <div id="node-details" class="p-4 data-[hidden=true]:hidden" data-hidden={(*view_mode != "details").to_string()}>
                        <h2 class="text-xl font-bold mb-2">{"Node Details"}</h2>
                        <p>{"Details will be displayed here."}</p>
                    </div>
                    <div class="p-4 data-[hidden=true]:hidden" data-hidden={(*view_mode != "logs").to_string()}>
                        <h2 class="text-xl font-bold mb-2">{"Traversal Logs"}</h2>
                        <p>{"Logs will be displayed here."}</p>
                    </div>
                </div>
            </div>
        </div>
    }
}
