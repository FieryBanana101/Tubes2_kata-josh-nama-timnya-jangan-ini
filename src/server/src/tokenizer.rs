use std::{collections::HashMap, fs, io, fmt, sync::Arc};
use html5gum::{Tokenizer, Token, HtmlString};
use reqwest;

#[derive(Debug, Clone, PartialEq)]
pub enum Node {
    Element(Arc<Element>),
    Text(Arc<String>)
}

#[derive(Debug, Clone, PartialEq)]
pub struct Element {
    pub tag: String,
    pub attributes: HashMap<String, String>,
    pub children: Vec<Node>
}

// DEBUGDEBUGDEBUGDEBUGDEBUGDEBUGDEBUGDEBUGDEBUG
// impl fmt::Display for Node {
//     fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
//         self.fmt_indent(f, 0)
//     }
// }

// impl Node {
//     fn fmt_indent(&self, f: &mut fmt::Formatter<'_>, depth: usize) -> fmt::Result {
//         let indent = "  ".repeat(depth);
//         match self {
//             Node::Element(el) => {
//                 write!(f, "{}<{}", indent, el.tag)?;
//                 for (k, v) in &el.attributes {
//                     write!(f, " {}=\"{}\"", k, v)?;
//                 }
//                 writeln!(f, ">")?;
//                 for child in &el.children {
//                     child.fmt_indent(f, depth + 1)?;
//                 }
//                 writeln!(f, "{}</{}>", indent, el.tag)
//             }
//             Node::Text(s) => writeln!(f, "{}\"{}\"", indent, s),
//         }
//     }
// }
// DEBUGDEBUGDEBUGDEBUGDEBUGDEBUGDEBUGDEBUGDEBUG

// pub async fn get_html(url: String) -> Result<String, reqwest::Error> {
//     let res = reqwest::get(url).await?;
    
//     if let Some(ct) = res.headers().get("content-type") {
//         println!("{:?}", ct);
//     }

//     let body = res.text().await?;
//     Ok(body)
// }

fn bounded(current: &str, incoming: &str) -> bool{
    match incoming {
        "li" => matches!(current, "ul" | "ol" | "menu" | "div" | "section" | "article" | "nav" | "aside"),
        "dt" | "dd" => current == "dl",
        "option" => current == "optgroup" || current == "select",
        "td" | "th" => current == "tr",
        _ => false
    }
}

fn closes_tag(current: &str, incoming: &str) -> bool{
    match current {
        "p" => matches!(incoming, 
            "address" | "article" | "aside" | "blockquote" | "div" | "dl" | 
            "fieldset" | "figure" | "footer" | "form" | "header" |
            "h1" |  "h2" | "h3" | "h4" | "h5" | "h6" | "hr" | "main" |
            "nav" | "ol" | "p" | "pre" | "section" | "table" | "ul" ),
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
        _ => false
    }
}

fn htmlstring_to_string(str: &HtmlString) -> String{
    return String::from_utf8_lossy(&str).to_string();
}

