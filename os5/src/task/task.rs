use super::{PidHandle, KernelStack, TaskContext};

use crate::mm::{PhysPageNum, MemorySet};
use crate::sync::UPSafeCell;


pub struct TaskControlBlock {
    pub pid: PidHandle,
    pub kernel_stack: KernelStack,
    inner: UPSafeCell<TaskControlBlockInner>,
}

impl TaskControlBlock {
    pub fn inner_exclusive_access(&self) -> RefMut<'_, TaskControlBlockInner> {
        self.inner.exclusive_access()
    }
    pub fn getpid(&self) -> usize {
        self.pid.0
    }
    pub fn new(elf_data: &[u8]) -> Self {
        //...
    }
    pub fn exec(&self, elf_data: &[u8]) {
        //...
    }
    pub fn fork(self: &Arc<TaskControlBlock>) -> Arc<TaskControlBlock> {
        //...
    }
}

pub struct TaskControlBlockInner {
    pub trap_cx_ppn: PhysPageNum,
    pub base_size: usize,
    pub task_cx: TaskContext,
    pub task_status: TaskStatus,
    pub memory_set: MemorySet,
    pub parent: Option<Weak<TaskControlBlock>>,
    pub children: Vec<Arc<TaskControlBlock>>,
    pub exit_code: i32,
}

impl TaskControlBlockInner {
    pub fn get_current_trap_cx(&self) -> &'static mut TrapContext {
        self.trap_cx_ppn.get_mut()
    }
    pub fn get_user_token(&self) -> usize {
        self.memory_set.token()
    }
    fn get_status(&self) -> TaskStatus {
        self.task_status
    }
    pub fn is_zombie(&self) -> bool {
        self.get_status() == TaskStatus::Zombie
    }
}

pub enum TaskStatus {
    UnInit,
    Ready,
    Running,
    Zombie,
}