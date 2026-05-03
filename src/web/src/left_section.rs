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
}

#[derive(Deserialize)]
struct CSSQueryResponse {
    results: Vec<ResultItem>,
}

#[component]
pub fn LeftSection() -> Html {
    let ctx = use_context::<GraphContext>().unwrap();
    
    let is_open = use_state(|| false);
    let html_submitted = use_state(|| false);
    
    // HTML Input State
    let input_type = use_state(|| "file".to_string());
    let file_content = use_state(|| String::new());
    let file_reader = use_state(|| None::<gloo::file::callbacks::FileReader>);
    let url_input_ref = use_node_ref();
    let text_input_ref = use_node_ref();
    let file_input_ref = use_node_ref();

    // CSS Query State
    let css_query = use_state(|| String::new());
    let use_dfs = use_state(|| false);
    let threads = use_state(|| 1usize);
    let result_data = use_state(|| Vec::<ResultItem>::new());

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

        Callback::from(move |_| {
            let content = match (*input_type).as_str() {
                "file" => (*file_content).clone(),
                "url" => url_input_ref.cast::<HtmlInputElement>().map(|i| i.value()).unwrap_or_default(),
                "plain_text" => text_input_ref.cast::<HtmlInputElement>().map(|i| i.value()).unwrap_or_default(),
                _ => String::new(),
            };
            
            let payload = QueryPostBody {
                query_type: "html".to_string(),
                input_type: (*input_type).clone(),
                content,
                css_query: String::new(),
                use_dfs: false,
                threads: None,
            };

            let ctx = ctx.clone();
            let html_submitted = html_submitted.clone();
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
        let css_query = css_query.clone();
        let use_dfs = use_dfs.clone();
        let threads = threads.clone();
        let result_data = result_data.clone();

        Callback::from(move |_| {
            let payload = QueryPostBody {
                query_type: "css".to_string(),
                input_type: String::new(),
                content: String::new(),
                css_query: (*css_query).clone(),
                use_dfs: *use_dfs,
                threads: Some(*threads),
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
        Callback::from(move |_| {
            let payload = QueryPostBody {
                query_type: "lca".to_string(),
                input_type: String::new(),
                content: String::new(),
                css_query: String::new(),
                use_dfs: false,
                threads: None,
            };

            spawn_local(async move {
                let res = Request::post("http://127.0.0.1:8081/query")
                    .json(&payload)
                    .expect("Failed to serialize body")
                    .send()
                    .await;
                
                match res {
                    Ok(res) => {
                        if !res.ok() {
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

    let on_traversal_mode_change = {
        let use_dfs = use_dfs.clone();
        Callback::from(move |e: Event| {
            let select: HtmlInputElement = e.target_unchecked_into();
            use_dfs.set(select.value() == "DFS");
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
                </section>

                {if *html_submitted {
                    html! {
                        <>
                        <section class="flex flex-col gap-4 border-b border-gray-400 pb-4">
                            <h3 class="font-bold">{"Try CSS Query section"}</h3>
                            <div class="flex flex-col gap-2">
                                <label class="text-sm font-medium">{"CSS Query:"}</label>
                                <textarea 
                                    oninput={on_css_query_change}
                                    class="w-full min-h-32 p-2 border border-gray-300 rounded focus:ring-2 focus:ring-green-500 outline-none" 
                                    placeholder="Enter your CSS selector here..."
                                />
                            </div>
                            
                            <div class="flex flex-row gap-2">
                                <div class="flex flex-col gap-1 flex-1">
                                    <label class="text-[10px] font-bold text-gray-500 uppercase">{"Mode:"}</label>
                                    <select onchange={on_traversal_mode_change} class="p-1 border border-gray-300 rounded text-xs bg-white">
                                        <option value="BFS" selected={!*use_dfs}>{"BFS"}</option>
                                        <option value="DFS" selected={*use_dfs}>{"DFS"}</option>
                                    </select>
                                </div>
                                <div class="flex flex-col gap-1 flex-1">
                                    <label class="text-[10px] font-bold text-gray-500 uppercase">{"Threads:"}</label>
                                    <input 
                                        type="number" 
                                        value={threads.to_string()} 
                                        oninput={on_threads_change}
                                        class="p-1 border border-gray-300 rounded text-xs" 
                                        min="1"
                                    />
                                </div>
                            </div>

                            <button class="bg-green-600 text-white py-2 rounded font-medium hover:bg-green-700 active:scale-95 transition-all shadow-sm active:shadow-inner" onclick={on_submit_css}>{"submit button for css query"}</button>
                            
                            <ResultPlayer is_open={true} result_data={(*result_data).clone()} />
                        </section>

                        <section class="flex flex-col gap-2 border-b border-gray-400 pb-4">
                            <h3 class="font-bold">{"Try to find LCA"}</h3>
                            <button class="bg-purple-600 text-white py-2 rounded font-medium hover:bg-purple-700 active:scale-95 transition-all shadow-sm active:shadow-inner" onclick={on_submit_lca}>{"submit button for try lca"}</button>
                        </section>
                        </>
                    }
                } else { html! {} }}
            </div>
        </div>
    }
}
