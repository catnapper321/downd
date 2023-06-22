use std::time::Duration;

use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    select,
    time::Instant,
};
use tokio_stream::{wrappers::LinesStream, Stream, StreamExt};

use crate::*;

const STUCK_DURATION: Duration = Duration::from_secs(15);

#[derive(Debug)]
/// The reason why the downloader process terminated
pub enum ExitReason {
    /// Normal termination, exit code 0
    Finished,
    /// A non-zero exit code
    ExitCode(i32),
    /// User cancelled the download
    Cancelled,
    /// User paused the download. Identical to Cancelled, but the URL
    /// remains at the head of the queue.
    Paused,
    /// An IO error happened while reading one of the downloader's streams
    IOError(std::io::Error),
    /// The downloader was killed by an external signal (SIGTERM, SIGKILL)
    ExternalSignal,
    /// The task that monitors the downloader process ended improperly (panic)
    Panic,
}

impl std::fmt::Display for ExitReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self}")
    }
}

impl std::error::Error for ExitReason {}

// Is this a bad idea?
// The Try::from_residual stuff hasn't landed yet…
impl<T: Default> From<ExitReason> for Result<T, ExitReason> {
    fn from(value: ExitReason) -> Self {
        match value {
            ExitReason::Finished => Ok(T::default()),
            _ => Err(value),
        }
    }
}

/// Spawns the given command and returns newline separated String streams for
/// stdout and stderr
pub fn spawn_downloader_command<'a>(
    mut cmd: Command,
) -> (
    Child,
    impl Stream<Item = tokio::io::Result<String>>,
) {
    let mut child = cmd
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("Malformed command");
    let out = child.stdout.take().expect("!!!");
    let err = child.stderr.take().expect("!!!");
    let out = BufReader::new(out).lines();
    let err = BufReader::new(err).lines();
    let out = LinesStream::new(out);
    let err = LinesStream::new(err);
    let st = out.merge(err);
    (child, st)
}

fn handle_queue_commands(q: &mut AsyncQueue<Url>, cmd: &DownloaderCommand, update_tx: &broadcast::Sender<DownloaderMsg>) {
    match cmd {
        DownloaderCommand::AddUrl(url) => {
            q.push(url);
        }
        DownloaderCommand::MoveDown(index) => {
            q.move_down(*index);
        }
        DownloaderCommand::MoveUp(index) => {
            q.move_up(*index);
        }
        DownloaderCommand::Delete(index) => {
            q.remove(*index);
        }
        _ => return,
    }
    update_tx.send(DownloaderMsg::QueueUpdate(q.contents()));
}

async fn run_hold(
    q: &mut AsyncQueue<Url>,
    url: &mut Option<Url>,
    cmd_rx: &mut UnboundedReceiver<DownloaderCommand>,
    update_tx: &broadcast::Sender<DownloaderMsg>,
) {
    info!("Holding for user input");
    loop {
        let cmd = cmd_rx.recv().await;
        if let Some(cmd) = cmd {
            debug!("Command received: {cmd:?}");
            use DownloaderCommand::*;
            match cmd {
                Resume => {
                    debug!("resume received while holding");
                    break
                },
                Cancel => {
                    // TODO: broadcast the cancellation
                    *url = None;
                }
                _ => handle_queue_commands(q, &cmd, update_tx),
            }
        } else {
            panic!("command channel dropped");
        }
    }
}

async fn run_idle(
    q: &mut AsyncQueue<Url>,
    current_url: &mut Option<Url>,
    cmd_rx: &mut UnboundedReceiver<DownloaderCommand>,
    update_tx: &broadcast::Sender<DownloaderMsg>,
) {
    info!("In idle");
    update_tx.send(DownloaderMsg::Idle);
    loop {
        select! {
            url = q.next() => {
                if let Some(url) = url {
                    *current_url = Some(url);
                    break;
                } else {
                    debug!("queue is empty");
                }
            },
            cmd = cmd_rx.recv() => {
                if let Some(cmd) = cmd {
                    debug!("Command received: {cmd:?}");
                    handle_queue_commands(q, &cmd, update_tx);
                    if matches!(cmd, DownloaderCommand::Pause) {
                        update_tx.send(DownloaderMsg::Hold("User hold".into()));
                        _ = run_hold(q, current_url, cmd_rx, update_tx).await;
                        update_tx.send(DownloaderMsg::Idle);
                    }
                } else {
                    panic!("command channel dropped");
                }
            },
            else => {
                panic!("nothing else to do in idle");
            },
        }
    }
}

fn start_downloader_test_process(
    url: impl AsRef<std::ffi::OsStr>,
) -> (
    Child,
    impl Stream<Item = Result<String, std::io::Error>>,
) {
    debug!(
        "Constructing command for url: {}",
        url.as_ref().to_string_lossy()
    );
    let mut cmd = Command::new("/usr/bin/cat");
    cmd.arg(&url);
    let (child, st) = spawn_downloader_command(cmd);
    let st = DelayedStream::new(st, Duration::from_millis(250));
    (child, st)
}

fn start_downloader_process(
    url: impl AsRef<std::ffi::OsStr>,
) -> (
    Child,
    impl Stream<Item = Result<String, std::io::Error>>,
) {
    let mut cmd = ytdlp_command(&url);
    cmd.arg(url);
    spawn_downloader_command(cmd)
}

