use wasm_bindgen::JsCast;
use yew::prelude::*;
use yew_hooks::use_interval;
use serde::{Serialize, Deserialize};

use crate::{GraphContext, GraphAction};

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
    pub paths: Vec<Vec<(usize, usize)>>,
    pub selected: Vec<usize>,
    pub duration: u128,
    pub nodes_count: usize,
    pub logs: Vec<String>,
    #[serde(default)]
    pub err: String,
}

#[derive(Properties, PartialEq)]
pub struct ResultPlayerProps {
    pub is_open: bool,
    #[prop_or_default]
    pub result_data: Vec<ResultItem>,
    #[prop_or_default]
    pub disabled: bool,
    pub anim_type: crate::AnimationType,
}

#[derive(Clone, Copy, PartialEq)]
enum PlaybackState {
    Stopped,
    Playing,
    Paused,
}

#[component]
pub fn ResultPlayer(props: &ResultPlayerProps) -> Html {
    let ctx = use_context::<GraphContext>().unwrap();
    let speed = use_state(|| DEFAULT_SPEED);
    let step = use_state(|| 0usize);
    let tick = use_state(|| 0usize);
    let playback_state = use_state(|| PlaybackState::Stopped);

    let total_steps = props.result_data.len();
    let is_playing = *playback_state == PlaybackState::Playing && !props.disabled;

    // Reset player when new results arrive
    {
        let step = step.clone();
        let tick = tick.clone();
        let playback_state = playback_state.clone();
        use_effect_with(props.result_data.clone(), move |_| {
            step.set(0);
            tick.set(0);
            playback_state.set(PlaybackState::Stopped);
            || ()
        });
    }

    // Simulation: Calculate when each node is actually activated based on dependencies
    let step_val = *step;
    let result_data = props.result_data.clone();
    let activation_ticks = use_memo((result_data, step_val), |(data, s)| {
        let mut ticks = std::collections::HashMap::<usize, usize>::new();
        if let Some(res) = data.get(*s) {
            let mut pointers = vec![0usize; res.paths.len()];
            let mut current_tick = 0usize;
            let mut visited = std::collections::HashSet::new();

            loop {
                let mut fired_this_tick = Vec::new();
                for i in 0..res.paths.len() {
                    if pointers[i] < res.paths[i].len() {
                        let (dep, node) = res.paths[i][pointers[i]];
                        // A node fires if it has no dependency (dep == node) OR if its dependency was already visited
                        if dep == node || visited.contains(&dep) {
                            fired_this_tick.push((i, node));
                        }
                    }
                }

                if fired_this_tick.is_empty() {
                    break;
                }

                for (thread_idx, node_id) in fired_this_tick {
                    if !ticks.contains_key(&node_id) {
                        ticks.insert(node_id, current_tick);
                    }
                    visited.insert(node_id);
                    pointers[thread_idx] += 1;
                }
                current_tick += 1;
                
                if current_tick > 10000 { break; }
            }
        }
        ticks
    });

    let max_ticks = if activation_ticks.is_empty() { 0 } else {
        *activation_ticks.values().max().unwrap_or(&0) + 1
    };

    let current_result = props.result_data.get(*step);

    // Playback Loop
    {
        let playback_state = playback_state.clone();
        let step = step.clone();
        let tick = tick.clone();
        let result_data = props.result_data.clone();
        let speed = *speed;
        let max_ticks_val = max_ticks;
        let disabled = props.disabled;

        use_interval(
            move || {
                if disabled { return; }
                let current_tick = *tick;
                let current_step = *step;

                if current_tick + 1 < max_ticks_val {
                    tick.set(current_tick + 1);
                } else if current_step + 1 < result_data.len() {
                    step.set(current_step + 1);
                    tick.set(0);
                } else {
                    // Stay at the last step/tick instead of stopping
                    // This keeps animation_active true and Reset enabled
                }
            },
            if is_playing { speed } else { 0 },
        );
    }

    let toggle_playback = {
        let playback_state = playback_state.clone();
        let ctx = ctx.clone();
        let disabled = props.disabled;
        let anim_type = props.anim_type;
        Callback::from(move |_| {
            if disabled { return; }
            if *playback_state == PlaybackState::Playing {
                playback_state.set(PlaybackState::Paused);
            } else {
                playback_state.set(PlaybackState::Playing);
                ctx.dispatch(GraphAction::SetAnimationType(anim_type));
                ctx.dispatch(GraphAction::SetAnimationActive(true));
            }
        })
    };

    let handle_reset = {
        let playback_state = playback_state.clone();
        let step = step.clone();
        let tick = tick.clone();
        let ctx = ctx.clone();
        let disabled = props.disabled;
        Callback::from(move |_| {
            if disabled { return; }
            playback_state.set(PlaybackState::Stopped);
            step.set(0);
            tick.set(0);
            ctx.dispatch(GraphAction::SetAnimationActive(false));
            ctx.dispatch(GraphAction::SetAnimationType(crate::AnimationType::None));
        })
    };

    let handle_prev = {
        let step = step.clone();
        let tick = tick.clone();
        let ctx = ctx.clone();
        let disabled = props.disabled;
        let anim_type = props.anim_type;
        Callback::from(move |_| {
            if disabled { return; }
            ctx.dispatch(GraphAction::SetAnimationType(anim_type));
            ctx.dispatch(GraphAction::SetAnimationActive(true));
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
        let max_ticks_val = max_ticks;
        let ctx = ctx.clone();
        let disabled = props.disabled;
        let anim_type = props.anim_type;
        Callback::from(move |_| {
            if disabled { return; }
            ctx.dispatch(GraphAction::SetAnimationType(anim_type));
            ctx.dispatch(GraphAction::SetAnimationActive(true));
            if *tick + 1 < max_ticks_val {
                tick.set(*tick + 1);
            } else if *step + 1 < total_steps {
                step.set(*step + 1);
                tick.set(0);
            }
        })
    };

    let handle_speed_change = {
        let speed = speed.clone();
        let disabled = props.disabled;
        Callback::from(move |e: InputEvent| {
            if disabled { return; }
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
        let is_open = props.is_open;
        let disabled = props.disabled;
        let pb_state = *playback_state;
        let activation_ticks_val = activation_ticks.clone();
        let current_res = current_result.cloned();
        let anim_active = ctx.animation_active;

        let total_steps_val = total_steps;
        let max_ticks_val = max_ticks;

        use_effect_with((step_val, tick_val, is_open, pb_state, anim_active, disabled), move |(s, t, open, state, active, dis)| {
            if !*open || !*active || *dis {
                // Only clear if we are the ones who were supposed to be active or if we just became disabled
                if *open && *active && !*dis {
                   // This branch won't be hit because of the if condition above
                } else {
                    // We only clear colors if we are the active component OR if we just transitioned to inactive
                    // But to avoid race conditions with other ResultPlayers, we should be careful.
                    // For now, if we are NOT open or NOT active or disabled, we clear.
                    // To mitigate the race, we could check ctx.animation_type, but ResultPlayer is generic.
                    clear_all_node_colors();
                }
            } else if let Some(res) = current_res {
                let mut visited = std::collections::HashSet::new();
                let mut current_nodes = std::collections::HashSet::new();
                let is_stopped = *state == PlaybackState::Stopped;
                let is_at_end = (*t + 1 >= max_ticks_val) && (*s + 1 >= total_steps_val);

                for (&node_id, &activation_tick) in activation_ticks_val.iter() {
                    if activation_tick <= *t {
                        visited.insert(node_id);
                        if activation_tick == *t && !is_stopped && !is_at_end {
                            current_nodes.insert(node_id);
                        }
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

    let container_class = if props.disabled {
        "flex flex-col gap-3 p-2 bg-gray-100 rounded border border-gray-200 opacity-60 pointer-events-none"
    } else {
        "flex flex-col gap-3 p-2 bg-white rounded border border-gray-300 shadow-sm"
    };

    html! {
        <div class={container_class}>
            <div class="flex flex-row justify-between items-center gap-1">
                <button disabled={props.disabled} onclick={handle_prev} class="px-2 py-1 bg-gray-200 hover:bg-gray-300 active:scale-95 transition-all rounded text-xs shadow-sm">{"Prev"}</button>
                <div class="font-mono text-[10px] font-bold text-blue-600 truncate flex-1 text-center px-1">
                    {current_result.map(|r| r.query.clone()).unwrap_or_default()}
                </div>
                <button disabled={props.disabled} onclick={handle_next} class="px-2 py-1 bg-gray-200 hover:bg-gray-300 active:scale-95 transition-all rounded text-xs shadow-sm">{"Next"}</button>
            </div>

            <div class="flex flex-row gap-1">
                <button 
                    disabled={props.disabled}
                    onclick={toggle_playback} 
                    class={format!("flex-[2] py-1 rounded text-xs text-white active:scale-95 transition-all shadow-sm active:shadow-inner {}", 
                        if is_playing { "bg-yellow-500 hover:bg-yellow-600" } else { "bg-green-600 hover:bg-green-700" }
                    )}
                >
                    {if is_playing { "Pause" } else { "Play" }}
                </button>
                <button 
                    disabled={props.disabled || *playback_state == PlaybackState::Stopped}
                    onclick={handle_reset} 
                    class={format!("flex-1 py-1 rounded text-xs text-white transition-all shadow-sm active:shadow-inner {}",
                        if props.disabled || *playback_state == PlaybackState::Stopped { "bg-gray-400 cursor-not-allowed" } else { "bg-red-600 hover:bg-red-700 active:scale-95" }
                    )}
                >
                    {"Reset"}
                </button>
            </div>

            <div class="flex flex-row items-center gap-2">
                <label class="text-[9px] font-bold text-gray-400 uppercase">{"Interval:"}</label>
                <input 
                    disabled={props.disabled}
                    type="number" 
                    value={speed.to_string()} 
                    oninput={handle_speed_change} 
                    class="flex-1 text-xs p-1 border border-gray-300 rounded"
                    min="10"
                />
                <span class="text-[9px] text-gray-500">{format!("{}/{}", if max_ticks > 0 { *tick + 1 } else { 0 }, max_ticks)}</span>
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
