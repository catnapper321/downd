use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tokio::time::{Duration, Instant};
use tokio_stream::Stream;

pub struct DelayedStream<S> {
    stream: Pin<Box<dyn Stream<Item = S> + Send>>,
    sleeper: Pin<Box<tokio::time::Sleep>>,
    /// millisecond delay
    delay: Duration,
}

impl<S> DelayedStream<S> {
    pub fn new(s: impl Stream<Item = S> + Send + 'static, delay: Duration) -> Self {
        Self {
            stream: Box::pin(s),
            sleeper: Box::pin(tokio::time::sleep(delay)),
            delay,
        }
    }
}

impl<S> Stream for DelayedStream<S> {
    type Item = S;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let delay = self.delay;
        match self.sleeper.as_mut().poll(cx) {
            Poll::Ready(_) => {
                self.sleeper.as_mut().reset(Instant::now() + delay);
                self.stream.as_mut().poll_next(cx)
            }
            Poll::Pending => Poll::Pending,
        }
    }
}
