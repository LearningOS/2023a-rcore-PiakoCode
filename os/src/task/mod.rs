//! Task management implementation
//!
//! Everything about task management, like starting and switching tasks is
//! implemented here.
//!
//! A single global instance of [`TaskManager`] called `TASK_MANAGER` controls
//! all the tasks in the whole operating system.
//!
//! A single global instance of [`Processor`] called `PROCESSOR` monitors running
//! task(s) for each core.
//!
//! A single global instance of `PID_ALLOCATOR` allocates pid for user apps.
//!
//! Be careful when you see `__switch` ASM function in `switch.S`. Control flow around this function
//! might not be what you expect.
mod context;
mod id;
mod manager;
mod processor;
mod switch;
#[allow(clippy::module_inception)]
mod task;

use crate::{
    config::{MAX_SYSCALL_NUM, BIG_STRIDE},
    loader::get_app_data_by_name,
    mm::{translated_str, MapPermission, VirtAddr},
};
use alloc::sync::Arc;
use lazy_static::*;
pub use manager::{fetch_task, TaskManager};
use switch::__switch;
pub use task::{TaskControlBlock, TaskStatus};

pub use context::TaskContext;
pub use id::{kstack_alloc, pid_alloc, KernelStack, PidHandle};
pub use manager::add_task;
pub use processor::{
    current_task, current_trap_cx, current_user_token, run_tasks, schedule, take_current_task,
    Processor,
};
/// Suspend the current 'Running' task and run the next task in task list.
pub fn suspend_current_and_run_next() {
    // There must be an application running.
    let task = take_current_task().unwrap();

    // ---- access current TCB exclusively
    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;

    task_inner.stride = task_inner.stride + BIG_STRIDE / task_inner.priority; 
    // println!("stride {}",task_inner.stride);
    // Change status to Ready
    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);
    // ---- release current PCB

    // push back to ready queue.
    add_task(task);
    // jump to scheduling cycle
    schedule(task_cx_ptr);
}

/// pid of usertests app in make run TEST=1
pub const IDLE_PID: usize = 0;

/// Exit the current 'Running' task and run the next task in task list.
pub fn exit_current_and_run_next(exit_code: i32) {
    // take from Processor
    let task = take_current_task().unwrap();

    let pid = task.getpid();
    if pid == IDLE_PID {
        println!(
            "[kernel] Idle process exit with exit_code {} ...",
            exit_code
        );
        panic!("All applications completed!");
    }

    // **** access current TCB exclusively
    let mut inner = task.inner_exclusive_access();
    // Change status to Zombie
    inner.task_status = TaskStatus::Zombie;
    // Record exit code
    inner.exit_code = exit_code;
    // do not move to its parent but under initproc

    // ++++++ access initproc TCB exclusively
    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }
    // ++++++ release parent PCB

    inner.children.clear();
    // deallocate user space
    inner.memory_set.recycle_data_pages();
    drop(inner);
    // **** release current PCB
    // drop task manually to maintain rc correctly
    drop(task);
    // we do not have to save task context
    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);
}

lazy_static! {
    /// Creation of initial process
    ///
    /// the name "initproc" may be changed to any other app name like "usertests",
    /// but we have user_shell, so we don't need to change it.
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new(TaskControlBlock::new(
        get_app_data_by_name("ch5b_initproc").unwrap()
    ));
}

///Add init process to the manager
pub fn add_initproc() {
    add_task(INITPROC.clone());
}




/// get the start time of current task
pub fn task_start_time() -> usize {
    current_task().unwrap().inner_exclusive_access().start_time
}

/// Update syscall times
pub fn update_syscall(syscall_id: usize) {
    let current_task = current_task().unwrap();
    current_task.inner_exclusive_access().syscall_times[syscall_id] += 1;
}

/// get current syscall times
pub fn get_current_syscall() -> [u32; MAX_SYSCALL_NUM] {
    let curent_task = current_task().unwrap();
    let syscall = curent_task.inner_exclusive_access().syscall_times.clone();
    return syscall;
}

/// memory_alloc
pub fn memory_alloc(start_va: VirtAddr, end_va: VirtAddr, permission: MapPermission) -> isize {
    let binding = current_task().unwrap();
    let memset = &mut binding.inner_exclusive_access().memory_set;

    // println!("alloc start_va{}", start_va.floor().0);

    if memset.is_mapped(start_va, end_va) {
        return -1;
    }

    memset.insert_framed_area(start_va, end_va, permission);
    0
}

/// memory dealloc
pub fn memory_dealloc(start_va: VirtAddr, end_va: VirtAddr) -> isize {
    let binding = current_task().unwrap();

    let memset = &mut binding.inner_exclusive_access().memory_set;

    let res = memset.delete_framed_area(start_va, end_va);

    return res;
}

/// implement spawn
pub fn new_spawn(_path: *const u8) -> isize {
    let token = current_user_token();
    let name = translated_str(token, _path);
    let current_task = current_task().unwrap();
    match get_app_data_by_name(&name) {
        Some(elf) => {
            let spawn_program = Arc::new(TaskControlBlock::new(elf));
            // 添加到父进程的child中
            current_task
                .inner_exclusive_access()
                .children
                .push(spawn_program.clone());
            let pid = spawn_program.pid.0;

            add_task(spawn_program);
            return pid as isize;
        }
        None => return -1,
    }
}

