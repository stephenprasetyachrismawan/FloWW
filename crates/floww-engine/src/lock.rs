//! One shared local database per workspace, so steps that touch it take turns.
//!
//! Waiters queue in order. A person can let a waiting PRD go first, which
//! pauses the current holder (it is cancelled and re-queued at the front).

use std::collections::VecDeque;
use std::sync::Arc;

use serde::Serialize;
use tokio::sync::{Mutex, Notify, watch};

#[derive(Debug, Clone, Serialize, Default)]
pub struct LockState {
    pub holder: Option<String>,
    pub doing: String,
    pub queue: Vec<String>,
}

struct Inner {
    holder: Option<(String, String, watch::Sender<bool>)>,
    queue: VecDeque<String>,
}

#[derive(Clone)]
pub struct DbLock {
    inner: Arc<Mutex<Inner>>,
    notify: Arc<Notify>,
    changed: Arc<dyn Fn() + Send + Sync>,
}

/// Held while a PRD uses the database. Dropping it releases the lock.
pub struct Guard {
    lock: DbLock,
    prd: String,
    /// Becomes `true` when someone takes the database over.
    pub preempted: watch::Receiver<bool>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        let lock = self.lock.clone();
        let prd = self.prd.clone();
        tokio::spawn(async move { lock.release(&prd).await });
    }
}

impl DbLock {
    pub fn new(changed: impl Fn() + Send + Sync + 'static) -> Self {
        Self {
            inner: Arc::new(Mutex::new(Inner {
                holder: None,
                queue: VecDeque::new(),
            })),
            notify: Arc::new(Notify::new()),
            changed: Arc::new(changed),
        }
    }

    pub async fn state(&self) -> LockState {
        let g = self.inner.lock().await;
        LockState {
            holder: g.holder.as_ref().map(|h| h.0.clone()),
            doing: g.holder.as_ref().map(|h| h.1.clone()).unwrap_or_default(),
            queue: g.queue.iter().cloned().collect(),
        }
    }

    /// Wait for the database. `on_wait` runs once if the PRD has to queue.
    pub async fn acquire(&self, prd: &str, doing: &str, on_wait: impl FnOnce(&str)) -> Guard {
        let mut announced = false;
        let mut on_wait = Some(on_wait);
        loop {
            let notified = self.notify.notified();
            {
                let mut g = self.inner.lock().await;
                let first = g.queue.front().map(|q| q == prd).unwrap_or(true);
                let free = g.holder.is_none();
                if free && first {
                    g.queue.retain(|q| q != prd);
                    let (tx, rx) = watch::channel(false);
                    g.holder = Some((prd.to_string(), doing.to_string(), tx));
                    drop(g);
                    (self.changed)();
                    return Guard {
                        lock: self.clone(),
                        prd: prd.to_string(),
                        preempted: rx,
                    };
                }
                if g.holder.as_ref().map(|h| h.0 == prd).unwrap_or(false) {
                    // Re-entrant use by the same PRD: hand out a guard that shares the hold.
                    let rx = g.holder.as_ref().expect("checked").2.subscribe();
                    return Guard {
                        lock: self.clone(),
                        prd: format!("{prd}#nested"),
                        preempted: rx,
                    };
                }
                if !g.queue.iter().any(|q| q == prd) {
                    g.queue.push_back(prd.to_string());
                }
                if !announced {
                    announced = true;
                    let holder = g.holder.as_ref().map(|h| h.0.clone()).unwrap_or_default();
                    drop(g);
                    (self.changed)();
                    if let Some(f) = on_wait.take() {
                        f(&holder);
                    }
                }
            }
            notified.await;
        }
    }

    async fn release(&self, prd: &str) {
        if prd.ends_with("#nested") {
            return;
        }
        let mut g = self.inner.lock().await;
        if g.holder.as_ref().map(|h| h.0 == prd).unwrap_or(false) {
            g.holder = None;
        }
        drop(g);
        self.notify.notify_waiters();
        (self.changed)();
    }

    /// Let `prd` go first: it moves to the front, and the holder is told to stop.
    /// Returns the PRD that was paused.
    pub async fn take_over(&self, prd: &str) -> Option<String> {
        let mut g = self.inner.lock().await;
        g.queue.retain(|q| q != prd);
        g.queue.push_front(prd.to_string());
        let paused = g.holder.as_ref().map(|h| {
            let _ = h.2.send(true);
            h.0.clone()
        });
        drop(g);
        (self.changed)();
        paused
    }

    /// Remove a PRD from the queue (it was cancelled while waiting).
    pub async fn leave(&self, prd: &str) {
        let mut g = self.inner.lock().await;
        g.queue.retain(|q| q != prd);
        drop(g);
        self.notify.notify_waiters();
        (self.changed)();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn waiters_go_in_order() {
        let lock = DbLock::new(|| {});
        let g1 = lock.acquire("A", "tests", |_| {}).await;
        let l2 = lock.clone();
        let t2 = tokio::spawn(async move {
            let _g = l2.acquire("B", "tests", |_| {}).await;
            "B"
        });
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(lock.state().await.queue, vec!["B"]);
        drop(g1);
        assert_eq!(t2.await.unwrap(), "B");
    }

    #[tokio::test]
    async fn take_over_preempts_holder() {
        let lock = DbLock::new(|| {});
        let mut g1 = lock.acquire("A", "odoo", |_| {}).await;
        let l2 = lock.clone();
        let t2 = tokio::spawn(async move { l2.acquire("B", "tests", |_| {}).await.prd.clone() });
        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(lock.take_over("B").await, Some("A".to_string()));
        g1.preempted.changed().await.unwrap();
        assert!(*g1.preempted.borrow());
        drop(g1);
        assert_eq!(t2.await.unwrap(), "B");
    }
}
