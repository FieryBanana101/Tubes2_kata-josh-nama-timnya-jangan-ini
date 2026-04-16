use actix_web::{web, App, HttpServer, HttpResponse, http};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};

#[derive(Serialize,Deserialize)]
struct QueryPostBody {
    input_type: String,
    css_query: String,
    file_payload: String,
    url_payload: String,
    text_payload: String,   
    use_dfs: bool,
}

#[derive(Serialize,Deserialize)]
struct QueryResponseNode {
    tag: String,
    class: String,
    id: String,
    attributes: Vec<(String, String)>,
    children: Vec<u32>,
}

#[derive(Serialize,Deserialize)]
struct ResultItem {
    query: String,
    paths: Vec<Vec<u32>>,
    selected: Vec<u32>,
}

#[derive(Serialize,Deserialize)]
struct QueryResponse {
    root_index: u32,
    nodes: Vec<QueryResponseNode>,
    results: Vec<ResultItem>,
    selected_nodes: Vec<u32>,
}

async fn query(body: web::Json<QueryPostBody>) -> HttpResponse {
    let res = QueryResponse {
        root_index: 0,
        nodes: vec![
            QueryResponseNode {
                tag: "html".to_string(),
                class: "".to_string(),
                id: "".to_string(),
                attributes: vec![],
                children: vec![1],
            },
            QueryResponseNode {
                tag: "body".to_string(),
                class: "page-body".to_string(),
                id: "".to_string(),
                attributes: vec![],
                children: vec![2, 3, 4],
            },
            QueryResponseNode {
                tag: "div".to_string(),
                class: "meow".to_string(),
                id: "meow1".to_string(),
                attributes: vec![],
                children: vec![],
            },
            QueryResponseNode {
                tag: "div".to_string(),
                class: "meow meoww".to_string(),
                id: "meow2".to_string(),
                attributes: vec![],
                children: vec![],
            },
            QueryResponseNode {
                tag: "div".to_string(),
                class: "meow meooow".to_string(),
                id: "meow3".to_string(),
                attributes: vec![],
                children: vec![],
            },
        ],
        results: vec![
            ResultItem {
                query: "body".to_string(),
                paths: vec![vec![0, 1]],
                selected: vec![1],
            },
            ResultItem {
                query: ".meow".to_string(),
                paths: vec![vec![1, 2], vec![1, 3], vec![1, 4]],
                selected: vec![2, 3, 4],
            },
        ],
        selected_nodes: vec![2, 3, 4],
    };

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