// kernel/src/task/mod.rs — Task & Cooperative Executor
// Phase 1: Cooperative multitasking menggunakan Rust async/await
// SPDX-License-Identifier: MIT

pub mod executor;
pub mod scheduler;

extern crate alloc;

use alloc::boxed::Box;
use core::{
    future::Future,
    pin::Pin,
    sync::atomic::{AtomicU64, Ordering},
    task::{Context, Poll},
};

// ── Task ID ───────────────────────────────────────────────────────────────

/// ID unik untuk setiap task (atomic, monoton)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct TaskId(u64);

impl TaskId {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        TaskId(NEXT.fetch_add(1, Ordering::Relaxed))
    }
}

// ── Task State ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Running,
    Blocked,
    Finished,
}

// ── Task Struct ───────────────────────────────────────────────────────────

/// Sebuah async task yang bisa di-schedule oleh executor
pub struct Task {
    pub id:     TaskId,
    pub state:  TaskState,
    pub name:   &'static str,
    future: Pin<Box<dyn Future<Output = ()> + Send>>,
}

impl Task {
    /// Buat task baru dari async function/block
    pub fn new(name: &'static str, future: impl Future<Output = ()> + Send + 'static) -> Self {
        Task {
            id:     TaskId::new(),
            state:  TaskState::Ready,
            name,
            future: Box::pin(future),
        }
    }

    /// Poll task — kembalikan apakah sudah selesai
    pub fn poll(&mut self, cx: &mut Context) -> Poll<()> {
        self.state = TaskState::Running;
        let result = self.future.as_mut().poll(cx);
        if result.is_ready() {
            self.state = TaskState::Finished;
        } else {
            self.state = TaskState::Ready;
        }
        result
    }
}

impl core::fmt::Debug for Task {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Task")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("state", &self.state)
            .finish()
    }
}
