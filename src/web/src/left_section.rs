use gloo::file::File;
use gloo::file::callbacks::read_as_data_url;
use gloo_net::http::Request;
use serde::Serialize;
use serde::Deserialize;
use web_sys::{HtmlInputElement, js_sys::futures::spawn_local};
use yew::prelude::*;

use crate::{GraphContext, GraphAction, html_input::HtmlInput, result_player::{ResultPlayer, ResultItem}};

#[derive(Serialize)]
struct QueryPostBody {
    input_type: String,
    css_query: String,
    file_payload: String,
    url_payload: String,
    text_payload: String,
    use_dfs: bool,
}

#[derive(Deserialize)]
struct GraphJson {
    #[allow(dead_code)]
    root_index: i32,
    #[allow(dead_code)]
    nodes: Vec<serde_json::Value>,
    results: Vec< serde_json::Value>,
    #[allow(dead_code)]
    selected_nodes: Vec<i32>,
}

fn parse_result_data(graph_json: &str) -> Vec<ResultItem> {
    if graph_json.trim().is_empty() || graph_json == "{}" {
        return Vec::new();
    }

    match serde_json::from_str::<GraphJson>(graph_json) {
        Ok(data) => {
            data.results
                .iter()
                .filter_map(|r| {
                    let query = r.get("query")?.as_str()?.to_string();
                    let paths: Vec<Vec<u32>> = r
                        .get("paths")?
                        .as_array()?
                        .iter()
                        .filter_map(|p| {
                            Some(p.as_array()?.iter().filter_map(|v| v.as_i64().map(|n| n as u32)).collect())
                        })
                        .collect();
                    let selected: Vec<u32> = r
                        .get("selected")?
                        .as_array()?
                        .iter()
                        .filter_map(|v| v.as_i64().map(|n| n as u32))
                        .collect();
                    let traversal_path: Vec<u32> = r
                        .get("traversal_path")?
                        .as_array()?
                        .iter()
                        .filter_map(|v| v.as_i64().map(|n| n as u32))
                        .collect();

                    Some(ResultItem {
                        query,
                        paths,
                        selected,
                        traversal_path,
                    })
                })
                .collect()
        }
        Err(_) => Vec::new(),
    }
}

#[component]
pub fn LeftSection() -> Html {
    // Context
    let ctx = use_context::<GraphContext>().unwrap();

    let result_data = use_memo(ctx.graph_data.clone(), |json| {
        parse_result_data(json)
    });

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
    let on_file_change = use_callback(file_content.clone(), move |e: Event, file_content| {
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

    // BFS / DFS Toggle
    let use_dfs = use_state(|| false);
    let on_dfs_toggle = {
        let use_dfs = use_dfs.clone();
        Callback::from(move |_| {
            use_dfs.set(!*use_dfs);
        })
    };

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
                ctx.dispatch(GraphAction::SetGraphData("".to_string()));
            } else {
                if let (Some(url_input), Some(text_input)) = (
                    url_input_ref.cast::<HtmlInputElement>(),
                    text_input_ref.cast::<HtmlInputElement>(),
                ) {
                    let payload = QueryPostBody {
                        input_type: (*input_type).clone(),
                        css_query: (*css_query).clone(),
                        file_payload: (*file_content).clone(),
                        url_payload: url_input.value(),
                        text_payload: text_input.value(),
                        use_dfs: *use_dfs,
                    };

                    spawn_local(async move {
                        let res = Request::post("http://localhost:8081/query")
                            .json(&payload)
                            .expect("Failed to serialize body")
                            .send()
                            .await
                            .unwrap();
                        if res.ok() {
                            let response_text = res.text().await.unwrap();
                            ctx.dispatch(GraphAction::SetGraphData(response_text));
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
                class="flex flex-col justify-between w-80 h-[80vh] bg-gray-200 data-[open=false]:hidden border border-black rounded-r-xl overflow-y-scroll overflow-x-hidden">
                <div class="flex flex-col gap-4">
                    <HtmlInput {onchange} input_type_str={(*input_type).clone()} {file_input_ref} {url_input_ref} {text_input_ref} {on_file_change} />
                    <div class="w-full flex flex-col gap-2">
                        <label>{"Query"}</label>
                        <input onchange={on_query_change} class="w-full" />
                    </div>
                    <div class="w-full flex items-center justify-between gap-2">
                        <label>{"Use DFS"}</label>
                        <input type="checkbox" onchange={on_dfs_toggle} class="w-4 h-4" />
                    </div>
                    <ResultPlayer is_open={true} result_data={(*result_data).clone()}  />
                    <div class="border-t border-black flex flex-col gap-4">
                        <p>{"Total Affected Nodes: 9"}</p>
                        <p>{"Total Matched Nodes: 10"}</p>
                        <div class="flex flex-col gap-2">
                            <p>{6}</p>
                            <p>{7}</p>
                            <p>{8}</p>
                        </div>
                    </div>
                </div>
                <button onclick={on_submit_click}>{if *has_submitted {"Clear"} else {"Submit"}}</button>
            </div>
        </div>
    }
}
