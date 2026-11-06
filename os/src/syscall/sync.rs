use crate::sync::{set_deadlock_detect, Condvar, Mutex, MutexBlocking, MutexSpin, Semaphore};
use crate::task::{block_current_and_run_next, current_process, current_task, TaskControlBlock};
use crate::timer::{add_timer, get_time_ms};
use alloc::sync::Arc;
use alloc::vec;
use alloc::vec::Vec;
/// sleep syscall
pub fn sys_sleep(ms: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_sleep",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let expire_ms = get_time_ms() + ms;
    let task = current_task().unwrap();
    add_timer(expire_ms, task);
    block_current_and_run_next();
    0
}
/// mutex create syscall
pub fn sys_mutex_create(blocking: bool) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mutex: Option<Arc<dyn Mutex>> = if !blocking {
        Some(Arc::new(MutexSpin::new()))
    } else {
        Some(Arc::new(MutexBlocking::new()))
    };
    let mut process_inner = process.inner_exclusive_access();
    if let Some(id) = process_inner
        .mutex_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.mutex_list[id] = mutex;
        id as isize
    } else {
        process_inner.mutex_list.push(mutex);
        process_inner.mutex_list.len() as isize - 1
    }
}
/// mutex lock syscall
pub fn sys_mutex_lock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_lock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.lock() as isize
}
/// mutex unlock syscall
pub fn sys_mutex_unlock(mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_mutex_unlock",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    mutex.unlock();
    0
}
/// semaphore create syscall
pub fn sys_semaphore_create(res_count: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .semaphore_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.semaphore_list[id] = Some(Arc::new(Semaphore::new(res_count)));
        id
    } else {
        process_inner
            .semaphore_list
            .push(Some(Arc::new(Semaphore::new(res_count))));
        process_inner.semaphore_list.len() - 1
    };
    id as isize
}
/// semaphore up syscall
pub fn sys_semaphore_up(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_up",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    sem.up();
    0
}
/// semaphore down syscall
pub fn sys_semaphore_down(sem_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_semaphore_down",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let sem = Arc::clone(process_inner.semaphore_list[sem_id].as_ref().unwrap());
    drop(process_inner);
    drop(process);
    // run the banker's deadlock detection before blocking
    if detect_semaphore_deadlock(sem_id) {
        return 0xdeadusize.wrapping_neg() as isize;
    }
    sem.down() as isize
}

/// Detect whether `semaphore_down(sem_id)` would cause a deadlock using the
/// banker's algorithm over the current process's semaphores and threads.
fn detect_semaphore_deadlock(sem_id: usize) -> bool {
    let process = current_process();
    // snapshot the semaphore list and thread list to avoid holding the lock
    let (sems, tasks, current) = {
        let inner = process.inner_exclusive_access();
        let sems: Vec<Option<Arc<Semaphore>>> = inner.semaphore_list.clone();
        let tasks: Vec<Arc<TaskControlBlock>> = inner
            .tasks
            .iter()
            .filter_map(|t| t.clone())
            .collect();
        (sems, tasks, current_task().unwrap())
    };
    let n_sem = sems.len();
    let n_thread = tasks.len();

    // gather available counts, holders and waiters for every semaphore
    let mut available = vec![0isize; n_sem];
    let mut holders: Vec<Vec<Arc<TaskControlBlock>>> = vec![Vec::new(); n_sem];
    let mut waiters: Vec<Vec<Arc<TaskControlBlock>>> = vec![Vec::new(); n_sem];
    for j in 0..n_sem {
        if let Some(sem) = &sems[j] {
            let inner = sem.inner.exclusive_access();
            // a negative count means waiting threads and no free unit
            available[j] = inner.count.max(0);
            holders[j] = inner.holders.clone();
            waiters[j] = inner.wait_queue.iter().cloned().collect();
        }
    }
    // if the semaphore still has a free unit, no blocking (thus no deadlock)
    if available[sem_id] > 0 {
        return false;
    }

    // build the Allocation and Need matrices (n_thread x n_sem)
    let mut allocation = vec![vec![0isize; n_sem]; n_thread];
    let mut need = vec![vec![0isize; n_sem]; n_thread];
    for j in 0..n_sem {
        for holder in &holders[j] {
            if let Some(i) = tasks.iter().position(|t| Arc::ptr_eq(t, holder)) {
                allocation[i][j] += 1;
            }
        }
        for waiter in &waiters[j] {
            if let Some(i) = tasks.iter().position(|t| Arc::ptr_eq(t, waiter)) {
                need[i][j] += 1;
            }
        }
    }
    // the current thread additionally requests one unit of `sem_id`
    if let Some(i) = tasks.iter().position(|t| Arc::ptr_eq(t, &current)) {
        need[i][sem_id] += 1;
    }

    // banker's safety check
    let mut work = available.clone();
    let mut finish = vec![false; n_thread];
    loop {
        let mut progressed = false;
        for i in 0..n_thread {
            if !finish[i] && (0..n_sem).all(|j| need[i][j] <= work[j]) {
                for j in 0..n_sem {
                    work[j] += allocation[i][j];
                }
                finish[i] = true;
                progressed = true;
            }
        }
        if !progressed {
            break;
        }
    }
    // unsafe state means a deadlock
    !finish.iter().all(|&f| f)
}
/// condvar create syscall
pub fn sys_condvar_create() -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_create",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let mut process_inner = process.inner_exclusive_access();
    let id = if let Some(id) = process_inner
        .condvar_list
        .iter()
        .enumerate()
        .find(|(_, item)| item.is_none())
        .map(|(id, _)| id)
    {
        process_inner.condvar_list[id] = Some(Arc::new(Condvar::new()));
        id
    } else {
        process_inner
            .condvar_list
            .push(Some(Arc::new(Condvar::new())));
        process_inner.condvar_list.len() - 1
    };
    id as isize
}
/// condvar signal syscall
pub fn sys_condvar_signal(condvar_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_signal",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    drop(process_inner);
    condvar.signal();
    0
}
/// condvar wait syscall
pub fn sys_condvar_wait(condvar_id: usize, mutex_id: usize) -> isize {
    trace!(
        "kernel:pid[{}] tid[{}] sys_condvar_wait",
        current_task().unwrap().process.upgrade().unwrap().getpid(),
        current_task()
            .unwrap()
            .inner_exclusive_access()
            .res
            .as_ref()
            .unwrap()
            .tid
    );
    let process = current_process();
    let process_inner = process.inner_exclusive_access();
    let condvar = Arc::clone(process_inner.condvar_list[condvar_id].as_ref().unwrap());
    let mutex = Arc::clone(process_inner.mutex_list[mutex_id].as_ref().unwrap());
    drop(process_inner);
    condvar.wait(mutex);
    0
}
/// enable deadlock detection syscall
///
/// `enabled` is 1 to enable deadlock detection, 0 to disable it.
pub fn sys_enable_deadlock_detect(enabled: usize) -> isize {
    trace!("kernel: sys_enable_deadlock_detect {}", enabled);
    if enabled != 0 && enabled != 1 {
        return -1;
    }
    set_deadlock_detect(enabled == 1);
    0
}
