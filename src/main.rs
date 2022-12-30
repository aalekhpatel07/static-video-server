use askama::Template;
use axum::{
    body::StreamBody,
    extract::{Path, State},
    http::StatusCode,
    response::{Html, IntoResponse, Redirect},
    routing::{get, post},
    Router,
};
use clap::Parser;
use std::{
    collections::HashMap,
    net::SocketAddr,
    path::PathBuf,
    sync::{
        Arc, Mutex,
    },
};
use tokio_util::io::ReaderStream;
use tracing::{info, log::error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use static_video_server::*;


struct HtmlTemplate<T>(T);

#[derive(Template)]
#[template(path = "index.html")]
pub struct IndexTemplate {
    pub videos: HashMap<String, String>,
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
        videos: state.lock().unwrap().videos.clone(),
    };
    HtmlTemplate(template)
}

pub async fn script() -> impl IntoResponse {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        "application/javascript".parse().unwrap(),
    );
    (headers, include_str!("../assets/index.js"))
}

pub async fn css() -> impl IntoResponse {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::CONTENT_TYPE,
        "text/css".parse().unwrap(),
    );
    (headers, include_str!("../assets/index.css"))
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

    let path = PathBuf::from(file_path);
    let extension = path.extension().unwrap().to_str().unwrap();
    drop(state);

    tokio::fs::File::open(path.clone())
        .await
        .map(|file| {
            let stream = ReaderStream::new(file);
            let body = StreamBody::new(stream);
            let mut headers = axum::http::HeaderMap::new();

            headers.insert(
                axum::http::header::CONTENT_TYPE,
                format!("video/{}", extension).parse().unwrap(),
            );
            (headers, body)
        })
        .map_err(|err| {
            error!("Failed to open file: \nError: {}", err);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to open file")
        })
}

pub fn set_up_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "static-video-server=info".into()),
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
        .route("/:video_id", get(video_handler))
        .route("/assets/index.js", get(script))
        .route("/assets/index.css", get(css))
        .route("/favicon.ico", get(favicon))
        .route("/", get(index))
        .route("/reload", post(reload))
        .with_state(state);

    let host_port = format!("{}:{}", config.host, config.port);
    let addr = host_port.parse::<SocketAddr>().unwrap();
    info!("Starting server on {}", host_port);
    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}
