use clap::Parser;
use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tracing::log::info;


/// The configuration for the video server.
#[derive(Parser, Debug, Clone)]
pub struct VideoPlayerConfig {
    #[clap(short, long, default_value = "assets")]
    pub assets_root: String,

    #[clap(short, long, default_value = "9092")]
    pub port: u16,

    #[clap(short, long, default_value = "0.0.0.0")]
    pub host: String,
}

/// The video index state that is shared between all requests.
/// Store a list of videos and their paths.
#[derive(Default)]
pub struct VideoPlayerState {
    pub videos: HashMap<String, String>,
    video_extensions: HashSet<String>,
    next_index: AtomicUsize,
    root: Option<String>,
}

pub type SharedState = Arc<Mutex<VideoPlayerState>>;

/// The list of video extensions that are supported.
pub static VIDEO_EXTENSIONS: [&str; 13] = [
    "mp4",
    "av1",
    "avi",
    "flv",
    "heic",
    "mkv",
    "mov",
    "mpg",
    "mpeg",
    "m4v",
    "webm",
    "wmv",
    "3gp"
];


impl VideoPlayerState {
    /// Create a new video index state.
    /// This will configure the video extensions that are interpreted as videos.
    pub fn new() -> Self {
        Self {
            video_extensions: HashSet::from_iter(
                VIDEO_EXTENSIONS.iter().map(|s| s.to_string()),
            ),
            ..Default::default()
        }
    }


    fn advance_index(&mut self) {
        self.next_index
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    }

    /// Check if a path is a supported video file.
    pub fn is_video_file<P: AsRef<std::path::Path>>(&self, path: P) -> bool {
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

    /// Load a video from a path.
    pub fn load_video(&mut self, path: PathBuf) {
        let stored_file_name = path.to_str().unwrap().to_string();
        let extension = path.extension().unwrap();
        let server_path = format!(
            "{}.{}",
            self.next_index.load(Ordering::SeqCst),
            extension.to_str().unwrap()
        );
        info!("Loading video: {} as {}", stored_file_name, server_path);
        self.advance_index();
        self.videos.insert(server_path, stored_file_name);
    }

    /// Recursively visit all directories and load videos from them.
    pub fn visit_dirs<P: AsRef<std::path::Path>>(&mut self, root: P) -> std::io::Result<()> {
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

    /// Build a new video index state from a config.
    pub fn build(config: &VideoPlayerConfig) -> Self {
        let mut state = Self::new();
        state.root = Some(config.assets_root.clone());
        state.load_videos(state.root.clone().unwrap()).unwrap();
        state
    }

    /// Reload the video index state.
    pub fn reload(&mut self) {
        self.next_index = AtomicUsize::new(0);
        self.videos.clear();
        self.load_videos(self.root.clone().unwrap()).unwrap();
    }
}