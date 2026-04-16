use gloo_net::http::Request;
use gloo::file::callbacks::read_as_data_url;
use gloo::file::File;
use web_sys::{HtmlInputElement, js_sys::futures::spawn_local};
use yew::prelude::*;
use serde::Serialize;

use crate::{GraphContext, html_input::HtmlInput};

#[derive(Serialize)]
struct QueryPostBody {
    input_type: String,
    css_query: String,
    file_payload: String,
    url_payload: String,
    text_payload: String,
    use_dfs: bool,
}

#[component]
pub fn LeftSection() -> Html {
    // Context
    let ctx = use_context::<GraphContext>().unwrap();

    // Toggle Left Section
    let is_open = use_state(|| false);
    let onclick = {
        let is_open = is_open.clone();
        move |_| {
            is_open.set(!(*is_open));
        }
    };

    // Input Type
    let input_type = use_state(|| "file".to_string());

    let onchange = {
        let input_type = input_type.clone();
        Callback::from(move |e: Event| {
            let select: web_sys::HtmlInputElement = e.target_unchecked_into();
            input_type.set(select.value());
        })
    };

    // Input Fields
    let url_input_ref = use_node_ref();
    let text_input_ref = use_node_ref();
    let file_input_ref = use_node_ref();
    
    let file_content = use_state(|| String::new());
    let on_file_change = use_callback(file_content.clone(), move |e: Event, file_content|{
        let el: HtmlInputElement = e.target_unchecked_into();
        if let Some(files) = el.files() {
            if let Some(file) = files.get(0) {
                let file = File::from(file);
                let file_content = file_content.clone();
                read_as_data_url(&file, move |res| {
                    if let Ok(data) = res {
                        file_content.set(data);
                    }
                });
            }
        }
    });
    
    // CSS Query Input
    let css_query = use_state(|| String::new());
    let on_query_change = use_callback(css_query.clone(), move |e: Event, css_query| {
        let el: HtmlInputElement = e.target_unchecked_into();
        css_query.set(el.value());
    });

    // Submit / Cancel
    let has_submitted = use_state(|| false);
    let on_submit_click = {
        let has_submitted = has_submitted.clone();
        let file_content = file_content.clone();
        let file_input_ref = file_input_ref.clone();
        let url_input_ref = url_input_ref.clone();
        let text_input_ref = text_input_ref.clone();
        let input_type = input_type.clone();
        let css_query = css_query.clone();
        let ctx = ctx.clone();

        Callback::from(move |_| {
            let ctx = ctx.clone();

            if *has_submitted {
                if let Some(file_input) = file_input_ref.cast::<HtmlInputElement>() {
                    file_input.set_value("");
                }
                if let Some(url_input) = url_input_ref.cast::<HtmlInputElement>() {
                    url_input.set_value("");
                }
                if let Some(text_input) = text_input_ref.cast::<HtmlInputElement>() {
                    text_input.set_value("");
                }
            } else {
                if let (Some(url_input), Some(text_input)) = 
                        (url_input_ref.cast::<HtmlInputElement>(), 
                        text_input_ref.cast::<HtmlInputElement>()) {
                    let payload = QueryPostBody {
                        input_type: (*input_type).clone(),
                        css_query: (*css_query).clone(),
                        file_payload: (*file_content).clone(),
                        url_payload: url_input.value(),
                        text_payload: text_input.value(),
                        use_dfs: false,
                    };

                    spawn_local(async move {
                        let res = Request::post("http://localhost:8081/query")
                            .json(&payload)
                            .expect("Failed to serialize body")
                            .send()
                            .await
                            .unwrap();
                        if res.ok() {
                            ctx.dispatch(res.text().await.unwrap());
                        }
                    });
                }
            }

            has_submitted.set(!*has_submitted);
        })
    };

    html! {
        <div class="absolute l-0 top-[10vh]">
            <button class="absolute h-[2em] -top-[2em] bg-gray-500 px-4"
                {onclick}>
                {if *is_open {"Close"} else {"Open"} }</button>
            <div
                data-open={(*is_open).to_string()}  
                class="flex flex-col justify-between w-80 h-[80vh] bg-gray-200 data-[open=false]:hidden border border-black rounded-r-xl">
                <div class="flex flex-col gap-4">
                    <HtmlInput {onchange} input_type_str={(*input_type).clone()} {file_input_ref} {url_input_ref} {text_input_ref} {on_file_change} />
                    <div class="w-full flex flex-col gap-2">
                        <label>{"Query"}</label>
                        <input onchange={on_query_change} class="w-full" />
                    </div>
                </div>
                <button onclick={on_submit_click}>{if *has_submitted {"Clear"} else {"Submit"}}</button>
            </div>
        </div>
    }
}