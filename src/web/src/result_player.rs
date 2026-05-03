use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_hooks::use_interval;
use serde::{Serialize, Deserialize};

const DEFAULT_SPEED: u32 = 1000;
const GRAPH_CONTAINER_ID: &str = "graph";

#[derive(Clone, Copy, PartialEq)]
enum NodeState {
    NotVisited,
    Visited,
    Selected,
    Current,
}

impl NodeState {
    fn value(&self) -> &'static str {
        match self {
            NodeState::NotVisited => "not-visited",
            NodeState::Visited => "visited",
            NodeState::Selected => "selected",
            NodeState::Current => "current",
        }
    }
}

#[derive(Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultItem {
    pub query: String,
    pub paths: Vec<Vec<usize>>,
    pub selected: Vec<usize>,
}

#[derive(Properties, PartialEq)]
pub struct ResultPlayerProps {
    pub is_open: bool,
    #[prop_or_default]
    pub result_data: Vec<ResultItem>,
}

#[derive(Clone, Copy, PartialEq)]
enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

#[component]
pub fn ResultPlayer(props: &ResultPlayerProps) -> Html {
    let speed = use_state(|| DEFAULT_SPEED);
    let step = use_state(|| 0usize);
    let tick = use_state(|| 0usize);
    let playback_state = use_state(|| PlaybackState::Stopped);

    let total_steps = props.result_data.len();
    let is_playing = *playback_state == PlaybackState::Playing;

    let current_result = props.result_data.get(*step);
    let max_ticks = current_result
        .map(|r| r.paths.iter().map(|p| p.len()).max().unwrap_or(0))
        .unwrap_or(0);

    // Playback Loop
    {
        let playback_state = playback_state.clone();
        let step = step.clone();
        let tick = tick.clone();
        let result_data = props.result_data.clone();
        let speed = *speed;

        use_interval(
            move || {
                let current_tick = *tick;
                let current_step = *step;

                if let Some(res) = result_data.get(current_step) {
                    let res_max_ticks = res.paths.iter().map(|p| p.len()).max().unwrap_or(0);
                    
                    if current_tick + 1 < res_max_ticks {
                        tick.set(current_tick + 1);
                    } else if current_step + 1 < result_data.len() {
                        step.set(current_step + 1);
                        tick.set(0);
                    } else {
                        playback_state.set(PlaybackState::Stopped);
                    }
                }
            },
            if is_playing { speed } else { 0 },
        );
    }

    let toggle_playback = {
        let playback_state = playback_state.clone();
        Callback::from(move |_| {
            if *playback_state == PlaybackState::Playing {
                playback_state.set(PlaybackState::Paused);
            } else {
                playback_state.set(PlaybackState::Playing);
            }
        })
    };

    let handle_reset = {
        let playback_state = playback_state.clone();
        let step = step.clone();
        let tick = tick.clone();
        Callback::from(move |_| {
            playback_state.set(PlaybackState::Stopped);
            step.set(0);
            tick.set(0);
            clear_all_node_colors();
        })
    };

    let handle_prev = {
        let step = step.clone();
        let tick = tick.clone();
        Callback::from(move |_| {
            if *tick > 0 {
                tick.set(*tick - 1);
            } else if *step > 0 {
                step.set(*step - 1);
                tick.set(0);
            }
        })
    };

    let handle_next = {
        let step = step.clone();
        let tick = tick.clone();
        let total_steps = total_steps;
        let max_ticks = max_ticks;
        Callback::from(move |_| {
            if *tick + 1 < max_ticks {
                tick.set(*tick + 1);
            } else if *step + 1 < total_steps {
                step.set(*step + 1);
                tick.set(0);
            }
        })
    };

    let handle_speed_change = {
        let speed = speed.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            if let Ok(val) = input.value().parse::<u32>() {
                speed.set(val);
            }
        })
    };

    // Effect to update DOM colors
    {
        let step_val = *step;
        let tick_val = *tick;
        let result_data = props.result_data.clone();
        let is_open = props.is_open;
        let pb_state = *playback_state;

        use_effect_with((step_val, tick_val, is_open, pb_state), move |(s, t, open, state)| {
            if !*open {
                clear_all_node_colors();
            } else if let Some(res) = result_data.get(*s) {
                let mut visited = std::collections::HashSet::new();
                let mut current_nodes = std::collections::HashSet::new();
                let is_stopped = *state == PlaybackState::Stopped;

                for path in &res.paths {
                    let bound = (*t + 1).min(path.len());
                    for &node_id in &path[..bound] {
                        visited.insert(node_id);
                    }
                    if !is_stopped && *t < path.len() {
                        current_nodes.insert(path[*t]);
                    }
                }

                color_nodes(&visited, &current_nodes, &res.selected);
            }
            || ()
        });
    }

    if !props.is_open || props.result_data.is_empty() {
        return html! {};
    }

    html! {
        <div class="flex flex-col gap-3 p-2 bg-white rounded border border-gray-300">
            <div class="flex flex-row justify-between items-center gap-1">
                <button onclick={handle_prev} class="px-2 py-1 bg-gray-200 hover:bg-gray-300 active:scale-95 transition-all rounded text-xs shadow-sm">{"Prev"}</button>
                <div class="font-mono text-[10px] font-bold text-blue-600 truncate flex-1 text-center px-1">
                    {current_result.map(|r| r.query.clone()).unwrap_or_default()}
                </div>
                <button onclick={handle_next} class="px-2 py-1 bg-gray-200 hover:bg-gray-300 active:scale-95 transition-all rounded text-xs shadow-sm">{"Next"}</button>
            </div>

            <div class="flex flex-row gap-1">
                <button 
                    onclick={toggle_playback} 
                    class={format!("flex-[2] py-1 rounded text-xs text-white active:scale-95 transition-all shadow-sm active:shadow-inner {}", 
                        if is_playing { "bg-yellow-500 hover:bg-yellow-600" } else { "bg-green-600 hover:bg-green-700" }
                    )}
                >
                    {if is_playing { "Pause" } else { "Play" }}
                </button>
                <button onclick={handle_reset} class="flex-1 py-1 bg-red-600 text-white rounded text-xs hover:bg-red-700 active:scale-95 transition-all shadow-sm active:shadow-inner">{"Reset"}</button>
            </div>

            <div class="flex flex-row items-center gap-2">
                <label class="text-[9px] font-bold text-gray-400 uppercase">{"Interval:"}</label>
                <input 
                    type="number" 
                    value={speed.to_string()} 
                    oninput={handle_speed_change} 
                    class="flex-1 text-xs p-1 border border-gray-300 rounded"
                    min="10"
                />
                <span class="text-[9px] text-gray-500">{format!("{}/{}", *tick + 1, max_ticks)}</span>
            </div>
        </div>
    }
}

