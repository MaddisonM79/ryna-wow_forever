//! Small things every module needs: the clock, file times, and how to name this machine.

use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crate::cancel::CancelToken;

/// The time and the means to wait, so retries and polling can be tested without waiting.
pub trait Clock {
    /// Unix seconds.
    fn now(&self) -> i64;
    fn sleep(&self, d: Duration);
    /// Sleep up to `d`, or until `cancel` fires; true if it has. The real clock wakes at once on
    /// a cancel; this default (the tests' clock) sleeps the whole time and checks before and after.
    fn sleep_unless_cancelled(&self, d: Duration, cancel: &CancelToken) -> bool {
        if cancel.is_cancelled() {
            return true;
        }
        self.sleep(d);
        cancel.is_cancelled()
    }
}

/// The real clock.
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> i64 {
        now_secs()
    }
    fn sleep(&self, d: Duration) {
        std::thread::sleep(d);
    }
    fn sleep_unless_cancelled(&self, d: Duration, cancel: &CancelToken) -> bool {
        cancel.wait(d)
    }
}

pub fn now_secs() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs() as i64)
}

/// A file's modification time, whole seconds and nanoseconds since the epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Mtime {
    pub secs: i64,
    pub nanos: i128,
}

pub fn mtime(path: &Path) -> std::io::Result<Mtime> {
    let modified = std::fs::metadata(path)?.modified()?;
    let (secs, nanos) = match modified.duration_since(UNIX_EPOCH) {
        Ok(d) => (d.as_secs() as i64, d.as_nanos() as i128),
        Err(e) => {
            let d = e.duration();
            (-(d.as_secs() as i64), -(d.as_nanos() as i128))
        }
    };
    Ok(Mtime { secs, nanos })
}

/// Unix seconds as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn iso_utc(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    let rem = secs.rem_euclid(86_400);
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// This machine's name for the heartbeat: `RYNA_HOST`, else the OS's short host name.
pub fn hostname() -> String {
    if let Ok(h) = std::env::var("RYNA_HOST") {
        if !h.trim().is_empty() {
            return h.trim().to_string();
        }
    }
    if cfg!(windows) {
        if let Ok(h) = std::env::var("COMPUTERNAME") {
            if !h.is_empty() {
                return h;
            }
        }
    } else if let Ok(out) = std::process::Command::new("hostname").arg("-s").output() {
        let h = String::from_utf8_lossy(&out.stdout).trim().to_string();
        if out.status.success() && !h.is_empty() {
            return h;
        }
    }
    "unknown".to_string()
}

/// `macos`, `windows` or `linux` (what the heartbeat's `os` says).
pub fn os_name() -> &'static str {
    std::env::consts::OS
}

/// Test doubles, for this crate's tests and (feature `testing`) a core's.
#[cfg(any(test, feature = "testing"))]
#[doc(hidden)]
pub mod testing {
    use super::*;
    use std::cell::{Cell, RefCell};

    /// A clock that only moves when something sleeps, and remembers every sleep.
    pub struct FakeClock {
        pub now: Cell<i64>,
        pub sleeps: RefCell<Vec<Duration>>,
    }

    impl FakeClock {
        pub fn at(now: i64) -> Self {
            FakeClock {
                now: Cell::new(now),
                sleeps: RefCell::new(Vec::new()),
            }
        }
    }

    impl Clock for FakeClock {
        fn now(&self) -> i64 {
            self.now.get()
        }
        fn sleep(&self, d: Duration) {
            self.now.set(self.now.get() + d.as_secs() as i64);
            self.sleeps.borrow_mut().push(d);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iso_dates() {
        assert_eq!(iso_utc(0), "1970-01-01T00:00:00Z");
        assert_eq!(iso_utc(1_762_464_000), "2025-11-06T21:20:00Z");
        assert_eq!(iso_utc(951_782_400), "2000-02-29T00:00:00Z");
        assert_eq!(iso_utc(-1), "1969-12-31T23:59:59Z");
    }

    #[test]
    fn mtime_of_a_new_file_is_now() {
        let f = tempfile::NamedTempFile::new().unwrap();
        let m = mtime(f.path()).unwrap();
        assert!((m.secs - now_secs()).abs() < 120);
        assert_eq!(m.nanos / 1_000_000_000, i128::from(m.secs));
    }
}
