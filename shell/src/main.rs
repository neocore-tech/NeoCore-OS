// shell/src/main.rs — ncsh (NeoCore Shell) — Standalone binary
// Digunakan saat user space siap (Phase 5+)
// Untuk sekarang, shell dijalankan langsung di dalam kernel (lihat kernel/src/shell/)
// SPDX-License-Identifier: MIT

fn main() {
    println!("ncsh — NeoCore Shell v0.1.0");
    println!("Standalone mode (user space — Phase 5+)");
    println!("Shell REPL saat ini berjalan langsung di dalam kernel.");
    println!("Ketik 'help' setelah boot QEMU.");
}
