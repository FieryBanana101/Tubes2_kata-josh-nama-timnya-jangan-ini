use actix_web::{web, App, HttpServer, HttpResponse, http, HttpRequest};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;

mod html;
mod css_selector;
mod traversal;
mod async_util;
mod matching;
mod lca;

use html::{parser as html_parser, Element, Node as TokenizerNode};
use async_util::get_current_tree;
use traversal::{async_dfs, async_bfs};

#[derive(Serialize, Deserialize, Debug)]
pub struct QueryPostBody {
    pub query_type: String,
    #[serde(default)]
    pub input_type: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub css_query: String,
    #[serde(default)]
    pub use_dfs: bool,
    pub threads: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FlattenedNode {
    pub tag: String,
    pub class: String,
    pub id: String,
    pub attributes: HashMap<String, String>,
    pub children: Vec<usize>,
    pub index: usize,
}

#[derive(Serialize)]
pub struct HtmlQueryResponse {
    pub nodes: HashMap<usize, FlattenedNode>,
    pub root_index: usize,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ResultItem {
    pub query: String,
    pub paths: Vec<Vec<usize>>,
    pub selected: Vec<usize>,
}

#[derive(Serialize)]
pub struct CSSQueryResponse {
    pub results: Vec<ResultItem>,
}

fn flatten_tree(element: &Arc<Element>, nodes: &mut HashMap<usize, FlattenedNode>) {
    let mut children_ids = Vec::new();
    for child in &element.children {
        if let TokenizerNode::Element(child_el) = child {
            children_ids.push(child_el.global_id);
            flatten_tree(child_el, nodes);
        }
    }

    let class = element.attributes.get("class").cloned().unwrap_or_default();
    let id = element.attributes.get("id").cloned().unwrap_or_default();

    nodes.insert(element.global_id, FlattenedNode {
        tag: element.tag.clone(),
        class,
        id,
        attributes: element.attributes.clone(),
        children: children_ids,
        index: element.global_id,
    });
}

async fn process_query_html(body: QueryPostBody) -> HttpResponse {
    let html_input = match body.input_type.as_str() {
        "file" | "plain_text" => body.content,
        "url" => {
            let client = reqwest::ClientBuilder::new()
                .danger_accept_invalid_certs(true)
                .user_agent("Mozilla/5.0")
                .build()
                .unwrap();
            
            match client.get(&body.content).send().await {
                Ok(resp) => resp.text().await.unwrap_or_default(),
                Err(e) => {
                    eprintln!("Network/Request Error: {:?}", e);
                    return HttpResponse::InternalServerError().body(format!("Failed to fetch URL: {}", e));
                }
            }
        }
        _ => return HttpResponse::BadRequest().body("Invalid input_type. Expected 'file', 'plain_text', or 'url'."),
    };

    if let Ok((root, _)) = html_parser(&html_input) {
        let mut tree_mutex = get_current_tree().lock().unwrap();
        *tree_mutex = Arc::clone(&root);

        let mut nodes = HashMap::new();
        flatten_tree(&root, &mut nodes);

        HttpResponse::Ok().json(HtmlQueryResponse {
            root_index: root.global_id,
            nodes,
        })
    } else {
        HttpResponse::InternalServerError().body("Failed to parse HTML")
    }
}

async fn process_query_css(body: QueryPostBody) -> HttpResponse {
    let tree = {
        let tree_mutex = get_current_tree().lock().unwrap();
        tree_mutex.clone()
    };
    let threads = body.threads.unwrap_or(1);
    
    let (matched, tracker) = if body.use_dfs {
        async_dfs(tree, &body.css_query, threads)
    } else {
        async_bfs(tree, &body.css_query, threads)
    };

    let selected: Vec<usize> = matched.unwrap_or_default().iter().map(|n| n.global_id).collect();
    let paths = tracker.unwrap_or_default();

    let result = ResultItem {
        query: body.css_query.clone(),
        paths,
        selected,
    };

    HttpResponse::Ok().json(CSSQueryResponse {
        results: vec![result],
    })
}

async fn process_query_lca(_body: QueryPostBody) -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({}))
}

async fn query(body: web::Json<QueryPostBody>) -> HttpResponse {
    let body = body.into_inner();
    println!("Received query: {:?}", body.query_type);
    match body.query_type.as_str() {
        "html" => process_query_html(body).await,
        "css" => process_query_css(body).await,
        "lca" => process_query_lca(body).await,
        _ => HttpResponse::BadRequest().body("Invalid query_type"),
    }
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("Starting server on 0.0.0.0:8081");
    HttpServer::new(|| {
        let cors = Cors::permissive();

        App::new()
            .wrap(cors)
            .service(web::resource("/query").route(web::post().to(query)))
    })
    .bind(("0.0.0.0", 8081))?
    .run()
    .await
}
