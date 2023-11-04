//! Process management syscalls
use core::mem::size_of;

use crate::{
    config::MAX_SYSCALL_NUM,
    task::{
        change_program_brk, exit_current_and_run_next, suspend_current_and_run_next, TaskStatus, get_current_syscall, current_user_token, memory_alloc, memory_dealloc
    }, timer::get_time_us, 
    mm::{translated_byte_buffer,MapPermission, VirtAddr},
};



#[repr(C)]
#[derive(Debug)]
pub struct TimeVal {
    pub sec: usize,
    pub usec: usize,
}

/// Task information
#[allow(dead_code)]
pub struct TaskInfo {
    /// Task status in it's life cycle
    status: TaskStatus,
    /// The numbers of syscall called by task
    syscall_times: [u32; MAX_SYSCALL_NUM],
    /// Total running time of task
    time: usize,
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
pub fn sys_get_time(_ts: *mut TimeVal, _tz: usize) -> isize {
    trace!("kernel: sys_get_time");
    
    let buffers = translated_byte_buffer(current_user_token(), _ts as *const u8, size_of::<TimeVal>());
    let us = get_time_us();

    let tv = TimeVal {
        sec: us / 1_000_000,
        usec: us % 1_000_000,
    };
    let mut tv_ptr= &tv as *const TimeVal as *const u8;
    for buffer in buffers {
        unsafe {
            
            tv_ptr.copy_to(buffer.as_ptr() as *mut u8, buffer.len());
            tv_ptr =tv_ptr.add(buffer.len());
        }
    }
    

    0
}

/// YOUR JOB: Finish sys_task_info to pass testcases
/// HINT: You might reimplement it with virtual memory management.
/// HINT: What if [`TaskInfo`] is splitted by two pages ?
pub fn sys_task_info(_ti: *mut TaskInfo) -> isize {
    trace!("kernel: sys_task_info");
    
    let mut buffers = translated_byte_buffer(current_user_token(), _ti as *const u8, size_of::<TaskInfo>());
    // let ti_ptr = _ti as *const TaskInfo as *const u8;

    let status  = TaskStatus::Running;
        
    let syscall_time = get_current_syscall();
        
    let time = get_time_us()/1000;
    
    // 灵感来自与 小队成员——肖炯 的交流， 交流内容： sys_task_info 函数的修改，访问内存地址
    let buffer_prt = buffers[0].as_mut_ptr() as *mut TaskInfo;

    unsafe {
        let taskinfo =  &mut *buffer_prt;
        taskinfo.status = status;
        taskinfo.syscall_times = syscall_time;
        taskinfo.time = time;
    };

    0
}

// YOUR JOB: Implement mmap.
pub fn sys_mmap(_start: usize, _len: usize, port: usize) -> isize {
    trace!("kernel: sys_mmap");
    // let current_token = current_user_token();
    
    // check permission
    if port == 0 || port > 7 {
        return -1;
    }


    let mut permission = MapPermission::from_bits((port as u8) << 1).unwrap();
    permission |= MapPermission::U;
    let start_va = VirtAddr::from(_start);
    let end_va = VirtAddr::from(_start+_len);     

    if start_va.aligned() == false{
        return -1;
    }


    let res =  memory_alloc(start_va, end_va, permission);
     
    res
}

// YOUR JOB: Implement munmap.
pub fn sys_munmap(_start: usize, _len: usize) -> isize {
    trace!("kernel: sys_munmap");
    let start_va = VirtAddr::from(_start);
    let end_va = VirtAddr::from(_start+_len);     

    if start_va.aligned() == false {
        return -1;
    }

    let result = memory_dealloc(start_va, end_va);
    result
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