fn color_nodes(
    visited: &std::collections::HashSet<usize>,
    current: &std::collections::HashSet<usize>,
    selected: &[usize],
) {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return; };
    let Some(container) = document.get_element_by_id(GRAPH_CONTAINER_ID) else { return; };

    if let Ok(node_cards) = container.query_selector_all(".graph-node") {
        for i in 0..node_cards.length() {
            if let Some(card_el) = node_cards.get(i).and_then(|n| n.dyn_into::<web_sys::Element>().ok()) {
                let id_str = card_el.get_attribute("id").unwrap_or_default();
                let node_index = id_str
                    .strip_prefix("graph-node-")
                    .and_then(|s| s.parse::<usize>().ok())
                    .unwrap_or(i as usize);

                let state = if current.contains(&node_index) {
                    NodeState::Current
                } else if selected.contains(&node_index) && visited.contains(&node_index) {
                    NodeState::Selected
                } else if visited.contains(&node_index) {
                    NodeState::Visited
                } else {
                    NodeState::NotVisited
                };

                let _ = card_el.set_attribute("data-state", state.value());
            }
        }
    }
}

fn clear_all_node_colors() {
    let Some(document) = web_sys::window().and_then(|w| w.document()) else { return; };
    let Some(container) = document.get_element_by_id(GRAPH_CONTAINER_ID) else { return; };

    if let Ok(node_cards) = container.query_selector_all(".graph-node") {
        for i in 0..node_cards.length() {
            if let Some(card_el) = node_cards.get(i).and_then(|n| n.dyn_into::<web_sys::Element>().ok()) {
                let _ = card_el.set_attribute("data-state", "not-visited");
            }
        }
    }
}
