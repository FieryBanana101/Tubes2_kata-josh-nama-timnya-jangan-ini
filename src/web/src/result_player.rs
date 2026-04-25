use wasm_bindgen::JsCast;
use web_sys::Element;
use yew::prelude::*;
use yew_hooks::use_interval;

use crate::GraphContext;

const DEFAULT_SPEED: u32 = 1000;

const GRAPH_CONTAINER_ID: &str = "graph";

#[derive(Clone, Copy, PartialEq)]
enum NodeState {
    NotVisited,
    Visited,
    Intermediate,
    Selected,
    Current,
}

impl NodeState {
    fn value(&self) -> &'static str {
        match self {
            NodeState::NotVisited => "not-visited",
            NodeState::Visited => "visited",
            NodeState::Intermediate => "intermediate",
            NodeState::Selected => "selected",
            NodeState::Current => "current",
        }
    }
}

#[derive(Clone, PartialEq)]
pub struct ResultItem {
    pub query: String,
    pub paths: Vec<Vec<u32>>,
    pub selected: Vec<u32>,
    pub traversal_path: Vec<u32>,
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
    let _ctx = use_context::<GraphContext>().unwrap();

    let speed = use_state(|| DEFAULT_SPEED);
    let _use_dfs = use_state(|| true);
    let step = use_state(|| 0u32);
    let sub_step = use_state(|| 0u32);
    let playback_state = use_state(|| PlaybackState::Stopped);

    let _affected_nodes = use_state(|| Vec::<u32>::new());
    let _matched_nodes = use_state(|| Vec::<u32>::new());

    let total_steps = if props.result_data.is_empty() {
        1u32
    } else {
        props.result_data.len() as u32
    };

    let is_playing = *playback_state == PlaybackState::Playing;

    let result_data = props.result_data.clone();
    let step_clone = step.clone();
    let sub_step_clone = sub_step.clone();
    let playback_state_clone = playback_state.clone();
    let speed_handle = speed.clone();

    use_interval(
        move || {
            if !result_data.is_empty() {
                let current_idx = *step_clone;
                let current_result = result_data.get(current_idx as usize);

                if let Some(result) = current_result {
                    let current_sub = *sub_step_clone;

                    if current_sub < result.traversal_path.len() as u32 - 1 {
                        sub_step_clone.set(current_sub + 1);
                    } else if current_idx < result_data.len() as u32 - 1 {
                        step_clone.set(current_idx + 1);
                        sub_step_clone.set(0);
                    } else {
                        playback_state_clone.set(PlaybackState::Stopped);
                    }
                }
            }
        },
        if is_playing { *speed_handle } else { 0 },
    );

    let handle_play = {
        let playback_state = playback_state.clone();
        Callback::from(move |_| {
            playback_state.set(PlaybackState::Playing);
        })
    };

    let handle_pause = {
        let playback_state = playback_state.clone();
        Callback::from(move |_| {
            playback_state.set(PlaybackState::Paused);
        })
    };

    let handle_stop = {
        let playback_state = playback_state.clone();
        let step = step.clone();
        let sub_step = sub_step.clone();
        Callback::from(move |_| {
            playback_state.set(PlaybackState::Stopped);
            step.set(0);
            sub_step.set(0);
            clear_all_node_colors();
        })
    };

    let handle_prev = {
        let step = step.clone();
        let sub_step = sub_step.clone();
        Callback::from(move |_| {
            if *sub_step > 0 {
                sub_step.set(*sub_step - 1);
            } else if *step > 0 {
                step.set(*step - 1);
            }
        })
    };

    let handle_next = {
        let step = step.clone();
        let sub_step = sub_step.clone();
        let result_data = props.result_data.clone();
        Callback::from(move |_| {
            let current = result_data.get(*step as usize);
            if let Some(res) = current {
                if *sub_step < res.traversal_path.len() as u32 - 1 {
                    sub_step.set(*sub_step + 1);
                } else if *step < result_data.len() as u32 - 1 {
                    step.set(*step + 1);
                    sub_step.set(0);
                }
            }
        })
    };

