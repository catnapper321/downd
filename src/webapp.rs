#![allow(unused)]
pub use tracing::{trace, debug, info, warn, error};
use std::{net::SocketAddr, convert::Infallible, ops::DerefMut};
use askama::Template;
use tokio::{
    select,
    time::Duration,
    sync::{broadcast, mpsc},
    stream,
};
use warp::{http::StatusCode, Filter, sse::Event};
use tokio_stream::{
    Stream,
    StreamExt,
    wrappers::BroadcastStream,     
};
use std::sync::{Arc, Mutex};

use crate::{humanize_bytes, DownloaderMsg, Config};
use crate::rollingrate::RollingRate;

#[derive(Template)]
#[template(path = "root.html")]
struct Root { }

#[derive(Clone)]
struct UpdateChan<T>(Arc<Mutex<broadcast::Sender<T>>>);
impl<T: Clone> UpdateChan<T> {
    fn new() -> Self {
        let (ch, _) = broadcast::channel(32);
        Self(Arc::new(Mutex::new(ch)))
    }
    fn subscribe(&self) -> broadcast::Receiver<T> {
        self.0.lock().unwrap().subscribe()
    }
    fn send(&self, msg: T) -> Result<usize, tokio::sync::broadcast::error::SendError<T>> {
        self.0.lock().unwrap().send(msg)
    }
}

pub async fn server(update_rx: broadcast::Receiver<DownloaderMsg>,
                    port: u16,
                    ) {
    // let (update_chan, _) = broadcast::channel::<DownloaderMsg>(32);
    // let update_chan = Arc::new(Mutex::new(update_chan));
    let update_chan = UpdateChan::new();
    info!("Starting web server");
    let (kick_tx, kick_rx) = mpsc::channel(1);
    tokio::task::spawn(statemonitor(update_rx, update_chan.clone(), kick_rx));
    let root_route = warp::path!("root")
        .and(warp::get())
        // and_then requires a fn that returns a TryFuture, whose
        // error type is warp::Rejection
        .and_then(test_root);
    let sse_route = warp::path("sse")
        .and(warp::get())
        .map(move || update_chan.subscribe())
        .map(move |a| {
            warp::sse::reply(warp::sse::keep_alive().stream(sse_test(a)))
        });
    let sse_kick = warp::path("sse")
        .and(warp::post())
        .map(move || kick_tx.clone())
        .and_then(kick)
        ;
    let routes = root_route
        .or(sse_kick)
        .or(sse_route)
        ;
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    warp::serve(routes).run(addr).await;
    // .with(warp::cors().allow_any_origin())
}

fn humanize_rate(r: Option<u64>) -> Option<String> {
    r.map(|r| format!("{}/s", humanize_bytes(r)))
}

/// keeps the state tracker in a dedicated task and manages the update 
/// broadcast channel
async fn statemonitor(mut update_rx: broadcast::Receiver<DownloaderMsg>, chan: UpdateChan<String>, mut kick_chan: mpsc::Receiver<()>) {
    debug!("statemonitor started");
    let mut update = crate::tracker::Tracker::new();
    loop {
        select! {
            Ok(msg) = update_rx.recv() => {
                update.update(msg);
            },
            _ = kick_chan.recv() => {
            },
            else => { continue },
        }
        let html = update.render();
        if let Ok(html) = html {
            chan.send(html);
        } else {
            error!("Could not construct HTML update");
        }
    }
    unreachable!()
}

// // synchronous code can use the simple warp::Reply type
// fn root() -> impl warp::Reply {
//     let h = Root {
//     };
//     let reply = h.render().unwrap();
//     warp::reply::html(reply)
// }

/// Initiates resending the latest SSE message to all connected clients
async fn kick(kick_chan: mpsc::Sender<()>) -> Result<impl warp::Reply, Infallible> {
    kick_chan.send(()).await;
    Ok(warp::reply::with_status(warp::reply(), warp::http::StatusCode::OK))
}

pub async fn test_root() -> Result<impl warp::Reply, Infallible> {
    let h = Root {
    };
    let reply = h.render().unwrap();
    Ok(warp::reply::html(reply))
}

fn sse_test(chan: broadcast::Receiver<String>) -> impl Stream<Item = Result<Event, Infallible>> {
    BroadcastStream::new(chan).map(|item| { Ok(Event::default().data(item.unwrap())) })
}

