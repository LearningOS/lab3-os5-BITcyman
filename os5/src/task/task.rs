use super::{PidHandle, pid_alloc, KernelStack, TaskContext};
use super::manager::{PRIORITY_INIT, PASS_INIT, BIG_STRIDE};
use crate::config::{TRAP_CONTEXT, MAX_SYSCALL_NUM};
use crate::trap::{TrapContext, trap_handler};
use crate::mm::{PhysPageNum, MemorySet, VirtAddr, KERNEL_SPACE};
use crate::sync::UPSafeCell;
use crate::syscall::TaskInfo;
use crate::timer::get_time_us;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use alloc::sync::{Arc, Weak};
use core::cell::RefMut;
use core::cmp::Ordering;

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
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();

        let task_control_block = Self {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe { UPSafeCell::new(TaskControlBlockInner {
                trap_cx_ppn,
                base_size: user_sp,
                task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                task_status: TaskStatus::Ready,
                memory_set,
                parent: None,
                children: Vec::new(),
                exit_code: 0,
                syscall_times: BTreeMap::new(),
                start_time: 0,
                priority: PRIORITY_INIT,
                pass: PASS_INIT,
                stride: 0,
            })},
        };

        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
    }
    pub fn exec(&self, elf_data: &[u8]) {
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        let mut inner = self.inner_exclusive_access();
        inner.memory_set = memory_set;
        inner.trap_cx_ppn = trap_cx_ppn;
        let trap_cx = inner.get_trap_cx();
        * trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            self.kernel_stack.get_top(),
            trap_handler as usize,
        );
    }
    pub fn fork(self: &Arc<TaskControlBlock>) -> Arc<TaskControlBlock> {
        let mut parent_inner = self.inner_exclusive_access();
        let memory_set = MemorySet::from_existed_user(
            &parent_inner.memory_set
        );
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        
        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();
        let task_control_block = Arc::new(TaskControlBlock {
            pid: pid_handle,
            kernel_stack,
            inner: unsafe { UPSafeCell::new(TaskControlBlockInner{
                trap_cx_ppn,
                base_size: parent_inner.base_size,
                task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                task_status: TaskStatus::Ready,
                memory_set,
                parent: Some(Arc::downgrade(self)),
                children: Vec::new(),
                exit_code: 0,
                syscall_times: parent_inner.syscall_times.clone(),
                start_time: parent_inner.start_time,
                priority: parent_inner.priority,
                pass: parent_inner.pass,
                stride: parent_inner.stride,
            })},
        });
        parent_inner.children.push(task_control_block.clone());

        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        trap_cx.kernel_sp = kernel_stack_top;
        
        task_control_block
    }

    pub fn spawn(self: &Arc<TaskControlBlock>, elf_data: &[u8]) -> Arc<TaskControlBlock> {
        let mut parent_inner = self.inner_exclusive_access();
        let (memory_set, user_sp, entry_point) = MemorySet::from_elf(elf_data);
        let trap_cx_ppn = memory_set
            .translate(VirtAddr::from(TRAP_CONTEXT).into())
            .unwrap()
            .ppn();
        let pid_handle = pid_alloc();
        let kernel_stack = KernelStack::new(&pid_handle);
        let kernel_stack_top = kernel_stack.get_top();

        let task_control_block = Arc::new(TaskControlBlock{ 
            pid: pid_handle,
            kernel_stack,
            inner: unsafe { UPSafeCell::new(TaskControlBlockInner {
                trap_cx_ppn,
                base_size: user_sp,
                task_cx: TaskContext::goto_trap_return(kernel_stack_top),
                task_status: TaskStatus::Ready,
                memory_set,
                parent: Some(Arc::downgrade(self)),
                children: Vec::new(),
                exit_code: 0,
                syscall_times: BTreeMap::new(),
                start_time: 0,
                priority: PRIORITY_INIT,
                pass: PASS_INIT,
                stride: 0,
            })},
        });
        parent_inner.children.push(task_control_block.clone());
        let trap_cx = task_control_block.inner_exclusive_access().get_trap_cx();
        *trap_cx = TrapContext::app_init_context(
            entry_point,
            user_sp,
            KERNEL_SPACE.exclusive_access().token(),
            kernel_stack_top,
            trap_handler as usize,
        );
        task_control_block
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
    pub syscall_times: BTreeMap<u16, u32>,
    pub start_time: usize,
    pub priority: usize,
    pub pass: usize,
    pub stride: usize,
}

impl TaskControlBlockInner {
    pub fn get_trap_cx(&self) -> &'static mut TrapContext {
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
    pub fn get_task_info(&self, ti: *mut TaskInfo) -> isize {
        let mut count = [0u32; MAX_SYSCALL_NUM];
        for (key, val) in self.syscall_times.iter() {
            count[*key as usize] = *val;
        }
        unsafe{
            *ti = TaskInfo{
                status: self.task_status,
                syscall_times: count,
                time: (get_time_us() - self.start_time) / 1000,
            }
        }
        0
    }
    pub fn increase_task_syscall(&mut self, syscall_id: usize) {
        let count = self.syscall_times.entry(syscall_id as u16).or_insert(0);
        *count += 1;
    }
    pub fn add_stride(&mut self){
        self.stride += BIG_STRIDE / self.priority;
    }
}

#[derive(Copy, Clone, PartialEq)]
pub enum TaskStatus {
    UnInit,
    Ready,
    Running,
    Zombie,
}


impl PartialEq for TaskControlBlock {
    fn eq(&self, other: &Self) -> bool {
        let my_inner = self.inner_exclusive_access();
        let other_inner = other.inner_exclusive_access();
        my_inner.stride == other_inner.stride
    }
}

impl PartialOrd for TaskControlBlock {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for TaskControlBlock {}

impl Ord for TaskControlBlock {
    fn cmp(&self, other: &Self) -> Ordering {
        // Reverse order for min heap
        let my_inner = self.inner_exclusive_access();
        let other_inner = other.inner_exclusive_access();
        // my_inner.stride.cmp(&other_inner.stride)
        match my_inner.stride.cmp(&other_inner.stride) {
            Ordering::Less => Ordering::Greater,
            Ordering::Equal => Ordering::Equal,
            Ordering::Greater => Ordering::Less,
        }
    }
}