    let handle_speed_change = {
        let speed = speed.clone();
        Callback::from(move |e: yew::events::InputEvent| {
            let input = e.target_unchecked_into::<web_sys::HtmlInputElement>();
            let val = input.value().parse().unwrap_or(1000);
            speed.set(val);
        })
    };

    let handle_speed_up = {
        let speed = speed.clone();
        Callback::from(move |_| {
            let new_speed = (*speed as f64 * 0.5).max(100f64);
            speed.set(new_speed as u32);
        })
    };

    let handle_speed_down = {
        let speed = speed.clone();
        Callback::from(move |_| {
            let new_speed = (*speed as f64 * 2.0).min(10000f64);
            speed.set(new_speed as u32);
        })
    };

    let current_step = *step;
    let current_sub_step = *sub_step;
    let current_speed = *speed;

    let (affected_display, matched_display, total_sub_steps) = {
        let current_result = props.result_data.get(current_step as usize);
        let total_sub = current_result
            .map(|r| r.traversal_path.len() as u32)
            .unwrap_or(1);

        let affected: Vec<Html> = if let Some(result) = current_result {
            // unique
            result
                .traversal_path
                .iter()
                .take(current_sub_step as usize + 1)
                .fold(Vec::new(), |mut acc, &node| {
                    if !acc.contains(&node) {
                        acc.push(node);
                    }
                    acc
                })
                .into_iter()
                .map(|node| html! { <p>{node}</p> })
                .collect()
        } else {
            vec![]
        };

        let matched: Vec<Html> = if let Some(result) = current_result {
            result
                .traversal_path
                .iter()
                .take(current_sub_step as usize + 1)
                .filter(|&&node| result.selected.contains(&node))
                .fold(Vec::new(), |mut acc, &node| {
                    if !acc.contains(&node) {
                        acc.push(node);
                    }
                    acc
                })
                .into_iter()
                .map(|node| html! { <p>{node}</p> })
                .collect()
        } else {
            vec![]
        };

        (affected, matched, total_sub)
    };

    let step_value = *step;
    let sub_step_value = *sub_step;
    let result_data_for_effect = props.result_data.clone();
    let is_open = props.is_open;

    use_effect_with(
        (step_value, sub_step_value),
        move |(current_step, current_sub)| {
            let result_data = result_data_for_effect.clone();
            if let Some(current) = result_data.get(*current_step as usize) {
                let traversed: Vec<u32> = current
                    .traversal_path
                    .iter()
                    .take(*current_sub as usize + 1)
                    .fold(Vec::new(), |mut acc, &node| {
                        if acc.contains(&node) {
                            acc.retain(|&n| n != node);
                        } else {
                            acc.push(node);
                        }
                        acc
                    });

                let visited: Vec<u32> = current
                    .traversal_path
                    .iter()
                    .take(*current_sub as usize + 1)
                    // unique
                    .fold(Vec::new(), |mut acc, &node| {
                        if !acc.contains(&node) {
                            acc.push(node);
                        }
                        acc
                    });

                color_nodes(
                    &traversed,
                    &current.selected,
                    &visited,
                    &current.paths,
                    *current_sub as usize,
                );
            }
            || ()
        },
    );

    use_effect_with(is_open, move |open| {
        if !*open {
            clear_all_node_colors();
        }
        || ()
    });

