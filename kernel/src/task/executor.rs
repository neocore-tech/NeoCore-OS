// kernel/src/task/executor.rs — Cooperative Task Executor
// Menjalankan semua async tasks hingga selesai (cooperative scheduling)
// SPDX-License-Identifier: MIT

use crate::serial_println;

extern crate alloc;

use alloc::{collections::BTreeMap, sync::Arc, task::Wake};
use core::task::{Context, Poll, Waker};
use spinning_top::Spinlock;

use super::{Task, TaskId, TaskState};

// ── Waker ─────────────────────────────────────────────────────────────────

/// Waker sederhana yang hanya menandai task sebagai "perlu di-poll lagi"
struct TaskWaker {
    id:       TaskId,
    wake_queue: Arc<Spinlock<alloc::vec::Vec<TaskId>>>,
}

impl Wake for TaskWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.wake_queue.lock().push(self.id);
    }
}

// ── Executor ──────────────────────────────────────────────────────────────

/// Cooperative executor — menjalankan semua tasks secara round-robin
/// Tasks yang `Pending` akan di-re-poll saat waker-nya dipanggil
pub struct Executor {
    tasks:      BTreeMap<TaskId, Task>,
    wake_queue: Arc<Spinlock<alloc::vec::Vec<TaskId>>>,
}

impl Executor {
    pub fn new() -> Self {
        Executor {
            tasks:      BTreeMap::new(),
            wake_queue: Arc::new(Spinlock::new(alloc::vec::Vec::new())),
        }
    }

    /// Tambahkan task baru ke executor
    pub fn spawn(&mut self, task: Task) {
        let id = task.id;
        serial_println!("[EXEC ] Spawned task {:?} '{}'", id, task.name);
        if self.tasks.insert(id, task).is_some() {
            panic!("Task ID collision: {:?}", id);
        }
        // Langsung masukkan ke wake queue agar di-poll di run pertama
        self.wake_queue.lock().push(id);
    }

    /// Buat Waker untuk task dengan id tertentu
    fn make_waker(&self, id: TaskId) -> Waker {
        Waker::from(Arc::new(TaskWaker {
            id,
            wake_queue: self.wake_queue.clone(),
        }))
    }

    /// Jalankan satu iterasi — poll semua task yang ada di wake queue
    /// Return jumlah task yang berhasil di-poll
    pub fn run_once(&mut self) -> usize {
        // Ambil semua IDs dari wake queue
        let ids: alloc::vec::Vec<TaskId> = {
            let mut q = self.wake_queue.lock();
            core::mem::take(&mut *q)
        };

        let mut polled = 0;

        for id in ids {
            // Cek apakah task ada dan belum selesai
            if !self.tasks.contains_key(&id) {
                continue;
            }
            if self.tasks.get(&id).map(|t| t.state) == Some(TaskState::Finished) {
                continue;
            }

            // Buat waker SEBELUM mutable borrow pada task
            let waker = self.make_waker(id);
            let mut cx = Context::from_waker(&waker);

            let task = match self.tasks.get_mut(&id) {
                Some(t) => t,
                None    => continue,
            };

            match task.poll(&mut cx) {
                Poll::Ready(()) => {
                    serial_println!("[EXEC ] Task {:?} '{}' finished", id, task.name);
                    self.tasks.remove(&id);
                }
                Poll::Pending => {}
            }
            polled += 1;
        }

        polled
    }

    /// Jalankan executor sampai semua tasks selesai (blocking loop)
    pub fn run(&mut self) {
        serial_println!("[EXEC ] Starting executor with {} tasks", self.tasks.len());

        loop {
            self.run_once();

            if self.tasks.is_empty() {
                serial_println!("[EXEC ] All tasks finished.");
                break;
            }

            // Idle — tunggu interrupt (hemat CPU)
            x86_64::instructions::interrupts::enable_and_hlt();
        }
    }

    /// Jumlah task aktif
    pub fn task_count(&self) -> usize {
        self.tasks.len()
    }
}

impl Default for Executor {
    fn default() -> Self {
        Self::new()
    }
}
