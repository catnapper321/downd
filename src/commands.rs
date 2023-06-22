#[derive(PartialEq, Eq, Debug)]
pub enum DownloaderCommand {
    AddUrl(String),
    Pause,
    Cancel,
    Resume,
    MoveDown(usize),
    MoveUp(usize),
    Delete(usize),
}
