// mod downloader;
// pub use downloader::*;
mod parser;
use parser::*;
use crate::Url;

#[derive(Debug, Clone)]
pub enum DownloaderMsg {
    Starting(Option<String>),
    Downloading {
        downloaded_bytes: u64,
        total_bytes: Option<u64>,
        frag_index: Option<u64>,
        frag_count: Option<u64>,
    },
    Moved(Option<String>),
    Stuck,
    Idle,
    Hold(String),
    QueueUpdate(Vec<Url>),
}

impl DownloaderMsg {
    pub fn progress(&self) -> Option<f64> {
        match *self {
            Self::Downloading {
                downloaded_bytes,
                total_bytes: Some(total_bytes),
                frag_index: Some(frag_index),
                frag_count: Some(frag_count),
                ..
            } => {
                let b = downloaded_bytes as f64 / total_bytes as f64;
                let f = frag_index as f64 / frag_count as f64;
                Some((b + f) * 0.5)
            }
            Self::Downloading {
                downloaded_bytes,
                total_bytes: Some(total_bytes),
                ..
            } => Some(downloaded_bytes as f64 / total_bytes as f64),
            Self::Downloading {
                frag_index: Some(frag_index),
                frag_count: Some(frag_count),
                ..
            } => Some(frag_index as f64 / frag_count as f64),
            _ => None,
        }
        .map(|value| value.clamp(0.0, 1.0))
    }
    pub fn downloaded_bytes(&self) -> Option<u64> {
        match *self {
            Self::Downloading {
                downloaded_bytes, ..
            } => Some(downloaded_bytes),
            _ => None,
        }
    }
    pub fn total_bytes(&self) -> Option<u64> {
        match *self {
            DownloaderMsg::Downloading { total_bytes, .. } => total_bytes,
            _ => None,
        }
    }
    pub fn title(&self) -> Option<&String> {
        use DownloaderMsg::*;
        match self {
            Starting(title) | Moved(title) => title.as_ref(),
            _ => None,
        }
    }
}

impl TryFrom<String> for DownloaderMsg {
    type Error = String; // hacky
    fn try_from(value: String) -> Result<Self, Self::Error> {
        parse_progress_update_line(&value).map_err(|e| e.to_string())
    }
}

pub fn ytdlp_command(url: impl AsRef<std::ffi::OsStr>) -> tokio::process::Command {
    let mut c = tokio::process::Command::new("/usr/bin/yt-dlp");
    c.arg("--progress")
    .arg("--progress-template=download:DOWNLOAD|%(progress.downloaded_bytes)d|%(progress.total_bytes,progress.total_bytes_estimate)d|%(progress.fragment_index)d|%(progress.fragment_count)d|")
    .arg("-O").arg("after_move:MOVED|%(title,alt_title,fulltitle,filename)s")
    .arg("-O").arg("video:START|%(title,alt_title,fulltitle,filename)s")
    .arg("--newline")
    .arg("-q")
    .arg("--")
    .arg(url);
    c
}
