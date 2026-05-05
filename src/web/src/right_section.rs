use crate::{GraphContext, GraphAction};
use yew::prelude::*;

#[component]
pub fn RightSection() -> Html {
    let ctx = use_context::<GraphContext>().unwrap();
    let is_open = use_state(|| false);

    let toggle_open = {
        let is_open = is_open.clone();
        move |_| is_open.set(!(*is_open))
    };

    let on_clear_logs = {
        let ctx = ctx.clone();
        Callback::from(move |_| {
            ctx.dispatch(GraphAction::ClearLogs);
        })
    };

    html! {
        <div class="absolute right-0 top-0 z-50 flex flex-row-reverse">
            <button 
                class="absolute h-[2em] right-0 top-0 bg-gray-500 px-4 text-white text-xs font-bold rounded-bl-lg active:scale-95 transition-all shadow-md z-[60]" 
                onclick={toggle_open}
            >
                {if *is_open {"Close"} else {"Open"} }
            </button>
            <div data-open={(*is_open).to_string()}
                 class="flex flex-col gap-4 w-96 h-screen bg-gray-200 data-[open=false]:hidden border-l border-black overflow-y-scroll p-6 pt-12 shadow-2xl transition-all">
                
                <div class="flex justify-between items-center border-b border-gray-400 pb-4">
                    <h3 class="font-bold text-lg">{"Activity Log"}</h3>
                    <button 
                        onclick={on_clear_logs}
                        class="bg-red-500 hover:bg-red-600 text-white text-[10px] px-2 py-1 rounded font-bold transition-all active:scale-95"
                    >
                        {"Clear Logs"}
                    </button>
                </div>

                <div class="flex flex-col gap-2 font-mono text-[11px]">
                    { if ctx.activity_logs.is_empty() {
                        html! { <div class="text-gray-500 italic text-center py-4">{"No logs recorded yet"}</div> }
                    } else {
                        html! {
                            <>
                            { for ctx.activity_logs.iter().enumerate().rev().map(|(i, log)| {
                                html! {
                                    <div key={i} class="p-2 bg-white rounded border border-gray-300 shadow-sm break-words">
                                        <span class="text-blue-600 font-bold mr-2">{format!("[{:03}]", i + 1)}</span>
                                        {log}
                                    </div>
                                }
                            }) }
                            </>
                        }
                    } }
                </div>
            </div>
        </div>
    }
}
