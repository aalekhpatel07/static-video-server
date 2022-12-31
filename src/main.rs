use askama::Template;
use axum::{
    body::{BoxBody, boxed, Body},
    extract::{Path, State},
    http::{StatusCode, Request, Response},
    response::{Html, IntoResponse, Redirect},
    routing::{get, post, get_service},
    Router,
};
use clap::Parser;
use tower::ServiceExt;
use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc, Mutex,
    },
};
use tracing::{info, log::error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use static_video_server::*;
use tower_http::{services::ServeDir, trace::TraceLayer};


struct HtmlTemplate<T>(T);

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub videos: HashMap<String, PathBuf>,
}

impl<T> IntoResponse for HtmlTemplate<T>
where
    T: Template,
{
    fn into_response(self) -> axum::response::Response {
        match self.0.render() {
            Ok(html) => Html(html).into_response(),
            Err(err) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("Failed to render template. Error: {}", err),
            )
                .into_response(),
        }
    }
}

pub async fn index(State(state): State<SharedState>) -> impl IntoResponse {
    let template = IndexTemplate {
        videos: state.lock().unwrap().videos.clone().into_iter().map(|(k, v)| {
            (k, PathBuf::from(v))
        }).collect(),
    };
    HtmlTemplate(template)
}

pub async fn favicon() -> impl IntoResponse {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        "image/x-icon".parse().unwrap(),
    );
    (headers, include_bytes!("../assets/favicon.ico").to_vec())
}

pub async fn reload(State(state): State<SharedState>) -> impl IntoResponse {
    state.lock().unwrap().reload();
    Redirect::to("/")
}

pub async fn get_static_file(path: PathBuf) -> Result<Response<BoxBody>, (StatusCode, String)> {

    let request = Request::builder().body(Body::empty()).unwrap();

    match ServeDir::new(path.clone()).oneshot(request).await {
        Ok(response) => Ok(response.map(boxed)),
        Err(err) => {
            error!("Failed to open file: \nError: {}", err);
            Err((StatusCode::INTERNAL_SERVER_ERROR, "Failed to open file".to_string()))
        }
    }
}

pub fn static_file_router() -> Router {
    let serve_dir = get_service(ServeDir::new("assets")).handle_error(handle_error);
    Router::new()
        .route("/", serve_dir.clone())
        .fallback_service(serve_dir)
}

async fn handle_error(_err: std::io::Error) -> impl IntoResponse {
    (StatusCode::INTERNAL_SERVER_ERROR, "Something went wrong...")
}

#[axum_macros::debug_handler]
pub async fn video_handler(
    Path(video_id): Path<String>,
    State(state): State<SharedState>,
) -> impl IntoResponse {
    let file_path = state
        .lock()
        .unwrap()
        .videos
        .get(&video_id)
        .unwrap_or_else(|| panic!("Failed to find video with given id: {}", video_id.clone()))
        .clone();

    drop(state);

    get_static_file(PathBuf::from(&file_path)).await
}

pub fn set_up_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "static-video-server=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

#[tokio::main]
pub async fn main() {
    set_up_logging();
    let config = VideoPlayerConfig::parse();
    let state = Arc::new(Mutex::new(VideoPlayerState::build(&config)));

    let app = Router::new()
        .nest_service("/assets/", static_file_router())
        .route("/favicon.ico", get(favicon))
        .route("/video/:video_id", get(video_handler))
        .route("/", get(index))
        .route("/reload", post(reload))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let host_port = format!("{}:{}", config.host, config.port);
    let addr = host_port.parse::<SocketAddr>().unwrap();
    info!("Starting server on {}", host_port);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
