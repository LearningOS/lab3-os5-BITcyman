mod context;
mod switch;
mod pid;
mod manager;
mod processor;
#[allow(clippy::module_inception)]
mod task;

pub use switch::__switch;
pub use manager::{fetch_task, add_task};
pub use task::{TaskControlBlock, TaskStatus};
pub use pid::{PidHandle, KernelStack, pid_alloc};
pub use context::TaskContext;
pub use processor::{current_user_token, current_trap_cx, run_tasks, current_task, take_current_task, schedule};

use crate::loader::get_app_data_by_name;
use alloc::sync::Arc;
use lazy_static::*;


lazy_static! {
    pub static ref INITPROC: Arc<TaskControlBlock> = Arc::new(
        TaskControlBlock::new(get_app_data_by_name("ch5b_initproc").unwrap())
    );
}

pub fn add_initproc() {
    add_task(INITPROC.clone());
}

pub fn suspend_current_and_run_next() {
    let task = take_current_task().unwrap();

    let mut task_inner = task.inner_exclusive_access();
    let task_cx_ptr = &mut task_inner.task_cx as *mut TaskContext;

    task_inner.task_status = TaskStatus::Ready;
    drop(task_inner);

    add_task(task);
    schedule(task_cx_ptr);
}

pub fn exit_current_and_run_next(exit_code: i32) {
    let task = take_current_task().unwrap();
    let mut inner = task.inner_exclusive_access();
    inner.task_status = TaskStatus::Zombie;
    inner.exit_code = exit_code;

    {
        let mut initproc_inner = INITPROC.inner_exclusive_access();
        for child in inner.children.iter() {
            child.inner_exclusive_access().parent = Some(Arc::downgrade(&INITPROC));
            initproc_inner.children.push(child.clone());
        }
    }
    inner.children.clear();
    inner.memory_set.recycle_data_pages();
    drop(inner);
    drop(task);

    let mut _unused = TaskContext::zero_init();
    schedule(&mut _unused as *mut _);

}


