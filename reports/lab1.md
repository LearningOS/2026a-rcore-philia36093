# Lab1 实验报告（ch3：多道程序与分时多任务）

## 一、功能总结

本章实现了系统调用 `sys_trace`（ID 410），用于追踪当前任务的系统调用历史信息。它根据 `trace_request` 提供三种功能：

- `trace_request = 0`：读取当前任务 `id` 地址处的一个字节；
- `trace_request = 1`：把一个字节 `data` 写入当前任务 `id` 地址处；
- `trace_request = 2`：返回当前任务调用编号为 `id` 的系统调用的次数（本次调用本身也计入统计）。

为此，我在 `TaskControlBlock` 中新增了 `syscall_times` 数组记录每个系统调用的次数，并在内核 `syscall` 分发入口处对每次系统调用（含 `sys_trace` 自身）进行计数。由于本章尚无地址空间隔离，读写直接以物理地址进行，未做安全检查。

## 二、简答作业

1. **正确进入 U 态后程序的特征**：U 态下执行特权指令（如 `sret`、`csrw satp` 等）会触发 Illegal Instruction 异常；访问 S 态寄存器（如 `sstatus`、`stvec`）也会触发异常，程序被内核终止。测试环境使用 RustSBI 0.4.1 作为 SBI 固件。

2. **`__alltraps` 与 `__restore` 的作用**：

   1. 刚进入 `__restore` 时，`sp` 指向内核栈上已分配的 `TrapContext` 的起始地址（低地址端）。`__restore` 有两种使用情景：一是 `trap_handler` 处理完陷阱后调用 `trap_return` 跳转到 `__restore` 恢复用户上下文；二是任务第一次被调度时，其 `TaskContext` 的 `ra` 被设为 `__restore`，通过 `__switch` 切到该任务后直接进入 `__restore` 进入用户态。

   2. 这几行汇编特殊处理了 `sstatus`、`sepc`、`sscratch` 三个 CSR：`sstatus` 的 SPP 位决定返回后的特权级，`sepc` 保存用户态返回后继续执行的 PC，`sscratch` 保存用户栈指针（`sp`）。

   3. 跳过了 `x2`（sp）和 `x4`（tp）：`sp` 在内核栈上已被改动，稍后通过 `csrrw sp, sscratch, sp` 一并恢复；`tp` 用户程序不使用。

   4. 执行 `csrrw sp, sscratch, sp` 后：`sp` 指向用户栈，`sscratch` 指向内核栈（`TrapContext` 顶部）。

   5. `__restore` 中发生状态切换的指令是 `sret`：它根据 `sstatus.SPP` 切换到 U 态，并跳转到 `sepc` 指向的用户代码。

   6. `__alltraps` 开头 `csrrw sp, sscratch, sp` 之后：`sp` 指向内核栈，`sscratch` 保存了用户栈指针。

   7. 从 U 态进入 S 态由用户程序的 `ecall` 指令（触发 Environment Call from U-mode 异常）完成，随后硬件跳转到 `stvec` 指向的 `__alltraps`。

## 三、荣誉准则

1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 **以下各位** 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

    *无*

2. 此外，我也参考了 **以下资料** ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

    *《rCore-Tutorial-Guide 实验指导》与《rCore-Tutorial-Book-v3》教材*

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。
