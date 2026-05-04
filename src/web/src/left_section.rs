use gloo::file::File;
use gloo::file::callbacks::read_as_text;
use gloo_net::http::Request;
use serde::{Serialize, Deserialize};
use web_sys::{HtmlInputElement, js_sys::futures::spawn_local};
use yew::prelude::*;

use crate::{GraphContext, GraphAction, html_input::HtmlInput, result_player::{ResultItem, ResultPlayer}, bindings::alert};

#[derive(Serialize)]
struct QueryPostBody {
    query_type: String,
    input_type: String,
    content: String,
    css_query: String,
    use_dfs: bool,
    threads: Option<usize>,
    search_limit: Option<usize>,
}

#[derive(Deserialize)]
struct HtmlQueryResponse {
    nodes_count: usize,
    duration: u128,
}

#[derive(Deserialize)]
struct CSSQueryResponse {
    results: Vec<ResultItem>,
}

#[derive(Deserialize)]
struct LCAQueryResponse {
    results: Vec<ResultItem>,
}

#[component]
pub fn LeftSection() -> Html {
    let ctx = use_context::<GraphContext>().unwrap();
    
    let is_open = use_state(|| false);
    let html_submitted = use_state(|| false);
    
    // HTML Metrics
    let html_nodes_count = use_state(|| 0usize);
    let html_duration = use_state(|| 0u128);

    // HTML Input State
    let input_type = use_state(|| "file".to_string());
    let file_content = use_state(|| String::new());
    let file_reader = use_state(|| None::<gloo::file::callbacks::FileReader>);
    let url_input_ref = use_node_ref();
    let text_input_ref = use_node_ref();
    let file_input_ref = use_node_ref();

    // CSS Query State
    let css_query = use_state(|| String::new());
    let use_dfs = use_state(|| true); // Default to DFS
    let threads = use_state(|| 1usize);
    let search_limit = use_state(|| 1usize);
    let limit_enabled = use_state(|| false);
    let result_data = use_state(|| Vec::<ResultItem>::new());

    // LCA Query State
    let lca_result_data = use_state(|| Vec::<ResultItem>::new());

    let on_file_change = {
        let file_content = file_content.clone();
        let file_reader = file_reader.clone();
        Callback::from(move |e: Event| {
            let el: HtmlInputElement = e.target_unchecked_into();
            if let Some(files) = el.files() {
                if let Some(file) = files.get(0) {
                    let file = File::from(file);
                    let file_content = file_content.clone();
                    let reader = read_as_text(&file, move |res| {
                        match res {
                            Ok(text) => file_content.set(text),
                            Err(e) => alert(&format!("Error reading file: {}", e)),
                        }
                    });
                    file_reader.set(Some(reader));
                }
            }
        })
    };

    let on_submit_html = {
        let ctx = ctx.clone();
        let input_type = input_type.clone();
        let file_content = file_content.clone();
        let url_input_ref = url_input_ref.clone();
        let text_input_ref = text_input_ref.clone();
        let html_submitted = html_submitted.clone();
        let html_nodes_count = html_nodes_count.clone();
        let html_duration = html_duration.clone();

        Callback::from(move |_| {
            let content = match (*input_type).as_str() {
                "file" => (*file_content).clone(),
                "url" => url_input_ref.cast::<HtmlInputElement>().map(|i| i.value()).unwrap_or_default(),
                "plain_text" => text_input_ref.cast::<HtmlInputElement>().map(|i| i.value()).unwrap_or_default(),
                _ => String::new(),
            };
            
            if content.trim().is_empty() {
                alert("HTML content is empty");
                return;
            }

            let payload = QueryPostBody {
                query_type: "html".to_string(),
                input_type: (*input_type).clone(),
                content,
                css_query: String::new(),
                use_dfs: false,
                threads: None,
                search_limit: None,
            };

            let ctx = ctx.clone();
            let html_submitted = html_submitted.clone();
            let html_nodes_count = html_nodes_count.clone();
            let html_duration = html_duration.clone();
            spawn_local(async move {
                let res = Request::post("http://127.0.0.1:8081/query")
                    .json(&payload)
                    .expect("Failed to serialize body")
                    .send()
                    .await;
                
                match res {
                    Ok(res) => {
                        if res.ok() {
                            let response_text = res.text().await.unwrap();
                            if let Ok(parsed) = serde_json::from_str::<HtmlQueryResponse>(&response_text) {
                                html_nodes_count.set(parsed.nodes_count);
                                html_duration.set(parsed.duration);
                            }
                            ctx.dispatch(GraphAction::SetGraphData(response_text));
                            html_submitted.set(true);
                        } else {
                            let msg = res.text().await.unwrap_or_else(|_| res.status_text());
                            alert(&format!("Server Error: {}", msg));
                        }
                    }
                    Err(e) => alert(&format!("Network Error: {}", e)),
                }
            });
        })
    };

    let on_submit_css = {
        let ctx = ctx.clone();
        let css_query = css_query.clone();
        let use_dfs = use_dfs.clone();
        let threads = threads.clone();
        let search_limit = search_limit.clone();
        let limit_enabled = limit_enabled.clone();
        let result_data = result_data.clone();

        Callback::from(move |_| {
            if (*css_query).trim().is_empty() {
                alert("CSS query is empty");
                return;
            }

            let payload = QueryPostBody {
                query_type: "css".to_string(),
                input_type: String::new(),
                content: String::new(),
                css_query: (*css_query).clone(),
                use_dfs: *use_dfs,
                threads: Some(*threads),
                search_limit: Some(if *limit_enabled { *search_limit } else { 0 }),
            };

            let result_data = result_data.clone();
            spawn_local(async move {
                let res = Request::post("http://127.0.0.1:8081/query")
                    .json(&payload)
                    .expect("Failed to serialize body")
                    .send()
                    .await;
                
                match res {
                    Ok(res) => {
                        if res.ok() {
                            match res.json::<CSSQueryResponse>().await {
                                Ok(response) => result_data.set(response.results),
                                Err(e) => alert(&format!("Failed to parse response: {}", e)),
                            }
                        } else {
                            let msg = res.text().await.unwrap_or_else(|_| res.status_text());
                            alert(&format!("CSS Query Failed: {}", msg));
                        }
                    }
                    Err(e) => alert(&format!("Network Error: {}", e)),
                }
            });
        })
    };

    let on_submit_lca = {
        let ctx = ctx.clone();
        let lca_result_data = lca_result_data.clone();
        Callback::from(move |_| {
            let lca_nodes = ctx.lca_selected.clone();
            
            if lca_nodes.is_empty() {
                alert("No nodes choosen for LCA operation");
                return;
            }

            let content = serde_json::to_string(&lca_nodes).unwrap_or_else(|_| "[]".to_string());
            
            let payload = QueryPostBody {
                query_type: "lca".to_string(),
                input_type: String::new(),
                content,
                css_query: String::new(),
                use_dfs: false,
                threads: None,
                search_limit: None,
            };

            let lca_result_data = lca_result_data.clone();
            spawn_local(async move {
                let res = Request::post("http://127.0.0.1:8081/query")
                    .json(&payload)
                    .expect("Failed to serialize body")
                    .send()
                    .await;
                
                match res {
                    Ok(res) => {
                        if res.ok() {
                            match res.json::<LCAQueryResponse>().await {
                                Ok(response) => lca_result_data.set(response.results),
                                Err(e) => alert(&format!("Failed to parse response: {}", e)),
                            }
                        } else {
                            let msg = res.text().await.unwrap_or_else(|_| res.status_text());
                            alert(&format!("LCA Query Failed: {}", msg));
                        }
                    }
                    Err(e) => alert(&format!("Network Error: {}", e)),
                }
            });
        })
    };

    let on_input_type_change = {
        let input_type = input_type.clone();
        Callback::from(move |e: Event| {
            let select: HtmlInputElement = e.target_unchecked_into();
            input_type.set(select.value());
        })
    };

    let on_css_query_change = {
        let css_query = css_query.clone();
        Callback::from(move |e: InputEvent| {
            let el: HtmlInputElement = e.target_unchecked_into();
            css_query.set(el.value());
        })
    };

    let on_threads_change = {
        let threads = threads.clone();
        Callback::from(move |e: InputEvent| {
            let el: HtmlInputElement = e.target_unchecked_into();
            if let Ok(val) = el.value().parse::<usize>() {
                threads.set(val);
            }
        })
    };

    let on_search_limit_change = {
        let search_limit = search_limit.clone();
        Callback::from(move |e: InputEvent| {
            let el: HtmlInputElement = e.target_unchecked_into();
            if let Ok(val) = el.value().parse::<usize>() {
                search_limit.set(val);
            }
        })
    };

    let on_traversal_mode_change = {
        let use_dfs = use_dfs.clone();
        Callback::from(move |e: Event| {
            let select: web_sys::HtmlInputElement = e.target_unchecked_into();
            use_dfs.set(select.value() == "DFS");
        })
    };

    let on_limit_enabled_change = {
        let limit_enabled = limit_enabled.clone();
        Callback::from(move |e: Event| {
            let checkbox: web_sys::HtmlInputElement = e.target_unchecked_into();
            limit_enabled.set(checkbox.checked());
        })
    };

    let toggle_open = {
        let is_open = is_open.clone();
        move |_| is_open.set(!(*is_open))
    };

    html! {
        <div class="absolute left-0 top-0 z-50">
            <button class="absolute h-[2em] left-0 top-0 bg-gray-500 px-4 text-white text-xs font-bold rounded-br-lg active:scale-95 transition-all shadow-md z-[60]" onclick={toggle_open}>
                {if *is_open {"Close"} else {"Open"} }
            </button>
            <div data-open={(*is_open).to_string()}
                 class="flex flex-col gap-6 w-96 h-screen bg-gray-200 data-[open=false]:hidden border-r border-black overflow-y-scroll p-6 pt-12 shadow-2xl transition-all">
                
                <section class="flex flex-col gap-4 border-b border-gray-400 pb-6">
                    <h3 class="font-bold">{"Submit HTML Section"}</h3>
                    <HtmlInput 
                        onchange={on_input_type_change} 
                        input_type_str={(*input_type).clone()} 
                        {file_input_ref} 
                        {url_input_ref} 
                        {text_input_ref} 
                        {on_file_change} 
                    />
                    <button class="bg-blue-600 text-white py-2 rounded font-medium hover:bg-blue-700 active:scale-95 transition-all shadow-sm active:shadow-inner" onclick={on_submit_html}>{"submit html button"}</button>
                    
                    {if *html_submitted {
                        let duration_text = if *html_duration < 1000 {
                            format!("{} µs", *html_duration)
                        } else if *html_duration < 1_000_000 {
                            format!("{:.2} ms", *html_duration as f64 / 1000.0)
                        } else {
                            format!("{:.2} s", *html_duration as f64 / 1_000_000.0)
                        };
                        html! {
                            <div class="text-[10px] font-mono text-gray-600 space-y-0.5 mt-1">
                                <div class="flex justify-between">
                                    <span>{"Total Nodes:"}</span>
                                    <span>{*html_nodes_count}</span>
                                </div>
                                <div class="flex justify-between">
                                    <span>{"Parsing Duration:"}</span>
                                    <span>{duration_text}</span>
                                </div>
                            </div>
                        }
                    } else { html! {} }}
                </section>

                {if *html_submitted {
                    html! {
                        <>
                        <section class="flex flex-col gap-4 border-b border-gray-400 pb-6">
                            <h3 class="font-bold">{"Try CSS Query section"}</h3>
                            <div class="flex flex-col gap-2">
                                <label class="text-sm font-medium">{"CSS Query:"}</label>
                                <textarea 
                                    oninput={on_css_query_change}
                                    class="w-full min-h-32 p-2 border border-gray-300 rounded focus:ring-2 focus:ring-green-500 outline-none" 
                                    placeholder="Enter your CSS selector here..."
                                />
                            </div>
                            
                            <div class="grid grid-cols-3 gap-2">
                                <div class="flex flex-col gap-1">
                                    <label class="text-[10px] font-bold text-gray-500 uppercase">{"Mode:"}</label>
                                    <select onchange={on_traversal_mode_change} class="p-1 border border-gray-300 rounded text-xs bg-white h-8">
                                        <option value="DFS" selected={*use_dfs}>{"DFS"}</option>
                                        <option value="BFS" selected={!*use_dfs}>{"BFS"}</option>
                                    </select>
                                </div>
                                <div class="flex flex-col gap-1">
                                    <label class="text-[10px] font-bold text-gray-500 uppercase">{"Threads:"}</label>
                                    <input 
                                        type="number" 
                                        value={threads.to_string()} 
                                        oninput={on_threads_change}
                                        class="p-1 border border-gray-300 rounded text-xs h-8" 
                                        min="1"
                                    />
                                </div>
                                <div class="flex flex-col gap-1">
                                    <div class="flex items-center gap-1">
                                        <input 
                                            type="checkbox" 
                                            id="limit-matched"
                                            checked={*limit_enabled}
                                            onchange={on_limit_enabled_change}
                                        />
                                        <label for="limit-matched" class="text-[10px] font-bold text-gray-500 uppercase">{"Limit:"}</label>
                                    </div>
                                    <input 
                                        type="number" 
                                        disabled={!*limit_enabled}
                                        value={search_limit.to_string()} 
                                        oninput={on_search_limit_change}
                                        class={format!("p-1 border border-gray-300 rounded text-xs h-8 {}", if !*limit_enabled { "bg-gray-100 text-gray-400" } else { "bg-white" })} 
                                        min="1"
                                    />
                                </div>
                            </div>

                            <button 
                                class="bg-green-600 text-white py-2 rounded font-medium hover:bg-green-700 active:scale-95 transition-all shadow-sm active:shadow-inner"
                                onclick={on_submit_css}
                            >
                                {"submit button for css query"}
                            </button>
                            
                            {if let Some(res) = result_data.first() {
                                let duration_text = if res.duration < 1000 {
                                    format!("{} µs", res.duration)
                                } else if res.duration < 1_000_000 {
                                    format!("{:.2} ms", res.duration as f64 / 1000.0)
                                } else {
                                    format!("{:.2} s", res.duration as f64 / 1_000_000.0)
                                };
                                html! {
                                    <div class="text-[10px] font-mono text-gray-600 space-y-0.5 mt-1">
                                        <div class="flex justify-between">
                                            <span>{"Visited Count:"}</span>
                                            <span>{res.nodes_count}</span>
                                        </div>
                                        <div class="flex justify-between">
                                            <span>{"Search Duration:"}</span>
                                            <span>{duration_text}</span>
                                        </div>
                                    </div>
                                }
                            } else { html! {} }}

                            <ResultPlayer 
                                is_open={!result_data.is_empty() && ctx.animation_type != crate::AnimationType::LCA} 
                                result_data={(*result_data).clone()} 
                                anim_type={crate::AnimationType::CSS}
                            />
                        </section>

                        <section class="flex flex-col gap-2 border-b border-gray-400 pb-4">
                            <h3 class="font-bold">{"Try to find LCA"}</h3>
                            <button 
                                class="bg-purple-600 hover:bg-purple-700 text-white py-2 rounded font-medium active:scale-95 transition-all shadow-sm active:shadow-inner"
                                onclick={on_submit_lca}
                            >
                                {"submit button for try lca"}
                            </button>
                            <div class="text-[10px] text-gray-500">
                                {format!("Selected Nodes for LCA: {:?}", ctx.lca_selected)}
                            </div>
                            
                            <ResultPlayer 
                                is_open={!lca_result_data.is_empty() && ctx.animation_type != crate::AnimationType::CSS} 
                                result_data={(*lca_result_data).clone()} 
                                anim_type={crate::AnimationType::LCA}
                            />
                        </section>
                        </>
                    }
                } else { html! {} }}
            </div>
        </div>
    }
}
