//! A cancel handle shared across threads: what stops a watch loop or a sign-in that is polling,
//! and what wakes a watch loop's wait for an early pass. Cloning shares it.

use std::sync::{Arc, Condvar, Mutex, MutexGuard};
use std::time::Duration;

#[derive(Debug, Default)]
struct Flags {
    cancelled: bool,
    /// An early pass was asked for; cleared by the wait it ends.
    wake: bool,
}

#[derive(Debug, Default)]
struct Shared {
    flags: Mutex<Flags>,
    cv: Condvar,
}

/// What ended a [`CancelToken::wait_for`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Waited {
    Cancelled,
    Woken,
    TimedOut,
}

/// Stops a watch loop or a sign-in from another thread. Cloning shares it.
#[derive(Debug, Clone, Default)]
pub struct CancelToken(Arc<Shared>);

impl CancelToken {
    pub fn new() -> Self {
        Self::default()
    }

    fn flags(&self) -> MutexGuard<'_, Flags> {
        self.0.flags.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn cancel(&self) {
        self.flags().cancelled = true;
        self.0.cv.notify_all();
    }

    pub fn is_cancelled(&self) -> bool {
        self.flags().cancelled
    }

    /// Whether `other` is a clone of this token (the same handle, not just the same state).
    pub fn same(&self, other: &CancelToken) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// Wait up to `d`; true if cancelled meanwhile (returns at once when it is). A wake doesn't
    /// end this wait.
    pub fn wait(&self, d: Duration) -> bool {
        let guard = self.flags();
        let (guard, _) = self
            .0
            .cv
            .wait_timeout_while(guard, d, |f| !f.cancelled)
            .unwrap_or_else(|e| e.into_inner());
        guard.cancelled
    }

    /// Ask the loop waiting on this token for a pass now (`wait_or_wake` returns early). A wake
    /// asked for while nothing waits ends the next wait at once.
    pub fn wake(&self) {
        self.flags().wake = true;
        self.0.cv.notify_all();
    }

    /// Wait up to `d`, or until cancelled or woken, and say which ended it. Consumes the wake.
    pub fn wait_for(&self, d: Duration) -> Waited {
        let guard = self.flags();
        let (mut guard, _) = self
            .0
            .cv
            .wait_timeout_while(guard, d, |f| !f.cancelled && !f.wake)
            .unwrap_or_else(|e| e.into_inner());
        let woken = std::mem::take(&mut guard.wake);
        if guard.cancelled {
            Waited::Cancelled
        } else if woken {
            Waited::Woken
        } else {
            Waited::TimedOut
        }
    }

    /// Wait up to `d`, or until cancelled or woken; true if cancelled. Consumes the wake.
    pub fn wait_or_wake(&self, d: Duration) -> bool {
        let guard = self.flags();
        let (mut guard, _) = self
            .0
            .cv
            .wait_timeout_while(guard, d, |f| !f.cancelled && !f.wake)
            .unwrap_or_else(|e| e.into_inner());
        guard.wake = false;
        guard.cancelled
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;

    #[test]
    fn cancel_wakes_a_wait_at_once() {
        let c = CancelToken::new();
        let other = c.clone();
        let t = std::thread::spawn(move || {
            let start = Instant::now();
            let cancelled = other.wait(Duration::from_secs(30));
            (cancelled, start.elapsed())
        });
        std::thread::sleep(Duration::from_millis(50));
        c.cancel();
        let (cancelled, took) = t.join().unwrap();
        assert!(cancelled && took < Duration::from_secs(5));
        assert_eq!(c.wait_for(Duration::from_secs(30)), Waited::Cancelled);
        assert!(!CancelToken::new().wait(Duration::from_millis(1)));
        assert!(c.same(&c.clone()) && !c.same(&CancelToken::new()));
    }

    #[test]
    fn a_wake_ends_one_wait_without_cancelling() {
        let c = CancelToken::new();
        let other = c.clone();
        let t = std::thread::spawn(move || {
            let start = Instant::now();
            let cancelled = other.wait_or_wake(Duration::from_secs(30));
            (cancelled, start.elapsed())
        });
        std::thread::sleep(Duration::from_millis(50));
        c.wake();
        let (cancelled, took) = t.join().unwrap();
        assert!(!cancelled && took < Duration::from_secs(5));
        assert!(!c.is_cancelled());
        // Asked for before the wait: the wait ends at once, and only that one.
        c.wake();
        let start = Instant::now();
        assert!(!c.wait_or_wake(Duration::from_secs(30)));
        assert!(start.elapsed() < Duration::from_secs(5));
        assert!(!c.wait_or_wake(Duration::from_millis(1)));
        // wait_for says what ended it.
        c.wake();
        assert_eq!(c.wait_for(Duration::from_secs(30)), Waited::Woken);
        assert_eq!(c.wait_for(Duration::from_millis(1)), Waited::TimedOut);
        // A plain wait ignores a wake.
        c.wake();
        let start = Instant::now();
        assert!(!c.wait(Duration::from_millis(100)));
        assert!(start.elapsed() >= Duration::from_millis(100));
    }
}
