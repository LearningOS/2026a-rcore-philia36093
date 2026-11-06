//! Synchronization and interior mutability primitives

mod condvar;
mod mutex;
mod semaphore;
mod up;

use lazy_static::*;

pub use condvar::Condvar;
pub use mutex::{Mutex, MutexBlocking, MutexSpin};
pub use semaphore::Semaphore;
pub use up::UPSafeCell;

lazy_static! {
    // Global flag controlling deadlock detection.
    static ref DEADLOCK_DETECT: UPSafeCell<bool> = unsafe { UPSafeCell::new(false) };
}

/// Enable or disable deadlock detection.
pub fn set_deadlock_detect(enabled: bool) {
    *DEADLOCK_DETECT.exclusive_access() = enabled;
}

/// Whether deadlock detection is currently enabled.
pub fn deadlock_detect() -> bool {
    *DEADLOCK_DETECT.exclusive_access()
}
