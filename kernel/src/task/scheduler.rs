// kernel/src/task/scheduler.rs — Scheduler Interface
// Phase 1: Counter & bookkeeping; Phase 5: Preemptive Round-Robin
// SPDX-License-Identifier: MIT

use crate::serial_println;

use core::sync::atomic::{AtomicU64, Ordering};

// ── Tick Counter ──────────────────────────────────────────────────────────

/// Jumlah timer tick yang sudah terjadi sejak scheduler init
static SCHED_TICKS: AtomicU64 = AtomicU64::new(0);

/// Dipanggil dari timer_handler (IRQ 0) setiap 1 ms
pub fn tick() {
    SCHED_TICKS.fetch_add(1, Ordering::Relaxed);
    // TODO Phase 5: Periksa apakah quantum task saat ini habis,
    //               jika ya — set flag preemption, context switch saat
    //               kembali dari interrupt handler
}

/// Baca jumlah scheduler ticks
pub fn ticks() -> u64 {
    SCHED_TICKS.load(Ordering::Relaxed)
}

// ── Stats ─────────────────────────────────────────────────────────────────

pub struct SchedStats {
    pub ticks:       u64,
    pub uptime_secs: u64,
}

pub fn stats() -> SchedStats {
    let t = ticks();
    SchedStats {
        ticks:       t,
        uptime_secs: t / 1_000, // 1000 Hz
    }
}

// ── Init ──────────────────────────────────────────────────────────────────

pub fn init() {
    serial_println!("[SCHED] Cooperative scheduler initialized (Phase 1)");
    serial_println!("[SCHED] Preemptive Round-Robin: Phase 5");
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_stats_not_crash() {
        let s = stats();
        // Hanya verifikasi tidak crash
        assert!(s.ticks >= 0);
    }
}
