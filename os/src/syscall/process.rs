//! Process management syscalls
use crate::config::PAGE_SIZE;
use crate::mm::{
    check_user_read_translated_byte_buffer, check_user_write_translated_byte_buffer,
    MapPermission, VirtAddr,
};
use crate::task::{
    change_program_brk, current_user_token, exit_current_and_run_next,
    get_current_task_syscall_number, mmap_current_task_memory_set,
    munmap_current_task_memory_set, suspend_current_and_run_next,
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

/// YOUR JOB: get time with second and microsecond
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TimeVal`] is splitted by two pages ?
pub fn sys_get_time(ts: *mut TimeVal, _tz: usize) -> isize {
    let token = current_user_token(); //现在用户空间的token

    //获取当前的系统时间并且写为时间结构体
    let time_us = get_time_us();
    let time_val: TimeVal = TimeVal {
        sec: time_us / 1_000_000,
        usec: time_us % 1_000_000,
    };

    //将需要传入进去的时间结构体转化为&[u8],便于写到用户传入的指针处
    let src = unsafe {
        core::slice::from_raw_parts(
            &time_val as *const TimeVal as *const u8,
            core::mem::size_of::<TimeVal>(),
        )
    };

    //将需要写入的指针转化静态（使用page_table的对外开放函数，translated_byte_buffer(),对跨页数据指针也生效）可变为&[u8]切片，便于写入
    if let Some(dst) = check_user_write_translated_byte_buffer(token, ts as *const u8, core::mem::size_of::<TimeVal>()) {
        //将数据传入
        let mut copied = 0;
        for chunk in dst {
            let n = chunk.len().min(src.len() - copied);
            chunk[..n].copy_from_slice(&src[copied..copied + n]);
            copied += n;
            if copied >= src.len() {
            break;
            }
        }

        0
    } else {
        -1
    }
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    let token = current_user_token(); //获取当前用户空间的token
    match trace_request {
        0 => {
            if let Some(dst) = check_user_read_translated_byte_buffer(token, id as *const u8, 1) {
                dst[0][0] as usize as isize
            } else {
                -1
            }
        },
        1 => {
            if let Some(mut dst) = check_user_write_translated_byte_buffer(token, id as *mut u8, 1) {
                let data = data as u8;
                dst[0][0] = data;
                0
            } else {
                -1
            }
        },
        2 => {
            get_current_task_syscall_number(id) as isize
        },
        _ => {
            -1
        }
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(start: usize, len: usize, port: usize) -> isize {
    if start % PAGE_SIZE != 0 {
        return -1;
    }

    if port & !0x7 != 0 || port & 0x7 == 0 {
        return -1;
    }

    // len == 0 is valid and maps an empty range.
    if len == 0 {
        return 0;
    }

    let end = match start.checked_add(len) {
        Some(end) => end,
        None => return -1,
    };
    let end = match end.checked_add(PAGE_SIZE - 1) {
        Some(end) => end / PAGE_SIZE * PAGE_SIZE,
        None => return -1,
    };

    let start_va = VirtAddr::from(start);
    let end_va = VirtAddr::from(end);

    //获得mappermission
    let mut permission = MapPermission::U;

    if port & 0x1 != 0 {
        permission |= MapPermission::R;
    }

    if port & 0x2 != 0 {
        permission |= MapPermission::W;
    }

    if port & 0x4 != 0 {
        permission |= MapPermission::X;
    }

    mmap_current_task_memory_set(start_va, end_va, permission)
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(start: usize, len: usize) -> isize {
    if start % PAGE_SIZE != 0 || len == 0 {
        return -1;
    }

    let end = match start.checked_add(len) {
        Some(end) => end,
        None => return -1,
    };
    let end = match end.checked_add(PAGE_SIZE - 1) {
        Some(end) => end / PAGE_SIZE * PAGE_SIZE,
        None => return -1,
    };

    let start_va = VirtAddr::from(start);
    let end_va = VirtAddr::from(end);
    munmap_current_task_memory_set(start_va, end_va)
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
