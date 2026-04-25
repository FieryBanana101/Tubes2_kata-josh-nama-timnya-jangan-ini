use actix_web::{web, App, HttpServer, HttpResponse, http};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

mod tokenizer;
mod css_selector;
mod traversal;
mod async_util;
mod matching;
mod lca;

use tokenizer::{parser as tokenizer_parse, Element, Node as TokenizerNode};
use css_selector::{CssSelectorParser, NodeFilter, Combinator, SelectorUnit};

#[derive(Serialize, Deserialize)]
pub struct QueryPostBody {
    input_type: String,
    css_query: String,
    file_payload: String,
    url_payload: String,
    text_payload: String,
    pub use_dfs: bool,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct QueryResponseNode {
    pub tag: String,
    pub class: String,
    pub id: String,
    pub attributes: Vec<(String, String)>,
    pub children: Vec<i32>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ResultItem {
    pub query: String,
    pub paths: Vec<Vec<i32>>,
    pub selected: Vec<i32>,
    pub traversal_path: Vec<i32>,
}

#[derive(Serialize, Deserialize)]
pub struct QueryResponse {
    pub root_index: i32,
    pub nodes: Vec<QueryResponseNode>,
    pub results: Vec<ResultItem>,
    pub selected_nodes: Vec<i32>,
}

fn build_flat_nodes(element: &Arc<Element>) -> Vec<QueryResponseNode> {
    let mut node_list: Vec<(Arc<Element>, i32)> = Vec::new();
    
    fn traverse(elem: &Arc<Element>, parent_idx: i32, list: &mut Vec<(Arc<Element>, i32)>) {
        let my_idx = list.len() as i32;
        list.push((Arc::clone(elem), parent_idx));
        
        for child in &elem.children {
            if let TokenizerNode::Element(c) = child {
                traverse(c, my_idx, list);
            }
        }
    }
    traverse(element, -1, &mut node_list);
    
    node_list.iter().enumerate().map(|(i, (elem, _))| {
        let class = elem.attributes.get("class").cloned().unwrap_or_default();
        let id = elem.attributes.get("id").cloned().unwrap_or_default();
        let attrs: Vec<(String, String)> = elem.attributes.iter()
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        
        let children: Vec<i32> = node_list.iter()
            .enumerate()
            .filter(|(_, (_, p))| *p == i as i32)
            .map(|(idx, _)| idx as i32)
            .collect();
        
        QueryResponseNode {
            tag: elem.tag.clone(),
            class,
            id,
            attributes: attrs,
            children,
        }
    }).collect()
}

fn matches_filter(node: &QueryResponseNode, filter: &SelectorUnit) -> bool {
    if let Some(ref tag) = filter.tag {
        if node.tag != *tag { return false; }
    }
    if let Some(ref ids) = filter.ids {
        for id in ids {
            if node.id != *id { return false; }
        }
    }
    if let Some(ref classes) = filter.classes {
        let node_classes: Vec<&str> = node.class.split_whitespace().collect();
        for class in classes {
            if !node_classes.contains(&class.as_str()) { return false; }
        }
    }
    true
}

fn process_selector(nodes: &[QueryResponseNode], filter: &SelectorUnit, root_idx: i32) -> (Vec<Vec<i32>>, Vec<i32>, Vec<i32>) {
    let mut out_paths: Vec<Vec<i32>> = Vec::new();
    let mut out_selected = Vec::new();
    let mut out_traversal = Vec::new();
    let mut curr_path: Vec<i32> = Vec::new();
    
    fn inner_dfs(nd: &[QueryResponseNode], ci: i32, ft: &SelectorUnit, cp: &mut Vec<i32>, ops: &mut Vec<Vec<i32>>, os: &mut Vec<i32>, ot: &mut Vec<i32>) {
        ot.push(ci);
        cp.push(ci);
        
        if matches_filter(&nd[ci as usize], ft) {
            ops.push(cp.clone());
            os.push(ci);
        }
        
        for &kid in &nd[ci as usize].children {
            inner_dfs(nd, kid, ft, cp, ops, os, ot);
        }
        
        ot.push(ci);
        cp.pop();
    }
    
    inner_dfs(nodes, root_idx, filter, &mut curr_path, &mut out_paths, &mut out_selected, &mut out_traversal);
    (out_paths, out_selected, out_traversal)
}

fn process_selector_with_combinator(nodes: &[QueryResponseNode], filter: &SelectorUnit, root_idx: i32, combinator: &Option<Combinator>) -> (Vec<Vec<i32>>, Vec<i32>, Vec<i32>) {
    match combinator {
        Some(Combinator::Child) | Some(Combinator::DirectNextSibling) => {
            let mut paths = Vec::new();
            let mut selected = Vec::new();
            let mut traversal_path = Vec::new();
            
            traversal_path.push(root_idx);
            
            let start = &nodes[root_idx as usize];
            for &c in &start.children {
                traversal_path.push(c);
                if matches_filter(&nodes[c as usize], filter) {
                    paths.push(vec![root_idx, c]);
                    selected.push(c);
                }
                traversal_path.push(c);
            }
            
            traversal_path.push(root_idx);
            (paths, selected, traversal_path)
        }
        _ => process_selector(nodes, filter, root_idx)
    }
}

async fn process_query(body: web::Json<QueryPostBody>) -> QueryResponse {
    let client = reqwest::ClientBuilder::new()
        .danger_accept_invalid_certs(true)
        .user_agent("Mozilla/5.0")
        .build()
        .unwrap();
    
    let html_input = if !body.file_payload.is_empty() {
        body.file_payload.clone()
    } else if !body.url_payload.is_empty() {
        match client.get(&body.url_payload).send().await {
            Ok(resp) => resp.text().await.unwrap_or_default(),
            Err(_) => String::new(),
        }
    } else {
        body.text_payload.clone()
    };

    let (root, _) = tokenizer_parse(&html_input).unwrap_or((Arc::new(Element { tag: "".to_string(), attributes: std::collections::HashMap::new(), children: vec![] }), tokenizer::TokenizerTraversal { steps: vec![] }));
    let flat_nodes = build_flat_nodes(&root);
    let root_idx = 0;
    
    let mut parser = CssSelectorParser::new(&body.css_query, false);
    let mut selector_units = Vec::new();
    let mut query_parts = Vec::new();
    
    loop {
        match parser.advance() {
            Ok((unit, is_eof)) => {
                let mut parts = Vec::new();
                if let Some(ref tag) = unit.selector.tag { parts.push(tag.clone()); }
                if let Some(ref classes) = unit.selector.classes { for c in classes { parts.push(format!(".{}", c)); } }
                if let Some(ref ids) = unit.selector.ids { for id in ids { parts.push(format!("#{}", id)); } }
                query_parts.push(parts.join(""));
                selector_units.push(unit);
                if is_eof { break; }
            }
            Err(_) => break,
        }
    }
    
    let mut results = Vec::new();
    let mut all_selected = Vec::new();
    let mut current_root = root_idx;
    
    for (i, unit) in selector_units.iter().enumerate() {
        let query_text = query_parts.get(i).cloned().unwrap_or_default();
        let (paths, selected, traversal_path) = process_selector_with_combinator(&flat_nodes, &unit.selector, current_root, &unit.prev_combinator);
        
        results.push(ResultItem {
            query: query_text,
            paths: paths.clone(),
            selected: selected.clone(),
            traversal_path,
        });
        
        all_selected.extend(selected);
        
        if let Some(last_match) = paths.last() {
            current_root = *last_match.last().unwrap_or(&current_root);
        }
    }
    
    QueryResponse {
        root_index: root_idx,
        nodes: flat_nodes,
        results,
        selected_nodes: all_selected,
    }
}

async fn query(body: web::Json<QueryPostBody>) -> HttpResponse {
    let res = process_query(body).await;
    HttpResponse::Ok().json(res)
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    HttpServer::new(|| {
        let cors = Cors::default()
            .allowed_origin("http://localhost:8080")
            .allowed_methods(vec!["GET", "POST"])
            .allowed_headers(vec![http::header::ACCEPT])
            .allowed_header(http::header::CONTENT_TYPE)
            .max_age(3600);

        App::new()
            .wrap(cors)
            .service(web::resource("/query").route(web::post().to(query)))
    })
    .bind(("127.0.0.1", 8081))?
    .run()
    .await
}