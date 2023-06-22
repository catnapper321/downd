use crate::*;
use std::{collections::VecDeque, task::Waker};

pub struct AsyncQueue<T> {
    queue: VecDeque<T>,
    waker: Option<Waker>,
}

impl<T: Clone> AsyncQueue<T> {
    pub fn new() -> Self {
        Self {
            queue: VecDeque::new(),
            waker: None,
        }
    }
    pub fn push(&mut self, url: impl Into<T>) {
        self.queue.push_back(url.into());
        if let Some(waker) = self.waker.take() {
            waker.wake();
        }
    }
    pub fn pop(&mut self) -> Option<T> {
        self.queue.pop_front()
    }
    pub fn len(&self) -> usize {
        self.queue.len()
    }
    pub fn move_up(&mut self, index: usize) {
        let len = self.len();
        if len < 2 || index == 0 || index > (len - 1) {
            return;
        }
        self.queue.swap(index, index - 1);
    }
    pub fn move_down(&mut self, index: usize) {
        let len = self.len();
        if len < 2 || index > (len - 2) {
            return;
        }
        self.queue.swap(index, index + 1);
    }
    pub fn remove(&mut self, index: usize) {
        let len = self.len();
        if len == 0 || index > (len - 1) {
            return;
        }
        self.queue.remove(index);
    }
    pub fn contents(&self) -> Vec<T> {
        self.queue.iter().map(|x| x.clone() ).collect()
    }
}

impl<T: Unpin + Clone> Stream for AsyncQueue<T> {
    type Item = T;

    /// returns None exactly once when queue is empty
    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        if let Some(url) = self.pop() {
            cx.waker().wake_by_ref();
            self.waker = None;
            Poll::Ready(Some(url))
        } else {
            let waker = cx.waker().clone();
            if self.waker.is_some() {
                self.waker = Some(waker);
                Poll::Pending
            } else {
                self.waker = Some(waker);
                Poll::Ready(None)
            }
        }
    }
}
