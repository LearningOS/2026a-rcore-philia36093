# Lab3 实验报告（ch5：进程管理）

## 一、功能总结

本章完成了以下工作：

- **迁移 `sys_get_time` / `sys_mmap` / `sys_munmap`**：适配基于 `Arc<TaskControlBlock>` 的新进程结构，通过 `inner_exclusive_access` 访问地址空间；`sys_get_time` 将 `TimeVal` 两个字段分别翻译写入以处理跨页情况；
- **实现 `sys_spawn`**：根据文件名从 ELF 数据构建全新地址空间的新进程（不复制父进程地址空间），建立父子关系后加入调度器，返回子进程 pid；
- **实现 `sys_set_priority`**：设置当前进程优先级（要求 ≥ 2），并据此更新 stride 调度的步长 `pass`；
- **实现 stride 调度**：在 TCB 中新增 `stride` 与 `pass` 字段，调度时在就绪队列中选取 `stride` 最小的进程，并在其被调度后将 `stride` 加上 `pass`。

## 二、问答作业：stride 算法深入

1. **两个 pass = 10 的进程，8bit 存储 stride，p1.stride = 255，p2.stride = 250，p2 执行一个时间片后轮到谁？**

   使用 8 位无符号存储，p2.stride = 250 + 10 = 260，发生溢出回绕为 4。此时 p2.stride = 4 < p1.stride = 255，因此**实际轮到的仍然是 p2**，而不是 p1。这就是 stride 算法的“反转”现象。

2. **为什么优先级全部 ≥ 2 时（不考虑溢出），STRIDE_MAX − STRIDE_MIN ≤ BigStride / 2？**

   因为优先级 ≥ 2，故 `pass = BigStride / priority ≤ BigStride / 2`。每调度一个进程，其 stride 至多增加 `BigStride / 2`。严格按算法执行时，任意两个进程的 stride 之差不会超过最大的 `pass`（即 `BigStride / 2`），否则 stride 小的进程必然先被调度，从而缩小差距。

3. **考虑溢出的比较器 `partial_cmp`**：

   用 `wrapping_sub` 计算两个 stride 的“向前距离” d = self − other（模 2ⁿ）。若 d = 0 则相等；若 d < 2ⁿ⁻¹（即小于半个周期），说明 self 在 other 之后且未回绕，self 更大；否则 self 更小：

   ```rust
   use core::cmp::Ordering;

   struct Stride(u64);

   impl PartialOrd for Stride {
       fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
           let d = self.0.wrapping_sub(other.0);
           if d == 0 {
               Some(Ordering::Equal)
           } else if d < (1u64 << 63) {
               Some(Ordering::Greater)
           } else {
               Some(Ordering::Less)
           }
       }
   }

   impl PartialEq for Stride {
       fn eq(&self, other: &Self) -> bool {
           false
       }
   }
   ```

## 三、荣誉准则

1. 在完成本次实验的过程（含此前学习的过程）中，我曾分别与 **以下各位** 就（与本次实验相关的）以下方面做过交流，还在代码中对应的位置以注释形式记录了具体的交流对象及内容：

    *无*

2. 此外，我也参考了 **以下资料** ，还在代码中对应的位置以注释形式记录了具体的参考来源及内容：

    *《rCore-Tutorial-Guide 实验指导》与《rCore-Tutorial-Book-v3》教材*

3. 我独立完成了本次实验除以上方面之外的所有工作，包括代码与文档。我清楚地知道，从以上方面获得的信息在一定程度上降低了实验难度，可能会影响起评分。

4. 我从未使用过他人的代码，不管是原封不动地复制，还是经过了某些等价转换。我未曾也不会向他人（含此后各届同学）复制或公开我的实验代码，我有义务妥善保管好它们。我提交至本实验的评测系统的代码，均无意于破坏或妨碍任何计算机系统的正常运转。我清楚地知道，以上情况均为本课程纪律所禁止，若违反，对应的实验成绩将按“-100”分计。
