use clap::Parser;
use askama::Template;
use axum::{
    body::StreamBody,
    http::StatusCode,
    response::{IntoResponse, Html, Redirect},
    Router,
    routing::{get, post}, 
    extract::{
        State,
        Path
    }
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use std::{collections::{HashMap, HashSet}, path::PathBuf, sync::{atomic::{AtomicUsize, Ordering}, Arc, Mutex}, net::SocketAddr};
use tracing::{log::error, info};
use tokio_util::io::ReaderStream;

#[derive(Parser, Debug, Clone)]
pub struct VideoPlayerConfig {
    #[clap(short, long, default_value = "assets")]
    pub assets_root: String,

    #[clap(short, long, default_value = "9092")]
    pub port: u16,

    #[clap(short, long, default_value = "0.0.0.0")]
    pub host: String,
}

#[derive(Default)]
pub struct VideoPlayerState {
    pub videos: HashMap<String, String>,
    video_extensions: HashSet<String>,
    next_index: AtomicUsize,
    root: Option<String>
}

pub type SharedState = Arc<Mutex<VideoPlayerState>>;

impl VideoPlayerState {
    pub fn new() -> Self {
        Self {
            video_extensions: HashSet::from_iter([
                "mp4",
                "av1",
                "avi",
                "flv",
                "heic",
                "mkv",
            ].map(String::from)),
            ..Default::default()
        }
    }

    fn advance_index(&mut self) {
        self.next_index.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    fn is_video_file<P: AsRef<std::path::Path>>(&self, path: P) -> bool {
        if let Some(extension) = path.as_ref().extension() {
            if self.video_extensions.contains(extension.to_str().unwrap()) {
                return true;
            }
        }
        false
    }

    pub fn load_videos<P: AsRef<std::path::Path>>(&mut self, root: P) -> std::io::Result<()> {
        self.visit_dirs(root)
    }

    pub fn load_video(&mut self, path: PathBuf) {
        let stored_file_name = path.to_str().unwrap().to_string();
        let extension = path.extension().unwrap();
        let server_path = format!("{}.{}", self.next_index.load(Ordering::SeqCst), extension.to_str().unwrap());
        info!("Loading video: {} as {}", stored_file_name, server_path);
        self.advance_index();
        self.videos.insert(server_path, stored_file_name);
    }

    fn visit_dirs<P: AsRef<std::path::Path>>(&mut self, root: P) -> std::io::Result<()> {
        if root.as_ref().is_dir() {
            if let Ok(dir) = std::fs::read_dir(root.as_ref()) {
                for entry in dir {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_dir() {
                        self.visit_dirs(path)?;
                    } else if self.is_video_file(path.as_path()) {
                        self.load_video(path);
                    }
                }
            }
        }
        Ok(())
    }

    pub fn build(config: &VideoPlayerConfig) -> Self {
        let mut state = Self::new();
        state.root = Some(config.assets_root.clone());
        state.load_videos(state.root.clone().unwrap()).unwrap();
        state
    }

    pub fn reload(&mut self) {
        self.next_index = AtomicUsize::new(0);
        self.videos.clear();
        self.load_videos(self.root.clone().unwrap()).unwrap();
    }
}


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


pub async fn index(
    State(state): State<SharedState>
) -> impl IntoResponse {
    let template = IndexTemplate {
        videos: state.lock().unwrap().videos.clone(),
    };
    HtmlTemplate(template)
}

pub async fn script() -> impl IntoResponse {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(axum::http::header::CONTENT_TYPE, "application/javascript".parse().unwrap());
    (headers, include_str!("../assets/index.js"))
}


pub async fn css() -> impl IntoResponse {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(axum::http::header::CONTENT_TYPE, "text/css".parse().unwrap());
    (headers, include_str!("../assets/index.css"))
}

pub async fn favicon() -> impl IntoResponse {
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(axum::http::header::CONTENT_TYPE, "image/x-icon".parse().unwrap());
    (headers, include_bytes!("../assets/favicon.ico").to_vec())
}


pub async fn reload(
    State(state): State<SharedState>
) -> impl IntoResponse {
    state.lock().unwrap().reload();
    Redirect::to("/")
}

#[axum_macros::debug_handler]
pub async fn video_handler(
    Path(video_id): Path<String>, 
    State(state): State<SharedState>,
) -> impl IntoResponse {
    let file_path = state.lock().unwrap().videos.get(&video_id).unwrap_or_else(|| panic!("Failed to find video with given id: {}", video_id.clone())).clone();

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
            format!("video/{}", extension).parse().unwrap()
        );
        (headers, body)
    }).map_err(|err| {
        error!("Failed to open file: \nError: {}", err);
        (StatusCode::INTERNAL_SERVER_ERROR, "Failed to open file")
    })
}

pub fn set_up_logging() {
    tracing_subscriber::registry()
    .with(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "video-player=debug,tower_http=debug".into()))
    .with(tracing_subscriber::fmt::layer())
    .init();
}

#[tokio::main]
pub async fn main() {
    set_up_logging();
    let config = VideoPlayerConfig::parse();
    let state = Arc::new(Mutex::new(VideoPlayerState::build(&config)));

    let app = 
        Router::new()
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