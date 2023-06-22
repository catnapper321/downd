use crate::DownloaderCommand;
use std::path::Path;
use std::str::FromStr;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    net::{UnixListener, UnixStream},
    sync::mpsc::UnboundedSender,
};
mod parser;

pub async fn server(
    socket: UnixListener,
    tx_command: UnboundedSender<DownloaderCommand>,
) -> Result<(), std::io::Error> {
    loop {
        let (stream, _) = socket.accept().await?;
        tokio::spawn(handle_stream(stream, tx_command.clone()));
    }
    Ok(())
}

pub async fn handle_stream(
    stream: UnixStream,
    tx_command: UnboundedSender<DownloaderCommand>,
) -> Result<(), std::io::Error> {
    let mut client = BufReader::new(stream).lines();
    loop {
        // TODO: implement timeout here?
        let line = client.next_line().await?;
        if let Some(line) = line {
            // TODO: decode the command
            let msg = DownloaderCommand::from_str(&line);
            // ignore lines that cannot be parsed
            if let Ok(cmd) = msg {
                if tx_command.send(cmd).is_err() {
                    break;
                }
            }
        } else {
            break;
        }
    }
    Ok(())
}

pub async fn prep_socket_path(path: impl AsRef<Path>) {
    _ = tokio::fs::remove_file(path).await;
}
