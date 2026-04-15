use yew::prelude::*;

#[component]
pub fn RightSection() -> Html {
    let is_open = use_state(|| false);
    let onclick = {
        let is_open = is_open.clone();
        move |_| {
            is_open.set(!(*is_open));
        }
    };

    html! {
        <div class="absolute right-0 top-[10vh] ">
            <button class="absolute h-[2em] -top-[2em] bg-gray-500 px-4 right-0"
                {onclick}>
                {if *is_open {"Close"} else {"Open"} }</button>
            <div
                data-open={(*is_open).to_string()}  
                class="w-80 h-[80vh] bg-gray-200 data-[open=false]:hidden border border-black rounded-l-xl">
            </div>
        </div>
    }
}