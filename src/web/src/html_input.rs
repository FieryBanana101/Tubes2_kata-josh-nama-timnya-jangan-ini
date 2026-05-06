use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct Props {
    pub input_type_str: String,
    pub onchange: Callback<Event>,
    pub file_input_ref: NodeRef,
    pub url_input_ref: NodeRef,
    pub text_input_ref: NodeRef,
    pub on_file_change: Callback<Event>,
}

#[component]
pub fn HtmlInput(props: &Props) -> Html {
    let onchange = props.onchange.clone();

    html! {
        <div class="w-full flex flex-col gap-2 border border-b">
            <select name="Input Type" {onchange}>
                <option value="file" selected={true}>{"File"}</option>
                <option value="url">{"URL"}</option>
                <option value="plain_text">{"Plain Text"}</option>
            </select>
            <div>
                <input ref={props.file_input_ref.clone()} onchange={props.on_file_change.clone()} data-active={(props.input_type_str == "file").to_string()} class="hidden data-[active=true]:block" type="file"/>
                <input ref={props.url_input_ref.clone()} data-active={(props.input_type_str == "url").to_string()} class="hidden data-[active=true]:block w-full" type="text" />
                <textarea ref={props.text_input_ref.clone()} data-active={(props.input_type_str == "plain_text").to_string()} class="hidden data-[active=true]:block w-full min-h-[300px]"/>
            </div>
        </div>
    }
}