    html! {
        <div class="border-t border-black flex flex-col gap-4 hidden data-[open=true]:block" data-open={props.is_open.to_string()}>
            <div class="flex flex-col gap-2 px-4 py-2">
                <div class="flex flex-row px-4 py-2 justify-between items-center">
                    <button onclick={handle_prev} class="px-2 py-1 bg-gray-300">{"Prev"}</button>
                    <div class="flex flex-col items-center">
                        {props.result_data.get(current_step as usize).map(|r| html! { <p>{r.query.clone()}</p> }).unwrap_or_default()}
                    </div>
                    <button onclick={handle_next} class="px-2 py-1 bg-gray-300">{"Next"}</button>
                </div>
                <div class="flex flex-row justify-around">
                    <span>{format!("Step: {}/{}", current_step + 1, total_steps)}</span>
                    <span>{format!("Sub-step: {}/{}", current_sub_step + 1, total_sub_steps)}</span>
                </div>
                <div class="flex flex-row justify-center gap-2 p-2">
                    <button onclick={handle_speed_down} class="px-4 py-1 bg-blue-500 text-white">{"/2"}</button>
                    <button onclick={handle_play} class="px-4 py-1 bg-green-500 text-white">{"Play"}</button>
                    <button onclick={handle_pause} class="px-4 py-1 bg-yellow-500 text-white">{"Pause"}</button>
                    <button onclick={handle_stop} class="px-4 py-1 bg-red-500 text-white">{"Stop"}</button>
                    <button onclick={handle_speed_up} class="px-4 py-1 bg-blue-500 text-white">{"2x"}</button>
                </div>
                <div class="flex flex-row justify-between p-2">
                    <label>
                        {"Speed (ms): "}
                        <input type="number" value={current_speed.to_string()} oninput={handle_speed_change} class="border border-gray-300 p-1" />
                    </label>
                </div>
                <div class="m-2 flex flex-col gap-2">
                    <p>{"Matched Nodes:"}</p>
                    <div class="flex flex-col gap-2">
                        {matched_display}
                    </div>
                </div>
                <div class="m-2 flex flex-col gap-2">
                    <p>{"Affected Nodes:"}</p>
                    <div class="flex flex-col gap-2">
                        {affected_display}
                    </div>
                </div>
            </div>
        </div>
    }
}

fn color_nodes(
    traversed: &[u32],
    selected: &[u32],
    visited: &[u32],
    paths: &[Vec<u32>],
    _current_sub_step: usize,
) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(container) = document.get_element_by_id(GRAPH_CONTAINER_ID) else {
        return;
    };

    reset_node_colors(&container);

    if let Ok(node_cards) = container.query_selector_all(".graph-node") {
        let len = node_cards.length();
        for i in 0..len {
            if let Some(card) = node_cards.get(i) {
                if let Some(card_el) = card.dyn_ref::<web_sys::Element>() {
                    let id_str = card_el.get_attribute("id").unwrap_or_default();
                    let node_index: u32 = id_str
                        .strip_prefix("graph-node-")
                        .and_then(|s| s.parse().ok())
                        .unwrap_or(i as u32);

                    let mut state = NodeState::NotVisited;

                    if selected.contains(&node_index) && visited.contains(&node_index) {
                        state = NodeState::Selected;
                    } else {
                        let intermediates: Vec<u32> = paths
                            .iter()
                            .filter(|p| p.last().map(|n| visited.contains(n)).unwrap_or(false))
                            .flatten()
                            .cloned()
                            .collect();
                        if intermediates.contains(&node_index) {
                            state = NodeState::Intermediate;
                        } else if visited.contains(&node_index) {
                            state = NodeState::Visited;
                        }
                    }

                    if traversed.last() == Some(&node_index) {
                        state = NodeState::Current;
                    }

                    let _ = card_el.set_attribute("data-state", state.value());
                }
            }
        }
    }
}

fn reset_node_colors(container: &Element) {
    if let Ok(node_cards) = container.query_selector_all(".graph-node") {
        let len = node_cards.length();
        for i in 0..len {
            if let Some(card) = node_cards.get(i) {
                if let Some(card_el) = card.dyn_ref::<web_sys::Element>() {
                    let _ = card_el.set_attribute("data-state", "not-visited");
                }
            }
        }
    }
}

fn clear_all_node_colors() {
    let Some(window) = web_sys::window() else {
        return;
    };
    let Some(document) = window.document() else {
        return;
    };
    let Some(container) = document.get_element_by_id(GRAPH_CONTAINER_ID) else {
        return;
    };

    reset_node_colors(&container);
}
