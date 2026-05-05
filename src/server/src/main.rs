use actix_web::{web, App, HttpServer, HttpResponse, http, HttpRequest};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use std::time;
use std::sync::{Arc, OnceLock, Mutex};
use std::collections::{HashMap, HashSet};

mod html;
mod css_selector;
mod traversal;
mod async_util;
mod matching;
mod lca;

use html::{parser as html_parser, Element, Node as TokenizerNode};
use async_util::{get_current_tree};
use traversal::{async_dfs, async_bfs};
use lca::{init_binary_lift_metadata, get_current_binary_lift_metadata, find_lca};

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
    pub search_limit: Option<usize>,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ResponseNode {
    pub tag: String,
    pub class: String,
    pub id: String,
    pub attributes: HashMap<String, String>,
    pub children: Vec<usize>,
    pub index: usize,
}

#[derive(Serialize)]
pub struct HtmlQueryResponse {
    pub nodes: HashMap<usize, ResponseNode>,
    pub root_index: usize,
    pub nodes_count: usize,
    pub depth: usize,
    pub duration: u128,
    pub logs: Vec<String>,
    pub err: String
}

#[derive(Serialize, Deserialize, Clone)]
pub struct ResultItem {
    pub query: String,
    pub paths: Vec<Vec<(usize, usize)>>,
    pub selected: Vec<usize>,
    pub duration: u128,
    pub nodes_count: usize,
    pub logs: Vec<String>,
    pub err: String
}



pub static CURRENT_ID_TO_NODE_MAP: OnceLock<Mutex<HashMap<usize, Arc<Element>>>> = OnceLock::new();

fn get_current_id_to_node_map() -> &'static Mutex<HashMap<usize, Arc<Element>>> {
    CURRENT_ID_TO_NODE_MAP.get_or_init( ||
        Mutex::new(HashMap::new())
    )
}


fn map_id_to_node(element: &Arc<Element>, response_nodes: &mut HashMap<usize, ResponseNode>) {
    let mut children_ids = Vec::new();
    for child in &element.children {
        if let TokenizerNode::Element(child_el) = child {
            children_ids.push(child_el.global_id);
            map_id_to_node(child_el, response_nodes);
        }
    }

    let class = element.attributes.get("class").cloned().unwrap_or_default();
    let id = element.attributes.get("id").cloned().unwrap_or_default();

    let mut map = get_current_id_to_node_map().lock().unwrap();
    map.insert(element.global_id, element.clone());

    response_nodes.insert(element.global_id, ResponseNode {
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


    match html_parser(&html_input) {
        Ok((root, nodes_count, duration)) => {
            let mut tree_mutex = get_current_tree().lock().unwrap();
            *tree_mutex = Arc::clone(&root);

            let depth = init_binary_lift_metadata(&TokenizerNode::Element(root.clone()));

            let mut nodes = HashMap::new();
            map_id_to_node(&root, &mut nodes);

            HttpResponse::Ok().json(HtmlQueryResponse {
                root_index: root.global_id,
                nodes,
                nodes_count,
                depth,
                duration,
                logs: vec![format!("Successfully parsed HTML with {} nodes", nodes_count)],
                err: "".to_string()
            })
        },
        Err(e) => {
            HttpResponse::Ok().json(HtmlQueryResponse {
                root_index: 0,
                nodes: HashMap::new(),
                nodes_count: 0,
                depth: 0,
                duration: 0,
                logs: Vec::new(),
                err: e
            })
        }   
    }

}



async fn process_query_css(body: QueryPostBody) -> HttpResponse {
    let tree = {
        let tree_mutex = get_current_tree().lock().unwrap();
        tree_mutex.clone()
    };
    let threads = body.threads.unwrap_or(1);
    let limit = body.search_limit.unwrap_or(1);
    
    let traversal_result = if body.use_dfs {
        async_dfs(tree, &body.css_query, threads, limit)
    } else {
        async_bfs(tree, &body.css_query, threads, limit)
    };


    let (matched, tracker, nodes_count, duration, traversal_log, err) = match traversal_result {
        Ok((m, t, c, d, l)) => (Some(m), Some(t), Some(c), Some(d), Some(l), None),
        Err(e) => (None, None, None, None, None, Some(e)),
    };

    let matched_nodes = matched.unwrap_or_default();
    let selected: Vec<usize> = matched_nodes.iter().map(|n| n.global_id).collect();
    let selected_count = selected.len();

    let mut logs = if err.is_none() {
        vec![format!("CSS query '{}' found {} matches", body.css_query, selected_count)]
    } else {
        vec![format!("CSS query '{}' failed", body.css_query)]
    };

    logs.append(&mut traversal_log.unwrap_or_default());

    let result = ResultItem {
            query: body.css_query.clone(),
            paths: tracker.unwrap_or_default(),
            selected,
            duration: duration.unwrap_or_default(),
            nodes_count: nodes_count.unwrap_or_default(),
            logs: logs,
            err: err.unwrap_or_default()
        };

    HttpResponse::Ok().json(result)
}



async fn process_query_lca(body: QueryPostBody) -> HttpResponse {
    let node_indices: Vec<usize> = match serde_json::from_str(&body.content) {
        Ok(indices) => indices,
        Err(_) => {
            return HttpResponse::BadRequest().body("Invalid content for LCA. Expected JSON array of integers.");
        }
    };

    let node_map = get_current_id_to_node_map().lock().unwrap();
    let metadata = get_current_binary_lift_metadata().lock().unwrap();

    let node1 = &html::Node::Element(node_map[&node_indices[0]].clone());
    let node2 =  if node_indices.len() > 1 {
        &html::Node::Element(node_map[&node_indices[1]].clone())
    }
    else {
        node1
    };

    let start_time = time::Instant::now();
    let mut logs = vec![format!("Calculated LCA for node indices: {:?}", node_indices)];

    let (mut lca_result, mut lca_path_tracker, mut curr_log) = find_lca(node1, node2, &metadata);
    logs.append(&mut curr_log);

    for i in 2..node_indices.len() {
        let next_node = &html::Node::Element(node_map[&node_indices[i]].clone());
        let (result, mut path_tracker, mut curr_log) = find_lca(&lca_result, next_node, &metadata);
        
        lca_result = result;
        lca_path_tracker[0].append(&mut path_tracker[0]);
        lca_path_tracker[1].append(&mut path_tracker[1]);

        logs.append(&mut curr_log);
    }

    let lca_result_id = match lca_result {
        html::Node::Element(el) => el.global_id,
        html::Node::Text(_) => unreachable!("Invalid LCA result node type"),
    };

    let nodes_count = lca_path_tracker
        .iter().flatten().collect::<HashSet<_>>().len();

    logs.push(format!("Succesfully calculated LCA by visiting {} nodes", nodes_count));

    let result = ResultItem {
        query: format!("LCA for {:?}", node_indices),
        paths: lca_path_tracker,
        selected: vec![lca_result_id],
        duration: start_time.elapsed().as_micros(),
        nodes_count: nodes_count,
        logs: logs,
        err: "".to_string()
    };

    HttpResponse::Ok().json(result)
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
