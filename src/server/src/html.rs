use html5gum::{HtmlString, Token, Tokenizer};
use reqwest;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fmt, fs, io, sync::Arc};

#[derive(Debug, Clone)]
pub enum Node {
    Element(Arc<Element>),
    Text(Arc<String>),
}

#[derive(Debug, Clone)]
pub struct Element {
    pub global_id: usize,
    pub tag: String,
    pub attributes: HashMap<String, String>,
    pub children: Vec<Node>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenizerStep {
    pub step_type: String,
    pub tag: Option<String>,
    pub position: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenizerTraversal {
    pub steps: Vec<TokenizerStep>,
}



fn closes_tag(current: &str, incoming: &str) -> bool {
    match current {
        "p" => matches!(
            incoming,
            "address"
                | "article"
                | "aside"
                | "blockquote"
                | "div"
                | "dl"
                | "fieldset"
                | "figure"
                | "footer"
                | "form"
                | "header"
                | "h1"
                | "h2"
                | "h3"
                | "h4"
                | "h5"
                | "h6"
                | "hr"
                | "main"
                | "nav"
                | "ol"
                | "p"
                | "pre"
                | "section"
                | "table"
                | "ul"
        ),
        "li" => incoming == "li",
        "dt" => incoming == "dt" || incoming == "dd",
        "dd" => incoming == "dt" || incoming == "dd",
        "tr" => incoming == "tr",
        "td" => incoming == "td" || incoming == "th",
        "th" => incoming == "td" || incoming == "th",
        "thead" => incoming == "tbody" || incoming == "tfoot",
        "tbody" => incoming == "tbody" || incoming == "tfoot",
        "tfoot" => incoming == "tbody",
        "option" => incoming == "option" || incoming == "optgroup",
        "optgroup" => incoming == "optgroup",
        _ => false,
    }
}

fn htmlstring_to_string(str: &HtmlString) -> String {
    return String::from_utf8_lossy(&str).to_string();
}

const VOID_TAGS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source",
    "track", "wbr",
];
const HEAD_TAGS: &[&str] = &[
    "title", "base", "link", "style", "meta", "script", "noscript", "template",
];
pub fn parser(html: &str) -> Result<(Arc<Element>, TokenizerTraversal), String> {
    let mut output = "".to_string();
    let mut stack: Vec<Element> = Vec::new();
    let mut head_read = false;
    let mut global_id_counter = 0;
    let mut root = Element {
        global_id: 0,
        tag: "".to_string(),
        attributes: HashMap::new(),
        children: Vec::new(),
    };
    let mut traversal = TokenizerTraversal { steps: Vec::new() };
    let mut position = 0;
    for token in Tokenizer::new(html).infallible() {
        match token {
            Token::StartTag(tag) => {
                let tag_string = htmlstring_to_string(&tag.name);

                if stack.len() == 0 && tag_string != "html" {
                    let node = Element {
                        global_id: global_id_counter,
                        tag: "html".to_string(),
                        attributes: HashMap::new(),
                        children: Vec::new(),
                    };
                    global_id_counter += 1;
                    stack.push(node);
                }
                if stack.len() == 1 && HEAD_TAGS.contains(&tag_string.as_str()) {
                    let node = Element {
                        global_id: global_id_counter,
                        tag: "head".to_string(),
                        attributes: HashMap::new(),
                        children: Vec::new(),
                    };
                    global_id_counter += 1;
                    head_read = true;
                    stack.push(node);
                } else if stack.len() == 1 && tag_string != "head" && tag_string != "body" {
                    if !head_read {
                        let node = Element {
                            global_id: global_id_counter,
                            tag: "head".to_string(),
                            attributes: HashMap::new(),
                            children: Vec::new(),
                        };
                        global_id_counter += 1;
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(Node::Element(Arc::new(node)));
                        }
                    }
                    let node = Element {
                        global_id: global_id_counter,
                        tag: "body".to_string(),
                        attributes: HashMap::new(),
                        children: Vec::new(),
                    };
                    global_id_counter += 1;
                    stack.push(node);
                }
                if let Some(pos) = stack.iter().position(|x| closes_tag(&x.tag, &tag_string)) {
                    while stack.len() > pos + 1 {
                        let node = stack.pop().unwrap();
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(Node::Element(Arc::new(node)));
                        }
                    }

                    let node = stack.pop().unwrap();
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(Node::Element(Arc::new(node)));
                    } else {
                        // the only element in the tree
                        root = node;
                    }
                }

                if tag.self_closing || VOID_TAGS.contains(&tag_string.as_str()) {

                    traversal.steps.push(TokenizerStep {
                        step_type: "start_tag".to_string(),
                        tag: Some(tag_string.clone()),
                        position,
                    });
                    position += 1;

                    let mut new_node = Element {
                        global_id: global_id_counter,
                        tag: tag_string.clone(),
                        attributes: HashMap::new(),
                        children: Vec::new(),
                    };
                    global_id_counter += 1;
                    for (key, value) in tag.attributes.iter() {
                        new_node
                            .attributes
                            .insert(htmlstring_to_string(key), htmlstring_to_string(value));
                    }

                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(Node::Element(Arc::new(new_node)));
                    }

                    output.push_str(&format!("SelfClosingToken({})\n", tag_string));
                } else {

                    traversal.steps.push(TokenizerStep {
                        step_type: "start_tag".to_string(),
                        tag: Some(tag_string.clone()),
                        position,
                    });
                    position += 1;

                    let mut new_node = Element {
                        global_id: global_id_counter,
                        tag: tag_string.clone(),
                        attributes: HashMap::new(),
                        children: Vec::new(),
                    };
                    global_id_counter += 1;
                    for (key, value) in tag.attributes.iter() {
                        new_node
                            .attributes
                            .insert(htmlstring_to_string(key), htmlstring_to_string(value));
                    }
                    stack.push(new_node);

                    output.push_str(&format!("StartToken({})\n", tag_string));
                }
            }
            Token::EndTag(tag) => {
                let tag_string = htmlstring_to_string(&tag.name);

                traversal.steps.push(TokenizerStep {
                    step_type: "end_tag".to_string(),
                    tag: Some(tag_string.clone()),
                    position,
                });
                position += 1;

                if let Some(pos) = stack.iter().position(|x| &x.tag == &tag_string) {
                    while stack.len() > pos + 1 {
                        let node = stack.pop().unwrap();
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(Node::Element(Arc::new(node)));
                        }
                    }

                    let node = stack.pop().unwrap();
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(Node::Element(Arc::new(node)));
                    } else {
                        // the only element in the tree
                        root = node;
                    }
                } else {
                    continue;
                }

                output.push_str(&format!("EndToken({})\n", tag_string));
            }
            Token::String(s) => {
                let tag_string = htmlstring_to_string(&s);
                let trimmed = tag_string.trim();
                if trimmed.is_empty() {
                    continue;
                }

                traversal.steps.push(TokenizerStep {
                    step_type: "text".to_string(),
                    tag: Some(tag_string.clone()),
                    position,
                });
                position += 1;

                if let Some(parent) = stack.last_mut() {
                    parent
                        .children
                        .push(Node::Text(Arc::new(tag_string.clone())));
                }
                output.push_str(&tag_string);
                output.push('\n');
            }
            Token::Comment(tag) => {}
            Token::Doctype(_) => {}
            other => panic!("unexpected input: {:?}", other),
        }
    }
    // html file stopped with unclosed StartTag
    while stack.len() > 1 {
        let node = stack.pop().unwrap();
        if let Some(parent) = stack.last_mut() {
            parent.children.push(Node::Element(Arc::new(node)));
        }
    }
    if let Some(node) = stack.pop() {
        root = node;
    }

    Ok((Arc::new(root), traversal))
}


