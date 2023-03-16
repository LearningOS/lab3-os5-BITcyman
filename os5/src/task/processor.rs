
use alloc::sync::Arc;
use super::{TaskContext, TaskControlBlock, __switch, fetch_task, TaskStatus};
use crate::trap::TrapContext;
use crate::sync::UPSafeCell;
use crate::mm::{VirtAddr, MapPermission};
use crate::syscall::TaskInfo;
use crate::timer::get_time_us;
use lazy_static::*;

lazy_static! {
    pub static ref PROCESSOR: UPSafeCell<Processor> = unsafe {
        UPSafeCell::new(Processor::new())
    };
}

pub struct Processor {
    current: Option<Arc<TaskControlBlock>>,
    idle_task_cx: TaskContext,
}

impl Processor {
    pub fn new() -> Self {
        Self {
            current: None,
            idle_task_cx: TaskContext::zero_init(),
        }
    }
    fn get_idle_task_cx_ptr(&mut self) -> *mut TaskContext {
        &mut self.idle_task_cx as *mut _
    }
    pub fn take_current(&mut self) -> Option<Arc<TaskControlBlock>>{
        self.current.take()
    }
    pub fn current(& self) -> Option<Arc<TaskControlBlock>> {
        self.current.as_ref().map(|task| Arc::clone(task))
    }
}

pub fn take_current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().take_current()
}

pub fn current_task() -> Option<Arc<TaskControlBlock>> {
    PROCESSOR.exclusive_access().current()
}

pub fn current_user_token() -> usize {
    let task = current_task().unwrap();
    let token = task.inner_exclusive_access().get_user_token();
    token
}

pub fn current_trap_cx() -> &'static mut TrapContext {
    current_task().unwrap().inner_exclusive_access().get_trap_cx()
}



pub fn current_mmap(start: VirtAddr, len: usize, perm: MapPermission) -> isize {
    current_task().unwrap().inner_exclusive_access().memory_set.mmap(start, len, perm)
}

pub fn current_munmap(start: VirtAddr, len: usize) -> isize {
    current_task().unwrap().inner_exclusive_access().memory_set.munmap(start, len)
}


pub fn get_current_task_info(ti: *mut TaskInfo) -> isize {
    current_task().unwrap().inner_exclusive_access().get_task_info(ti)
}

pub fn increase_current_task_syscall(syscall_id: usize) {
    current_task().unwrap().inner_exclusive_access().increase_task_syscall(syscall_id);
}



pub fn run_tasks() {
    loop {
        let mut processor = PROCESSOR.exclusive_access();
        if let Some(task) = fetch_task() {
            let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
            
            let mut task_inner = task.inner_exclusive_access();
            let next_task_cx_ptr = &task_inner.task_cx as *const TaskContext;

            task_inner.task_status = TaskStatus::Running;
            if task_inner.start_time == 0 {
                task_inner.start_time = get_time_us();
            }
            drop(task_inner);
            processor.current = Some(task);

            drop(processor);
            unsafe {
                __switch(
                    idle_task_cx_ptr,
                    next_task_cx_ptr,
                );
            }
        }
    }
}


pub fn schedule(switched_task_cx_ptr: *mut TaskContext) {
    let mut processor = PROCESSOR.exclusive_access();
    let idle_task_cx_ptr = processor.get_idle_task_cx_ptr();
    drop(processor);
    unsafe {
        __switch(
            switched_task_cx_ptr,
            idle_task_cx_ptr,
        );
    }
}