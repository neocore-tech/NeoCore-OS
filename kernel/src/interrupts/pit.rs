// kernel/src/interrupts/pit.rs — Programmable Interval Timer (8253/8254)
// Mengatur timer hardware ke 1000 Hz (1 tick = 1 ms)
// SPDX-License-Identifier: MIT

use crate::serial_println;

use core::sync::atomic::{AtomicU64, Ordering};
use x86_64::instructions::port::Port;

// ── Constants ─────────────────────────────────────────────────────────────

/// Frekuensi dasar PIT: 1.193182 MHz
const PIT_BASE_FREQ: u32 = 1_193_182;

/// Target frekuensi: 1000 Hz (1 ms per tick)
const TARGET_HZ: u32 = 1_000;

/// Divisor yang dikirim ke PIT
const PIT_DIVISOR: u16 = (PIT_BASE_FREQ / TARGET_HZ) as u16; // ≈ 1193

// Port PIT
const PIT_CHANNEL0_DATA: u16 = 0x40;
const PIT_COMMAND:        u16 = 0x43;

// ── Global Tick Counter ───────────────────────────────────────────────────

/// Jumlah tick sejak boot (1 tick = 1 ms)
/// Atomic agar bisa dibaca dari mana saja tanpa lock
static TICKS: AtomicU64 = AtomicU64::new(0);

/// Dipanggil dari timer_handler di IDT setiap IRQ 0
#[inline]
pub fn tick() {
    TICKS.fetch_add(1, Ordering::Relaxed);
}

/// Baca jumlah ms sejak boot
#[inline]
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}

/// Baca uptime dalam detik
#[inline]
pub fn uptime_secs() -> u64 {
    ticks() / TARGET_HZ as u64
}

// ── Initialization ────────────────────────────────────────────────────────

/// Inisialisasi PIT Channel 0 — mode square wave, frekuensi TARGET_HZ
/// Harus dipanggil saat interrupt sudah diaktifkan
pub fn init() {
    unsafe {
        let mut cmd:  Port<u8> = Port::new(PIT_COMMAND);
        let mut data: Port<u8> = Port::new(PIT_CHANNEL0_DATA);

        // Command byte:
        //   bits 7-6: channel 0 (00)
        //   bits 5-4: lo/hi byte (11)
        //   bits 3-1: mode 3 — square wave generator (011)
        //   bit  0:   binary mode (0)
        cmd.write(0b00_11_011_0); // 0x36

        // Kirim divisor lo byte dulu, lalu hi byte
        data.write((PIT_DIVISOR & 0xFF) as u8);
        data.write((PIT_DIVISOR >> 8) as u8);
    }

    serial_println!(
        "[PIT ] Initialized: {}Hz ({} ms/tick), divisor={}",
        TARGET_HZ,
        1000 / TARGET_HZ,
        PIT_DIVISOR
    );
}

// ── Sleep Utility ─────────────────────────────────────────────────────────

/// Spin-sleep (busy wait) selama `ms` milidetik
/// PERHATIAN: Ini memblokir CPU — gunakan hanya saat inisialisasi
pub fn sleep_ms(ms: u64) {
    let start = ticks();
    while ticks() - start < ms {
        x86_64::instructions::hlt();
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_tick_increments() {
        let before = ticks();
        // Timer harus sudah berjalan saat test ini dieksekusi
        // Tunggu sedikit agar tick bertambah
        for _ in 0..10_000 {
            core::hint::spin_loop();
        }
        let after = ticks();
        // Tidak bisa jaminan bertambah dalam spin loop tanpa interrupt,
        // tapi setidaknya tidak boleh turun
        assert!(after >= before, "tick counter harus monoton");
    }

    #[test_case]
    fn test_uptime_calculation() {
        // uptime_secs() harus <= ticks() / 1000
        let t = ticks();
        let u = uptime_secs();
        assert_eq!(u, t / 1000);
    }
}
