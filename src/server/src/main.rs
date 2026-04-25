use actix_web::{web, App, HttpServer, HttpResponse, http};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;

mod tokenizer;
mod css_selector;
mod traversal;
mod async_util;
mod matching;
mod lca;

use tokenizer::{parser as tokenizer_parse, Element, Node as TokenizerNode};
use crate::traversal::{get_traversal_result, TraversalResult};

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

fn build_node_index_map(element: &Arc<Element>) -> HashMap<usize, i32> {
    let mut map = HashMap::new();
    let mut counter: i32 = 0;
    
    fn traverse(elem: &Arc<Element>, map: &mut HashMap<usize, i32>, counter: &mut i32) {
        let ptr = Arc::as_ptr(elem) as usize;
        map.insert(ptr, *counter);
        *counter += 1;
        
        for child in &elem.children {
            if let TokenizerNode::Element(c) = child {
                traverse(c, map, counter);
            }
        }
    }
    
    traverse(element, &mut map, &mut counter);
    map
}

fn convert_traversal_result(
    traversal_result: TraversalResult,
    flat_nodes: &[QueryResponseNode],
    node_index_map: &HashMap<usize, i32>,
    query_parts: &[String],
    root_idx: i32,
) -> (Vec<ResultItem>, Vec<i32>) {
    let mut results = Vec::new();
    let mut all_selected = Vec::new();
    
    let matched_ptrs: Vec<usize> = traversal_result.matched_elements.iter()
        .map(|e| Arc::as_ptr(e) as usize)
        .collect();
    
    let selected: Vec<i32> = matched_ptrs.iter()
        .filter_map(|ptr| node_index_map.get(ptr).copied())
        .collect();
    
    let paths: Vec<Vec<i32>> = selected.iter()
        .map(|&s| vec![root_idx, s])
        .collect();
    
    let traversal_path: Vec<i32> = traversal_result.traversal_order.iter()
        .map(|e| {
            let ptr = Arc::as_ptr(e) as usize;
            node_index_map.get(&ptr).copied().unwrap_or(0)
        })
        .collect();
    
    for (i, query_text) in query_parts.iter().enumerate() {
        results.push(ResultItem {
            query: query_text.clone(),
            paths: paths.clone(),
            selected: selected.clone(),
            traversal_path: traversal_path.clone(),
        });
    }
    
    all_selected.extend(selected);
    
    (results, all_selected)
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

    let (root, _) = tokenizer_parse(&html_input).unwrap_or((
        Arc::new(Element { 
            tag: "".to_string(), 
            attributes: HashMap::new(), 
            children: vec![] 
        }), 
        tokenizer::TokenizerTraversal { steps: vec![] }
    ));
    
    let flat_nodes = build_flat_nodes(&root);
    let root_idx = 0;
    let node_index_map = build_node_index_map(&root);
    
    let traversal_result = get_traversal_result(&html_input, &body.css_query, body.use_dfs);
    
    let query_parts: Vec<String> = body.css_query.split(',')
        .map(|s| s.trim().to_string())
        .collect();
    
    let (results, all_selected) = if let Some(result) = traversal_result {
        convert_traversal_result(result, &flat_nodes, &node_index_map, &query_parts, root_idx)
    } else {
        (Vec::new(), Vec::new())
    };
    
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