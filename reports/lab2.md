# 实验二：地址空间与系统调用

## 实现内容

本章使用每个任务自己的 `MemorySet` 管理用户地址空间。`sys_get_time` 将时间结构按字节写入用户缓冲区，支持结构体跨越两个页面；`sys_trace` 在读取或写入前检查页面是否对用户可见、是否具备相应权限。用户地址无效时返回 `-1`。

`sys_mmap` 检查起始地址的页对齐和 `prot` 的有效位，将非零长度向上取整到整页，并把 `prot` 转换为带 `U` 位的 `MapPermission`。任务管理模块通过当前任务的 `memory_set` 检查目标页是否空闲，成功后建立匿名映射。`sys_munmap` 检查待解除的页面是否全部已映射，再调用 `MemorySet` 的解除映射接口。当前 `unmap_range` 要求解除范围与一个 `MapArea` 完全一致；这是当前实现的范围限制。

## 第一版代码修改

在第一次集中修改时，主要完成了以下实现；随后编译发现的问题记录在下一节。

1. **用户指针转换与权限检查**：在 `mm/page_table.rs` 新增检查用户读、写权限的缓冲区转换函数，逐页查询页表项，检查 `U` 位及所需的 `R` 或 `W` 位，并返回分段切片；在 `mm/mod.rs` 将这些函数公开。`sys_get_time` 使用写检查后的分段切片复制 `TimeVal` 的字节，支持跨页；`sys_trace` 分别使用读、写检查处理请求 0、1，请求 2 查询当前任务的系统调用次数。
2. **`mmap` 参数与权限转换**：在 `sys_mmap` 中检查 `start` 页对齐、`prot` 仅使用低三位且不为零；`len == 0` 直接成功返回 `0`。对于非零长度，使用 `checked_add` 计算终点，向上取整到页边界，再把 `prot` 的三个位分别转换成 `MapPermission::R/W/X`，同时加上用户可访问位 `U`。
3. **当前任务的映射入口**：在 `task/mod.rs` 新增当前任务 `mmap`、`munmap` 的调用接口，从 `TaskManagerInner.current_task` 找到对应的 `TaskControlBlock.memory_set`。`mmap` 先检查范围是否空闲，再调用 `insert_framed_area` 建立映射，成功返回 `0`。
4. **范围检查与解除映射**：在 `mm/memory_set.rs` 中用虚拟页号实现 `is_range_free` 和 `is_range_mapped`；前者检查新区间不与已有区域重叠，后者逐页确认解除映射范围均已映射。`unmap_range` 找到边界完全相同的区域，从 `areas` 中移除，并通过 `MapArea::unmap` 解除页表映射。
5. **`munmap` 参数处理**：在 `sys_munmap` 中检查 `start` 页对齐、非零 `len`、地址相加是否溢出，并将范围末端向上取整；然后调用当前任务的解除映射入口，成功返回 `0`，错误返回 `-1`。

第一版集中修复的改动还可按检查点归纳如下：

- `sys_get_time`：按页检查目标缓冲区的用户可见性和可写权限，将时间结构体逐段写入，处理跨页情况。
- `sys_trace`：读取时检查用户可读权限，写入时检查用户可写权限；非法地址或权限不足返回 `isize` 的 `-1`。
- `mmap`：转换 `prot` 到 `MapPermission`，检查起始地址页对齐、长度加法溢出和权限位，按页向上取整；允许 `len == 0` 并返回 `0`。
- `munmap`：补充起始地址、长度和终点地址的参数检查，并检查目标页是否已映射。
- `MemorySet`：补充 `is_range_free`、`is_range_mapped` 和 `unmap_range`，用于检查区间及解除映射。
- 修正 `currnet`、`umap`、`inner.task` 等拼写错误，并补充 `syscall`、`task`、`mm` 模块间所需的导入。

## 编译错误及修正记录

实现过程中，编译器依次暴露了以下问题，均已在源码中修正：

1. `task/mod.rs` 中的 `use mm::...` 在 Rust 2018 模块路径规则下无法解析，改为 `use crate::mm::...`；随后移除未使用的 `MemorySet` 导入，以满足 `#![deny(warnings)]`。
2. `syscall/mod.rs` 调用 `add_current_task_syscall_number` 时缺少导入，补上来自 `crate::task` 的导入。
3. `task/task.rs` 初始化计数数组时将 `syscall_counts_init` 误写为 `syscall_count_init`，统一变量名；访问任务控制块时改用 `&mut inner.tasks[current_idx]`，避免移动任务控制块。
4. `mm/page_table.rs` 把 `VirtAddr::from` 误写为 `VirtAddr::From`，修正大小写；此前还曾将返回 `Vec` 的 `translated_byte_buffer` 写成在溢出分支返回 `None`，造成返回类型不匹配，现已恢复与函数签名一致的写法。可返回 `Option` 的用户缓冲区检查函数使用 `checked_add`，并在页面不存在时返回 `None`。
5. `syscall/process.rs` 的 `sys_trace` 写入分支需要修改返回的缓冲区，匹配结果改为 `Some(mut dst)`。
6. `mm/memory_set.rs` 中的 `currnet`、`umap` 和 `task/mod.rs` 中的 `inner.task` 等拼写错误分别改为 `current`、`unmap` 和 `inner.tasks`。
7. 内核启用了 `#![deny(missing_docs)]`。为新增的公开范围检查与解除映射方法、用户页面检查函数、任务映射接口及系统调用计数字段补充 `///` 文档注释。

## 验证结果

在项目的 QEMU 8.2.2 容器中，内核可以编译并启动。第 4 章检查器的功能断言结果为 `16/16`。首次完整测试命令在报告检查阶段提示缺少上一章的 `reports/lab1.md`；该文件随后从 `ch3` 分支已提交的实验一报告恢复。尚未在恢复报告后重新执行完整测试命令。

### ai工具使用情况

使用ai工具进行代码的辅助审查和编译错误更改整理，文档编写
