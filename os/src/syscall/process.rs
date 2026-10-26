//! Process management syscalls
use crate::config::PAGE_SIZE;
use crate::mm::{translated_byte_buffer, PageTable, PTEFlags, VirtAddr};
use crate::task::{
    change_program_brk, current_user_token, exit_current_and_run_next,
    suspend_current_and_run_next,
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
    let len = core::mem::size_of::<TimeVal>();
    let start = ts as usize;
    let Some(end) = start.checked_add(len) else {
        return -1;
    };
    if start == 0 || end > (1usize << 38) {
        return -1;
    }

    let token = current_user_token();
    let page_table = PageTable::from_token(token);
    let mut page_start = start;
    while page_start < end {
        let vpn = VirtAddr::from(page_start).floor();
        let Some(pte) = page_table.translate(vpn) else {
            return -1;
        };
        if !pte.is_valid() || !pte.writable() || !pte.flags().contains(PTEFlags::U) {
            return -1;
        }
        page_start = ((page_start / PAGE_SIZE) + 1) * PAGE_SIZE;
    }

    let time_us = get_time_us();
    let time_val = TimeVal {
        sec: time_us / 1_000_000,
        usec: time_us % 1_000_000,
    };

    let mut src = [0; core::mem::size_of::<TimeVal>()];
    let (sec, usec) = src.split_at_mut(core::mem::size_of::<usize>());
    sec.copy_from_slice(&time_val.sec.to_ne_bytes());
    usec.copy_from_slice(&time_val.usec.to_ne_bytes());

    let mut offset = 0;
    for chunk in translated_byte_buffer(token, ts as *const u8, len) {
        let next_offset = offset + chunk.len();
        chunk.copy_from_slice(&src[offset..next_offset]);
        offset = next_offset;
    }
    0
}

/// TODO: Finish sys_trace to pass testcases
/// HINT: You might reimplement it with virtual memory management.
pub fn sys_trace(trace_request: usize, id: usize, data: usize) -> isize {
    match trace_request {
        0 => {
            let token = current_user_token();
            let mut buf = translated_byte_buffer(token, trace_request as *cont u8, 1);
            if buf.is_empty() {
                
            }
        },
        1 => {
            
        },
        2 => {

        }
    }
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, _port: usize) -> isize {
    trace!("kernel: sys_mmap NOT IMPLEMENTED YET!");
    -1
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap NOT IMPLEMENTED YET!");
    -1
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
