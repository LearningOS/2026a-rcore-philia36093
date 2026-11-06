//! Process management syscalls
use crate::mm::{PageTable, PTEFlags, VirtAddr};
use crate::task::{
    change_program_brk, current_user_token, exit_current_and_run_next, get_syscall_times,
    mmap_current, munmap_current, suspend_current_and_run_next,
};
use crate::timer::get_time_us;

#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// task exits and submit an exit code
pub fn sys_exit(_exit_code: i32) -> ! {
    trace!("kernel: sys_exit");
    exit_current_and_run_next();
    panic!("Unreachable in sys_exit!");
}

/// current task gives up resources for other tasks
pub fn sys_yield() -> isize {
    trace!("kernel: sys_yield");
    suspend_current_and_run_next();
    0
}

/// Write a single `usize` value to a user virtual address through the page
/// table, checking that the page is present, user-accessible and writable.
fn write_usize(page_table: &PageTable, va: VirtAddr, value: usize) -> bool {
    if let Some(pte) = page_table.translate(va.floor()) {
        if pte.is_valid() && pte.writable() && pte.flags().contains(PTEFlags::U) {
            let offset = va.page_offset();
            let page = pte.ppn().get_bytes_array();
            page[offset..offset + core::mem::size_of::<usize>()]
                .copy_from_slice(&value.to_ne_bytes());
            return true;
        }
    }
    false
}

/// get time with second and microsecond
///
/// The two `usize` fields are translated and written separately, so a
/// [`TimeVal`] split across two pages is handled correctly.
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    let us = get_time_us();
    let token = current_user_token();
    let page_table = PageTable::from_token(token);
    let sec = us / 1_000_000;
    let usec = us % 1_000_000;
    let sec_va = VirtAddr::from(ts as usize);
    let usec_va = VirtAddr::from(ts as usize + core::mem::size_of::<usize>());
    if write_usize(&page_table, sec_va, sec) && write_usize(&page_table, usec_va, usec) {
        0
    } else {
        -1
    }
}

/// trace syscall: read a byte, write a byte or count syscall invocations of
/// the current task, depending on `trace_request`. Addresses are translated
/// through the page table and checked for user visibility and permissions.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    trace!("kernel: sys_trace");
    let token = current_user_token();
    let page_table = PageTable::from_token(token);
    match trace_request {
        0 => {
            // read a byte at virtual address `id`
            let va = VirtAddr::from(id);
            match page_table.translate(va.floor()) {
                Some(pte)
                    if pte.is_valid()
                        && pte.readable()
                        && pte.flags().contains(PTEFlags::U) =>
                {
                    pte.ppn().get_bytes_array()[va.page_offset()] as isize
                }
                _ => -1,
            }
        }
        1 => {
            // write a byte to virtual address `id`
            let va = VirtAddr::from(id);
            match page_table.translate(va.floor()) {
                Some(pte)
                    if pte.is_valid()
                        && pte.writable()
                        && pte.flags().contains(PTEFlags::U) =>
                {
                    pte.ppn().get_bytes_array()[va.page_offset()] = data as u8;
                    0
                }
                _ => -1,
            }
        }
        2 => get_syscall_times(id) as isize,
        _ => -1,
    }
}

/// map an anonymous memory region [start, start + len) with the given prot.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap START={:#x}, LEN={:#x}, PORT={:#x}", start, len, port);
    mmap_current(start, len, port)
}

/// unmap an anonymous memory region starting at `start`.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    trace!("kernel: sys_munmap START={:#x}, LEN={:#x}", start, len);
    munmap_current(start, len)
}

/// change data segment size
pub fn sys_sbrk(size: i32) -> isize {
    trace!("kernel: sys_sbrk");
    if let Some(old_brk) = change_program_brk(size) {
        old_brk as isize
    } else {
        -1
    }
}
