#![allow(unused)]
use crate::*;
use tokio::{
    sync::broadcast,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    time::{sleep, Duration},
};
use tokio_stream::Stream;
use rollingrate::RollingRate;

async fn test_command(
    cmd: DownloaderCommand,
    chan: &UnboundedSender<DownloaderCommand>,
    delay: Duration,
) {
    sleep(delay).await;
    trace!("Sending {cmd:?}");
    _ = chan.send(cmd);
}
fn secs(n: u64) -> Duration {
    Duration::from_secs(n)
}
fn millis(n: u64) -> Duration {
    Duration::from_millis(n)
}

async fn reporter(mut update_rx: broadcast::Receiver<DownloaderMsg>) {
    debug!("reporter loop running");
    let mut avg = RollingRate::new(Duration::from_millis(2000), Duration::from_millis(10300));
    loop {
        let msg = update_rx.recv().await;
        if let Ok(msg) = msg {
            use DownloaderMsg::*;
            match msg {
                Starting(Some(url)) => debug!("Starting download of {url}"),
                Downloading { downloaded_bytes, .. } => {
                    let p = msg.progress().map(|x| x * 100.0);
                    avg.push(downloaded_bytes);
                    let r = avg.rate().map(|x| format!("{}/s", humanize_bytes(x)));
                    debug!("{p:.2?} | rate: {r:?}")
                }
                Moved(Some(url)) => {
                    debug!("Moved {url}");
                    avg.reset();
                },
                Stuck => error!("STUCK DOWNLOAD"),
                Hold(_) => {},
                Idle => {},
                QueueUpdate(urls) => {},
                _ => panic!("BLARG"),
            }
        } else {
            error!("Error msg: {msg:?}");
        }
    }
}

pub async fn main(cmd_tx: UnboundedSender<DownloaderCommand>,
                  mut update_rx: broadcast::Receiver<DownloaderMsg>,
                  ) -> Anything<()> {

    let _reporter = tokio::spawn(reporter(update_rx));

    // let thr = tokio::spawn(main_outer_loop(cmd_rx, update_tx));
    // let cmd = DownloaderCommand::AddUrl("fpQiIE8586Q".into());
    // test_command(cmd, &cmd_tx, secs(1)).await;

    let cmd = DownloaderCommand::AddUrl("testdata/download_raw.txt".into());
    test_command(cmd, &cmd_tx, secs(0)).await;
    // let cmd = DownloaderCommand::Pause;
    // test_command(cmd, &cmd_tx, secs(2)).await;
    // let cmd = DownloaderCommand::AddUrl("testdata/download_raw.txt".into());
    // test_command(cmd, &cmd_tx, secs(2)).await;
    // let cmd = DownloaderCommand::Resume;
    // test_command(cmd, &cmd_tx, secs(2)).await;
    // let cmd = DownloaderCommand::AddUrl("testdata/download_raw.txt".into());
    // test_command(cmd, &cmd_tx, secs(2)).await;

    Ok(())
}


