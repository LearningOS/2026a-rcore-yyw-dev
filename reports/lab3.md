# 实验三：进程创建与调度

## 实验环境与目标

在 `ch5` 分支使用 Rust `nightly-2024-05-02`、`riscv64gc-unknown-none-elf` 目标和项目提供的 QEMU 容器。实验目标是将上一章的时间与映射系统调用适配到独立进程地址空间，实现 `spawn` 创建子进程，并探索按优先级分配 CPU 的 stride 调度。

## 进程创建

内核系统调用表将编号 `400` 分发到 `sys_spawn`。系统调用读取调用者地址空间中的以零结尾程序名，通过 `get_app_data_by_name` 查找内置 ELF。找不到程序时返回 `-1`；找到后由当前任务的 `TaskControlBlock::spawn` 创建新任务，取得 PID，将任务加入就绪队列并返回 PID。

`TaskControlBlock::spawn` 使用目标 ELF 构建新的 `MemorySet`，分配 PID 和内核栈，设置父进程弱引用，并把子任务加入父进程的 `children`。新任务的 trap context 使用目标程序入口、用户栈顶和新内核栈顶初始化；`heap_bottom`、`program_brk` 使用新程序的 `user_sp`。创建函数返回 `Arc<TaskControlBlock>`，由 syscall 层负责把任务交给调度器。这一步直接创建新地址空间与新任务，不需要先复制父进程地址空间再执行 `exec`。

当前实现依赖 `translated_str` 读取程序名。它对非法用户指针使用 `unwrap()`，因此本实现对“程序名不存在”返回 `-1`，但尚未保证非法指针也安全返回 `-1`。

## 上一章系统调用的迁移

- `sys_get_time` 将微秒时间拆成秒与微秒，逐页检查目标用户缓冲区的 `U/R/W` 权限，再分段复制 `TimeVal`，覆盖结构体跨页的情况。
- `sys_mmap` 校验页对齐、权限位和地址运算溢出，把长度向上取整到整页，在当前进程的 `memory_set` 中检查空闲页并建立带用户权限的映射。
- `sys_munmap` 校验参数，检查待解除范围的页面已映射，并在当前进程的 `memory_set` 中解除映射。`unmap_range` 目前要求范围恰好等于一个 `MapArea`。

迁移时发现 `is_range_free` 把 `PageTable::translate(vpn).is_some()` 当作已占用判断。解除映射会清空叶子页表项，但上级页表仍在，`translate` 可返回 `Some(无效叶子项)`，导致解除后再次映射误报冲突。将判断改为检查页表项的有效位后，`ch4_unmap` 能通过。

本章不再要求维护 `sys_trace`，当前内核系统调用分发没有接入该调用。

## 调度算法的当前状态

任务控制块已有 `priority` 和 `stride` 字段，新任务默认优先级为 `16`、步长为 `0`。`TaskManager::stride_fetch` 可以从就绪队列中找出最小 `stride` 的任务，并按 `BIGSTRIDE / priority` 增加其步长。`sys_set_priority` 对小于 `2` 的参数返回 `-1`，对有效参数更新当前任务优先级并返回新值。

但是当前 `fetch_task()` 仍调用 FIFO 的 `fetch()`，所以 `stride_fetch()` 尚未接入实际调度路径。用户程序的 `set_priority` 返回值测试能够通过，却不能据此认定 stride 调度已经生效。运行久后新任务初始步长为 `0` 以及 `usize` 累加溢出，也需要进一步评估。

## 验证记录

在现有容器中构建内核并启动 QEMU，交互运行 `ch5_usertest`。观察到 `get_time OK!`、`Test 04_5 ummap OK!`、`Test 04_6 ummap2 OK!`、`Test spawn0 OK!`、`Test wait OK!`、`Test waitpid OK!`、`Test set_priority OK!`，相关正常子进程退出码为 `0`。`ch4_mmap1` 和 `ch4_mmap2` 的 `-2` 是预期的访问异常。测试程序最后输出 `ch5 Usertests passed!`。

这次运行的是用户程序测试，不是官方完整检查器；尚无官方 `N/N` 结论。`ch5_stride` 虽然以退出码 `0` 结束，但优先级 `5` 至 `10` 的 `ratio` 从约 `588` 万下降到 `305` 万，和当前仍使用 FIFO 的代码一致，不能作为 stride 调度通过的证据。

## 工具使用

使用 AI 工具辅助检查实现、分析映射故障、运行编译与 QEMU 测试，并整理本报告。最终代码与报告内容仍需结合实验要求审阅。
