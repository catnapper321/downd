#![allow(unused)]
#![allow(dead_code, unreachable_code)]
use std::{
    pin::Pin,
    task::{Context, Poll},
    sync::{Arc, Mutex},
    path::{Path, PathBuf},
};
use tokio::{
    sync::broadcast,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    time::{sleep, Duration},
};
use tokio_stream::Stream;
mod delayed_stream;
use delayed_stream::*;
mod queue;
use queue::*;
mod ytdlp;
pub use ytdlp::*;
mod downloader;
use downloader::*;

mod unixsocket;
mod commands;
pub use commands::DownloaderCommand;
mod webapp;
use webapp::server;

mod testcode;
mod rollingrate;

mod tracker;
use tracker::*;

pub use tracing::{debug, error, info, trace, warn};

type Anything<T> = Result<T, Box<dyn std::error::Error + Send + Sync>>;
type Url = String;

#[derive(Debug, Clone, Copy)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace
}

#[derive(clap::Parser, Debug)]
struct Config {
    #[clap(short = 'p', long = "port", default_value = "3000")]
    port: u16,
    #[clap(short = 's', long = "socket")]
    socket: Option<std::path::PathBuf>,
    #[clap(short = 'v', action = clap::ArgAction::Count)]
    verbosity: u8,
}

async fn setup(config: &Config) {
    let loglevel = match config.verbosity {
        0 => tracing::Level::ERROR,
        1 => tracing::Level::INFO,
        2 => tracing::Level::DEBUG,
        _ => tracing::Level::TRACE,
    };
    let subscriber = tracing_subscriber::FmtSubscriber::builder()
        .with_max_level(loglevel)
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("Failed to start tracing");
}

fn get_socket_path(config: &Config) -> Result<PathBuf, String> {
    if let Some(p) = config.socket.clone() {
        return Ok(p)
    } 
    if let Ok(d) = std::env::var("XDG_RUNTIME_DIR") {
        let mut socket_path = PathBuf::new();
        socket_path.push(d);
        socket_path.push("downd");
        debug!("Using default socket path");
        Ok(socket_path)
    } else {
        error!("Socket path must be specified");
        Err("No socket path".into())
    }
}

use tokio::net::UnixListener;
async fn start_unix_socket(socket_path: impl AsRef<Path>) -> std::io::Result<UnixListener> {
    // attempt to remove the socket, if it exists already
    if let Ok(true) = tokio::fs::try_exists(&socket_path).await {
        tokio::fs::remove_file(&socket_path).await?;
    }
    UnixListener::bind(socket_path)
}

// #[tokio::main(flavor = "current_thread")]
#[tokio::main]
async fn main() -> Anything<()> {
    let c: Config = clap::Parser::parse();
    setup(&c).await;
    // figure out the path for the unix socket
    let socket_path = get_socket_path(&c)?;
    info!("Socket path is: {:?}", socket_path);
    let socket = start_unix_socket(socket_path).await?;
    // set up app channels
    let (cmd_tx, cmd_rx) = unbounded_channel::<DownloaderCommand>();
    let (update_tx, update_rx) = broadcast::channel(1024);
    // start web server
    // let webserver_task = tokio::spawn(
    //     webapp::server(update_tx.subscribe(), &c)
    //     );
    let web_ui = webapp::server(update_tx.subscribe(), c.port);
    // start unix socket
    let unix_socket = unixsocket::server(socket, cmd_tx.clone());
    // testing harness
    // let testcode_fut = testcode::main(cmd_tx.clone(), update_tx.subscribe());
    // start the main downloader task
    let main_thr = main_outer_loop(cmd_rx, update_tx);
    tokio::join!(web_ui, main_thr, unix_socket);
    unreachable!()
}

pub fn humanize_bytes(x: u64) -> String {
    let mut result = x as f64;
    for i in ["B", "KB", "MB", "GB", "TB", "PB", "EB"] {
        if result < 1000.0 {
            return format!("{result:.1} {i}");
        }
        result /= 1000.0;
    }
    format!("{result:.1} ZB")
}
