//! Semaphore

use crate::sync::UPSafeCell;
use crate::task::{block_current_and_run_next, current_task, wakeup_task, TaskControlBlock};
use alloc::{collections::VecDeque, sync::Arc, vec::Vec};

/// semaphore structure
pub struct Semaphore {
    /// semaphore inner
    pub inner: UPSafeCell<SemaphoreInner>,
}

pub struct SemaphoreInner {
    pub count: isize,
    pub wait_queue: VecDeque<Arc<TaskControlBlock>>,
    /// Threads currently holding one unit of this semaphore (a multiset).
    pub holders: Vec<Arc<TaskControlBlock>>,
}

impl Semaphore {
    /// Create a new semaphore
    pub fn new(res_count: usize) -> Self {
        trace!("kernel: Semaphore::new");
        Self {
            inner: unsafe {
                UPSafeCell::new(SemaphoreInner {
                    count: res_count as isize,
                    wait_queue: VecDeque::new(),
                    holders: Vec::new(),
                })
            },
        }
    }

    /// up operation of semaphore
    pub fn up(&self) {
        trace!("kernel: Semaphore::up");
        let mut inner = self.inner.exclusive_access();
        inner.count += 1;
        // the caller releases one unit
        let caller = current_task().unwrap();
        if let Some(pos) = inner.holders.iter().position(|t| Arc::ptr_eq(t, &caller)) {
            inner.holders.remove(pos);
        }
        if inner.count <= 0 {
            if let Some(task) = inner.wait_queue.pop_front() {
                // the woken thread now holds the released unit
                inner.holders.push(task.clone());
                wakeup_task(task);
            }
        }
    }

    /// down operation of semaphore; return 0 on success. Deadlock detection is
    /// performed by the syscall layer before calling this method.
    pub fn down(&self) -> usize {
        trace!("kernel: Semaphore::down");
        let mut inner = self.inner.exclusive_access();
        inner.count -= 1;
        if inner.count < 0 {
            inner.wait_queue.push_back(current_task().unwrap());
            drop(inner);
            block_current_and_run_next();
        } else {
            inner.holders.push(current_task().unwrap());
        }
        0
    }
}
