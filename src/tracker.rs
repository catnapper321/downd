mod rollingrate;
use rollingrate::RollingRate;
use crate::*;
use askama::Template;

#[derive(Template, Default)]
#[template(path = "sse_update.html")]
pub struct Tracker {
    pub title: Option<String>,
    pub state: String,
    pub progress: Option<f64>,
    rate: Option<u64>,
    pub rate_h: Option<String>,
    pub queue: Vec<String>,
    pub total_bytes: Option<u64>,
    downloaded_bytes: u64,
    pub eta: Option<u64>,
    rolling_rate: RollingRate,
}

impl Tracker {
    pub fn new() -> Self {
        let mut x = Self::default();
        x.state = "Idle".into();
        x.rolling_rate = RollingRate::new(Duration::from_millis(1500), Duration::from_secs(15));
        x
    }
    pub fn update(&mut self, msg: DownloaderMsg) {
        use DownloaderMsg::*;
        match msg {
            Starting(title) => {
                self.state = "Starting".into();
                self.title = title;
            },
            Downloading { downloaded_bytes, total_bytes, .. } => {
                self.rolling_rate.push(downloaded_bytes);
                self.rate = self.rolling_rate.rate();
                self.state = "Downloading".into();
                self.progress = msg.progress();
                self.total_bytes = total_bytes;
                self.downloaded_bytes = downloaded_bytes;
                self.calculate();
            },
            Moved(title) => {
                // self.state = "Finishing".into();
            },
            Stuck => {
                self.state = "Stuck".into();
                self.progress = None;
                self.rolling_rate.reset();
            },
            Idle => {
                self.state = "Idle".into();
                self.progress = None;
                self.title = None;
                self.rate_h = None;
                self.rolling_rate.reset();
            },
            Hold(reason) => {
                self.state = format!("Holding: {reason}");
                self.rolling_rate.reset();
            },
            QueueUpdate(urls) => {
                self.queue = urls;
            }
        }
    }
    /// Evaluates the calculated fields
    pub fn calculate(&mut self) {
        self.rate_h = humanize_rate(self.rate); 
        if let Some(r) = self.rate {
            if let Some(t) = self.total_bytes {
                self.eta = Some( (t - self.downloaded_bytes) / r );
                return;
            }
        }
    }
}

fn humanize_rate(r: Option<u64>) -> Option<String> {
    r.map(|r| format!("{}/s", humanize_bytes(r)))
}
