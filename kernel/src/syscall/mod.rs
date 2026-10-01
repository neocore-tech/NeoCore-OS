// kernel/src/syscall/mod.rs — System Call Interface (Placeholder)
// Akan diisi lengkap saat user space siap (Phase 5+)

use crate::serial_println;

/// Syscall numbers NeoCore OS
pub mod nr {
    pub const READ:      u64 = 0;
    pub const WRITE:     u64 = 1;
    pub const OPEN:      u64 = 2;
    pub const CLOSE:     u64 = 3;
    pub const FORK:      u64 = 10;
    pub const EXEC:      u64 = 11;
    pub const EXIT:      u64 = 12;
    pub const GETPID:    u64 = 13;
    pub const SOCKET:    u64 = 20;
    pub const BIND:      u64 = 21;
    pub const CONNECT:   u64 = 22;
    pub const SEND:      u64 = 23;
    pub const RECV:      u64 = 24;
    pub const NETCONFIG: u64 = 30; // Custom: konfigurasi network
    pub const NETROUTE:  u64 = 31; // Custom: routing table
    pub const NETFILTER: u64 = 32; // Custom: firewall rules
}

/// Dispatch syscall berdasarkan nomor di RAX
/// (dipanggil dari IDT handler INT 0x80)
pub fn dispatch() {
    // TODO Phase 5: Implementasi syscall dispatch
    // Baca RAX (syscall number) dan argumen dari RDI, RSI, RDX, R10, R8, R9
    serial_println!("[SYSCALL] dispatch called (not implemented yet)");
}
