// kernel/src/net/netlink.rs — IPC Channel: Kernel ↔ Go neonetd
// Placeholder untuk Phase 3

use crate::serial_println;

/// Tipe pesan IPC antara kernel dan neonetd
#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MsgType {
    SetInterface = 0x0001,
    GetInterface = 0x0002,
    AddRoute     = 0x0003,
    DelRoute     = 0x0004,
    GetRoutes    = 0x0005,
    AddFilter    = 0x0006,
    DelFilter    = 0x0007,
    GetStats     = 0x0008,
    SetDns       = 0x0009,
    SendUdp      = 0x0014,
    RecvUdp      = 0x0015,
    Ack          = 0x00FF,
}

/// Header setiap pesan IPC (12 bytes)
#[repr(C, packed)]
pub struct Header {
    pub length:  u32,       // Total panjang (header + payload)
    pub msg_type: u16,
    pub flags:   u16,
    pub seq:     u32,       // Sequence number
}

/// Terapkan konfigurasi interface dari neonetd
/// (placeholder — Phase 3)
pub fn apply_config(_data: &[u8]) -> u64 {
    // TODO Phase 3
    serial_println!("[NETLINK] apply_config called (not implemented yet)");
    0
}

/// Terapkan routing entry dari neonetd
pub fn apply_route(_data: &[u8]) -> u64 {
    // TODO Phase 3
    serial_println!("[NETLINK] apply_route called (not implemented yet)");
    0
}