const VOID_TAGS: &[&str] = &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];
const HEAD_TAGS: &[&str] = &["title", "base", "link", "style", "meta", "script", "noscript", "template"];
pub fn parser(html: &str) -> Result<Arc<Element>, String> {

    let mut output = "".to_string();
    let mut stack: Vec<Element> = Vec::new();
    let mut head_read = false;
    let mut root = Element {
        tag: "".to_string(),
        attributes: HashMap::new(),
        children: Vec::new()
    };
    for token in Tokenizer::new(html).infallible() {
        match token {
            Token::StartTag(tag) => {
                let tag_string = htmlstring_to_string(&tag.name);

                if stack.len() == 0 && tag_string != "html" {
                    let mut node = Element {
                        tag: "html".to_string(),
                        attributes: HashMap::new(),
                        children: Vec::new()
                    };
                    stack.push(node);
                }
                if stack.len() == 1 && HEAD_TAGS.contains(&tag_string.as_str()){
                    let mut node = Element {
                        tag: "head".to_string(),
                        attributes: HashMap::new(),
                        children: Vec::new()
                    };
                    head_read = true;
                    stack.push(node);
                } 
                else if stack.len() == 1 && tag_string != "head" && tag_string != "body" {
                    if !head_read {
                        let mut node = Element {
                            tag: "head".to_string(),
                            attributes: HashMap::new(),
                            children: Vec::new()
                        };
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(Node::Element(Arc::new(node)));
                        }
                    }
                    let mut node = Element {
                        tag: "body".to_string(),
                        attributes: HashMap::new(),
                        children: Vec::new()
                    };
                    stack.push(node);
                }
                if let Some(pos) = stack.iter().rposition(|x| closes_tag(&x.tag, &tag_string)){
                    if !stack[pos + 1..].iter().any(|x| bounded(&x.tag, &tag_string)){
                        while stack.len() > pos + 1 {
                            let node = stack.pop().unwrap();
                            if let Some(parent) = stack.last_mut() {
                                parent.children.push(Node::Element(Arc::new(node)));
                            }
                        }
                        let node = stack.pop().unwrap();
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(Node::Element(Arc::new(node)));
                        }
                        else {
                            // the only element in the tree
                            root = node;
                        }   
                    }
                }

                if tag.self_closing || VOID_TAGS.contains(&tag_string.as_str()) {
                    println!("SelfClosingToken({})", &tag_string);

                    let mut new_node = Element { 
                        tag: tag_string.clone(), 
                        attributes: HashMap::new(), 
                        children: Vec::new() 
                    };
                    for (key, value) in tag.attributes.iter() {
                        new_node.attributes.insert(htmlstring_to_string(key), htmlstring_to_string(value));
                    }
                    
                    if let Some(parent) = stack.last_mut(){
                        parent.children.push(Node::Element(Arc::new(new_node)));
                    }

                    output.push_str(&format!("SelfClosingToken({})\n", tag_string));
                } else {
                    println!("StartToken({})", &tag_string);

                    let mut new_node = Element { 
                        tag: tag_string.clone(),
                        attributes: HashMap::new(), 
                        children: Vec::new() 
                    };
                    for (key, value) in tag.attributes.iter() {
                        new_node.attributes.insert(htmlstring_to_string(key), htmlstring_to_string(value));
                    }
                    stack.push(new_node);

                    output.push_str(&format!("StartToken({})\n", tag_string));
                }
            }
            Token::EndTag(tag) => {
                let tag_string = htmlstring_to_string(&tag.name);
                println!("EndToken({})", &tag_string);

                if let Some(pos) = stack.iter().rposition(|x| &x.tag == &tag_string){
                    if !stack[pos + 1..].iter().any(|x| bounded(&x.tag, &tag_string)){
                        while stack.len() > pos + 1 {
                            let node = stack.pop().unwrap();
                            if let Some(parent) = stack.last_mut() {
                                parent.children.push(Node::Element(Arc::new(node)));
                            }
                        }
                        let node = stack.pop().unwrap();
                        if let Some(parent) = stack.last_mut() {
                            parent.children.push(Node::Element(Arc::new(node)));
                        }
                        else {
                            // the only element in the tree
                            root = node;
                        }   
                    }
                } else {
                    continue;
                }

                output.push_str(&format!("EndToken({})\n", tag_string));
            }
            Token::String(s) => {
                let tag_string = htmlstring_to_string(&s);
                let trimmed = tag_string.trim();
                if trimmed.is_empty() { continue; }
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(Node::Text(Arc::new(tag_string.clone())));
                }
                println!(); 
                    output.push_str(&tag_string); output.push('\n');
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
    fs::write("tokenized.txt", output).ok();
    Ok(Arc::new(root))
}

// #[tokio::main]
// pub async fn main(){
//     let mut input = String::new();
//     io::stdin().read_line(&mut input).expect("Input failed\n");
//     let input = input.trim().to_string();
//     let html = get_html(input).await;

//     match html {
//         Ok(body) => {
//             fs::write("html.txt", &body).ok();
//             let root = parser(&body);
//             match root {
//                 Ok(root) => {
//                     let tree = Node::Element(root);
//                     // fs::write("tree.txt", tree.to_string()).ok();
//                 }
//                 Err(e) => println!("Unable to parse tree: {}", e)
//             }
//         }
//         Err(e) => println!("Unable to fetch HTML: {:?}\n", e)
//     }
// }