async fn main_inner_loop(
    q: &mut AsyncQueue<Url>,
    cmd_rx: &mut UnboundedReceiver<DownloaderCommand>,
    update_tx: &broadcast::Sender<DownloaderMsg>,
) -> ExitReason {
    let mut current_url: Option<Url> = None;
    loop {
        run_idle(q, &mut current_url, cmd_rx, update_tx).await;
        update_tx.send(DownloaderMsg::QueueUpdate(q.contents()));

        // downloader loop
        while let Some(url) = &current_url {
            // TODO: remove test code
            let (child, st) = start_downloader_process(&url);
            // let (child, st) = start_downloader_test_process(&url);

            let exitreason = handle_downloader(q, cmd_rx, child, st, update_tx).await;
            info!("transition from downloading is {exitreason:?}");
            use ExitReason::*;
            match exitreason {
                Finished => break,
                ExitCode(e) => {
                    error!("Downloader exited with error code {e}");
                    update_tx.send(DownloaderMsg::Hold(format!("Error code {e}")));
                    run_hold(q, &mut current_url, cmd_rx, update_tx).await;
                    break;
                }
                Cancelled => {
                    debug!("Download cancelled by user");
                    break;
                }
                Paused => {
                    update_tx.send(DownloaderMsg::Hold("User hold".into()));
                    run_hold(q, &mut current_url, cmd_rx, update_tx).await;
                }
                IOError(e) => {
                    error!("Error: {e:?}");
                    update_tx.send(DownloaderMsg::Hold("IO Error!".into()));
                    run_hold(q, &mut current_url, cmd_rx, update_tx).await;
                    break;
                }
                ExternalSignal => {
                    error!("Downloader killed via external signal");
                    update_tx.send(DownloaderMsg::Hold("Downloader killed".into()));
                    run_hold(q, &mut current_url, cmd_rx, update_tx).await;
                    break;
                }
                Panic => todo!(),
            }
        } // downloader loop
    } // inner loop
}

pub async fn main_outer_loop(
    mut cmd_rx: UnboundedReceiver<DownloaderCommand>,
    // update_tx: broadcast::Sender<DownloaderMsg>,
    update_tx: broadcast::Sender<DownloaderMsg>,
) {
    let mut q = AsyncQueue::<Url>::new();
    info!("Entering main outer loop");
    loop {
        let trans = main_inner_loop(&mut q, &mut cmd_rx, &update_tx).await;
        info!("Inner loop exited with {trans:?}");
        break;
    }
}

fn handle_line(line: tokio::io::Result<String>, chan: &broadcast::Sender<DownloaderMsg>) {
    // ignore lines that we can't parse
    if let Ok(x) = line {
        if let Ok(msg) = DownloaderMsg::try_from(x) {
            _ = chan.send(msg);
        }
    }
}

async fn handle_downloader(
    q: &mut AsyncQueue<Url>,
    cmd_rx: &mut UnboundedReceiver<DownloaderCommand>,
    mut child: Child,
    st: impl Stream<Item = tokio::io::Result<String>>,
    tx: &broadcast::Sender<DownloaderMsg>,
) -> ExitReason {
    info!("In downloader handler");
    tokio::pin!(st); // I expect these buffers to be allocated anyway
    let mut reading_out = true;
    let mut stuck = false;
    let stuck_timer = tokio::time::sleep(STUCK_DURATION);
    // set if cancel or pause command is received
    let mut user_exitreason: Option<ExitReason> = None;
    tokio::pin!(stuck_timer);
    loop {
        select! {
            _ = &mut stuck_timer, if ! stuck => {
                stuck = true;
                tx.send(DownloaderMsg::Stuck);
            },
            line = st.next(), if reading_out => {
                stuck = false;
                stuck_timer.as_mut().reset(Instant::now() + STUCK_DURATION);
                if let Some(x) = line {
                    handle_line(x, tx);
                } else {
                    reading_out = false;
                }
            },
            stat = child.wait(), if !reading_out => {
                // if user cancelled or paused, just return the reason
                if let Some(exitreason) = user_exitreason {
                    return exitreason;
                }
                match stat {
                    Ok(exit_status) => {
                        return match exit_status.code() {
                            Some(0) => ExitReason::Finished,
                            Some(code) => ExitReason::ExitCode(code),
                            // External signal killed the process, most likely
                            // will likely get ioerror from broken pipes first
                            None => ExitReason::ExternalSignal,
                        }
                    },
                    Err(e) => return ExitReason::IOError(e),
                }
            },
            cmd = cmd_rx.recv() => {
                if let Some(cmd) = cmd {
                    debug!("Command received while downloading: {cmd:?}");
                    match cmd {
                        DownloaderCommand::Cancel => {
                            _ = child.kill().await;
                            reading_out = false;
                            user_exitreason = Some(ExitReason::Cancelled);
                        },
                        DownloaderCommand::Pause => {
                            _ = child.kill().await;
                            reading_out = false;
                            user_exitreason = Some(ExitReason::Paused);
                        },
                        _ => {
                            handle_queue_commands(q, &cmd, tx)
                        },
                    }
                } else {
                    error!("command channel dropped");
                    return ExitReason::Panic;
                }
            },
        }
    }
    unreachable!()
}
