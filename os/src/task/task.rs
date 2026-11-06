//! Types related to task management
use super::TaskContext;
use crate::config::{MAX_SYSCALL_NUM, PAGE_SIZE, TRAP_CONTEXT_BASE};
use crate::mm::{
    kernel_stack_position, MapPermission, MemorySet, PhysPageNum, VirtAddr, VPNRange, KERNEL_SPACE,
};
use crate::trap::{trap_handler, TrapContext};

/// The task control block (TCB) of a task.
pub struct TaskControlBlock {
    /// Save task context
    pub task_cx: TaskContext,

    /// Maintain the execution status of the current process
    pub task_status: TaskStatus,

    /// Application address space
    pub memory_set: MemorySet,

    /// The phys page number of trap context
    pub trap_cx_ppn: PhysPageNum,

    /// The size(top addr) of program which is loaded from elf file
    pub base_size: usize,

    /// Heap bottom
    pub heap_bottom: usize,

    /// Program break
    pub program_brk: usize,

    /// Count how many times each syscall has been invoked by this task.
    pub syscall_times: [u32; MAX_SYSCALL_NUM],
}

impl TaskControlBlock {
    /// get the trap context
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    /// get the user token
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    /// Based on the elf info in program, build the contents of task in a new address space
    pub fn new(elf_data: &[u8], app_id: usize) -> Self {
        // memory_set with elf program headers/trampoline/trap context/user stack
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT_BASE).into())
            .unwrap()
            .ppn();
        let task_status = TaskStatus::Ready;
        // map a kernel-stack in kernel space
        let (kernel_stack_bottom, kernel_stack_top) = kernel_stack_position(app_id);
        KERNEL_SPACE.exclusive_access().insert_framed_area(
            kernel_stack_bottom.into(),
            kernel_stack_top.into(),
            MapPermission::R | MapPermission::W,
        );
        let task_control_block = Self {
            task_status,
            task_cx: TaskContext::goto_trap_return(kernel_stack_top),
            memory_set,
            trap_cx_ppn,
            base_size: user_sp,
            heap_bottom: user_sp,
            program_brk: user_sp,
            syscall_times: [0; MAX_SYSCALL_NUM],
        };
        // prepare TrapContext in user space
        let trap_cx = task_control_block.get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }
    /// change the location of the program break. return None if failed.
    pub fn change_program_brk(&mut self, size: i32) -> Option<usize> {
        let old_break = self.program_brk;
        let new_brk = self.program_brk as isize + size as isize;
        if new_brk < self.heap_bottom as isize {
            return None;
        }
        let result = if size < 0 {
            self.memory_set
                .shrink_to(VirtAddr(self.heap_bottom), VirtAddr(new_brk as usize))
        } else {
            self.memory_set
                .append_to(VirtAddr(self.heap_bottom), VirtAddr(new_brk as usize))
        };
        if result {
            self.program_brk = new_brk as usize;
            Some(old_break)
        } else {
            None
        }
    }
    /// map an anonymous memory region [start, start + len) with the given
    /// `prot` (bit 0 = readable, bit 1 = writable, bit 2 = executable).
    pub fn mmap(&mut self, start: usize, len: usize, prot: usize) -> isize {
        // start must be page-aligned
        if start % PAGE_SIZE != 0 {
            return -1;
        }
        // prot: only the low 3 bits are valid and at least one must be set
        if prot & !0x7 != 0 || prot & 0x7 == 0 {
            return -1;
        }
        let map_perm =
            MapPermission::from_bits(((prot & 0x7) << 1) as u8).unwrap() | MapPermission::U;
        let mmap_start = VirtAddr::from(start);
        let mmap_end = VirtAddr::from(start + len);
        // the region must not overlap with any existing area
        if self
            .memory_set
            .overlap_with(VPNRange::new(mmap_start.floor(), mmap_end.ceil()))
        {
            return -1;
        }
        self.memory_set.insert_framed_area(mmap_start, mmap_end, map_perm);
        0
    }
    /// unmap the anonymous memory region starting at `start`.
    pub fn munmap(&mut self, start: usize, len: usize) -> isize {
        if start % PAGE_SIZE != 0 || len == 0 {
            return -1;
        }
        let start_va = VirtAddr::from(start);
        let end_vpn = VirtAddr::from(start + len).ceil();
        // the area must start exactly at `start` and cover [start, start + len)
        if !self.memory_set.area_covers(start_va, end_vpn) {
            return -1;
        }
        if self.memory_set.remove_area_with_start(start_va) {
            0
        } else {
            -1
        }
    }
}

#[derive(Copy, Clone, PartialEq)]
/// task status: UnInit, Ready, Running, Exited
pub enum TaskStatus {
    /// uninitialized
    UnInit,
    /// ready to run
    Ready,
    /// running
    Running,
    /// exited
    Exited,
}
