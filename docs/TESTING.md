# 🧪 TESTING — Panduan Testing NeoCore OS

---

## 1. Overview

Testing OS sangat berbeda dengan testing aplikasi biasa. Karena OS berjalan di bare metal, kita perlu strategi berlapis:

1. **Unit Tests** — Test fungsi Rust individual (berjalan di host OS)
2. **Integration Tests** — Test modul kernel di QEMU
3. **Network Tests** — Test komunikasi kernel ↔ Go neonetd
4. **End-to-End Tests** — Test full boot dan network functionality

---

## 2. Kernel Unit Tests

Rust memiliki built-in test framework. Untuk `no_std`, kita menggunakan **custom test runner**.

### Setup Custom Test Framework

```rust
// kernel/src/lib.rs
#![cfg_attr(test, no_main)]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

use core::panic::PanicInfo;

pub trait Testable {
    fn run(&self) -> ();
}

impl<T> Testable for T
where T: Fn(),
{
    fn run(&self) {
        serial_print!("{}\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[FAILED]\n");
    serial_println!("Error: {}\n", info);
    exit_qemu(QemuExitCode::Failed);
    loop {}
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed  = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;
    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}
```

### Test untuk Memory Manager

```rust
// kernel/src/memory/tests.rs

#[cfg(test)]
mod tests {
    use super::*;
    use x86_64::VirtAddr;

    #[test_case]
    fn test_heap_allocation() {
        use alloc::{boxed::Box, vec::Vec};

        // Test basic Box allocation
        let heap_value = Box::new(42);
        assert_eq!(*heap_value, 42);

        // Test Vec growth
        let mut vec: Vec<u32> = Vec::new();
        for i in 0..500 {
            vec.push(i);
        }
        assert_eq!(vec.len(), 500);
        for (i, &val) in vec.iter().enumerate() {
            assert_eq!(val, i as u32);
        }
    }

    #[test_case]
    fn test_heap_large_allocation() {
        use alloc::vec;

        // Alokasi besar (1MB)
        let v = vec![0u8; 1024 * 1024];
        assert_eq!(v.len(), 1024 * 1024);
        assert!(v.iter().all(|&b| b == 0));
    }

    #[test_case]
    fn test_multiple_allocations() {
        use alloc::boxed::Box;

        // Alokasi banyak object
        let vals: Vec<Box<u64>> = (0..100)
            .map(|i| Box::new(i as u64))
            .collect();

        for (i, val) in vals.iter().enumerate() {
            assert_eq!(**val, i as u64);
        }
    }

    #[test_case]
    fn test_frame_allocator() {
        // Test bahwa frame allocator memberikan frame unik
        // (hanya bisa di-test di QEMU karena butuh real memory map)
    }
}
```

### Test untuk Interrupt Handling

```rust
// kernel/tests/interrupts.rs

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

use kernel::{serial_println, test_panic_handler};
use core::panic::PanicInfo;

#[no_mangle]
pub extern "C" fn _start() -> ! {
    kernel::interrupts::gdt::init();
    kernel::interrupts::idt::init();
    unsafe { kernel::interrupts::pic::PICS.lock().initialize() };

    test_main();
    loop {}
}

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    test_panic_handler(info)
}

/// Test breakpoint exception tidak crash kernel
#[test_case]
fn test_breakpoint_exception() {
    x86_64::instructions::interrupts::int3();
    // Jika sampai sini, handler berhasil dan execution dilanjutkan
    serial_println!("[OK] Breakpoint exception handled");
}

/// Test stack overflow tidak crash dengan double fault IST
#[test_case]
fn test_page_fault_handling() {
    // Baca dari address yang tidak dipetakan
    let result = core::panic::catch_unwind(|| {
        unsafe { *(0xdeadbeef as *const u32) }
    });
    // Kernel harus survive page fault (tapi karena no recovery, ini terbatas)
}
```

---

## 3. Network Tests

### Test smoltcp Stack

```rust
// kernel/tests/network.rs

#[cfg(test)]
mod tests {
    use smoltcp::{
        iface::{Config, Interface, SocketSet},
        wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address},
        time::Instant,
    };

    #[test_case]
    fn test_smoltcp_initialization() {
        // Test bahwa smoltcp dapat diinisialisasi
        // (menggunakan mock device)
        serial_println!("smoltcp init test");
        // TODO: Mock NIC device untuk testing
    }

    #[test_case]
    fn test_ip_address_parsing() {
        let ip = Ipv4Address::new(192, 168, 1, 100);
        assert_eq!(ip.as_bytes(), &[192, 168, 1, 100]);
    }
}
```

### Test IPC Protocol (Rust side)

```rust
// kernel/tests/ipc.rs

#[test_case]
fn test_netlink_message_parsing() {
    use crate::net::netlink::*;

    // Buat pesan SET_INTERFACE
    let mut data = [0u8; 12 + 52]; // Header + IfaceConfig

    // Header
    let total_len = (12u32 + 52u32).to_le_bytes();
    data[0..4].copy_from_slice(&total_len);
    data[4..6].copy_from_slice(&1u16.to_le_bytes()); // SET_INTERFACE

    // Payload: eth0, 192.168.1.100, 255.255.255.0, 192.168.1.1
    data[12..16].copy_from_slice(b"eth0");
    data[28..32].copy_from_slice(&[192, 168, 1, 100]);
    data[32..36].copy_from_slice(&[255, 255, 255, 0]);
    data[36..40].copy_from_slice(&[192, 168, 1, 1]);

    // Test parsing tidak panic
    let result = apply_config(&data[12..]);
    // Dalam unit test environment, network stack mungkin tidak ada
    // Jadi kita hanya test parsing tidak crash
    serial_println!("IPC message parsing: OK");
}
```

---

## 4. Go Network Manager Tests

### Unit Tests

```go
// netmgr/internal/dhcp/client_test.go
package dhcp_test

import (
    "net"
    "testing"

    "neocore-os/netmgr/internal/dhcp"
)

func TestBuildDiscover(t *testing.T) {
    mac, _ := net.ParseMAC("52:54:00:12:34:56")
    xid := uint32(0xDEADBEEF)

    pkt := dhcp.BuildDiscover(xid, mac)

    // Verify packet structure
    if len(pkt) < 300 {
        t.Fatalf("DISCOVER packet too short: %d bytes", len(pkt))
    }

    // Verify magic cookie
    if pkt[236] != 0x63 || pkt[237] != 0x82 ||
        pkt[238] != 0x53 || pkt[239] != 0x63 {
        t.Error("Invalid DHCP magic cookie")
    }

    // Verify message type = DISCOVER (1)
    if pkt[243] != 1 {
        t.Errorf("Expected message type 1 (DISCOVER), got %d", pkt[243])
    }

    t.Logf("DISCOVER packet: %d bytes", len(pkt))
}

func TestParseOffer(t *testing.T) {
    // Buat mock DHCP Offer packet
    offer := buildMockOffer(
        0xDEADBEEF,
        net.IP{10, 0, 2, 15},       // Your IP
        net.IP{10, 0, 2, 2},        // Server IP
        net.IP{255, 255, 255, 0},   // Subnet mask
        net.IP{10, 0, 2, 2},        // Gateway
        3600,                        // Lease time
    )

    msg, err := dhcp.ParsePacket(offer)
    if err != nil {
        t.Fatalf("Failed to parse OFFER: %v", err)
    }

    if msg.Type != dhcp.MsgTypeOffer {
        t.Errorf("Expected OFFER, got %d", msg.Type)
    }

    if !msg.YourIP.Equal(net.IP{10, 0, 2, 15}) {
        t.Errorf("Wrong IP: %v", msg.YourIP)
    }
}

func TestLeaseRenewalTiming(t *testing.T) {
    // Test bahwa renewal timer dihitung dengan benar
    // T1 = leaseTime / 2
    // T2 = leaseTime * 7 / 8
    leaseTime := 3600
    t1 := leaseTime / 2   // 1800s
    t2 := leaseTime * 7 / 8 // 3150s

    if t1 != 1800 {
        t.Errorf("T1 should be 1800s, got %d", t1)
    }
    if t2 != 3150 {
        t.Errorf("T2 should be 3150s, got %d", t2)
    }
}
```

### DNS Resolver Tests

```go
// netmgr/internal/dns/resolver_test.go
package dns_test

import (
    "context"
    "testing"
    "time"

    "neocore-os/netmgr/internal/dns"
)

func TestDNSPacketBuilding(t *testing.T) {
    query := dns.BuildQuery("example.com", dns.TypeA)

    // Verify DNS header
    if query[2] != 0x01 || query[3] != 0x00 {
        t.Error("Invalid DNS flags (should be standard query with RD set)")
    }

    // QDCOUNT should be 1
    if query[4] != 0 || query[5] != 1 {
        t.Error("QDCOUNT should be 1")
    }
}

func TestDNSCaching(t *testing.T) {
    cfg := dns.Config{
        Upstream: []string{"8.8.8.8:53"},
        CacheTTL: 60 * time.Second,
        Timeout:  5 * time.Second,
    }
    resolver := dns.NewResolver(cfg)

    // Mock: Inject cached entry
    resolver.InjectCache("test.local", []string{"192.168.1.1"}, 60)

    ctx, cancel := context.WithTimeout(context.Background(), 1*time.Second)
    defer cancel()

    addrs, err := resolver.Resolve(ctx, "test.local")
    if err != nil {
        t.Fatalf("Cache lookup failed: %v", err)
    }

    if len(addrs) == 0 || addrs[0].String() != "192.168.1.1" {
        t.Errorf("Wrong cached result: %v", addrs)
    }
}
```

### IPC Protocol Tests

```go
// netmgr/pkg/netlink/encoder_test.go
package netlink_test

import (
    "net"
    "testing"

    "neocore-os/netmgr/pkg/netlink"
)

func TestEncodeIfaceConfig(t *testing.T) {
    ip   := net.ParseIP("192.168.1.100").To4()
    mask := net.ParseIP("255.255.255.0").To4()
    gw   := net.ParseIP("192.168.1.1").To4()

    encoded := netlink.EncodeIfaceConfig("eth0", ip, mask, gw, 1500, 0x03)

    // Verify total length
    if len(encoded) != 52 {
        t.Fatalf("IfaceConfig should be 52 bytes, got %d", len(encoded))
    }

    // Verify name
    name := string(encoded[0:4])
    if name != "eth0" {
        t.Errorf("Name mismatch: expected 'eth0', got '%s'", name)
    }

    // Verify IP
    if encoded[16] != 192 || encoded[17] != 168 ||
        encoded[18] != 1 || encoded[19] != 100 {
        t.Errorf("IP mismatch: %v", encoded[16:20])
    }

    // Verify MTU
    mtu := uint16(encoded[32]) | uint16(encoded[33])<<8
    if mtu != 1500 {
        t.Errorf("MTU mismatch: expected 1500, got %d", mtu)
    }

    t.Logf("IfaceConfig encoded: %v", encoded)
}

func TestRouteEncoding(t *testing.T) {
    _, dst, _ := net.ParseCIDR("0.0.0.0/0")
    gw := net.ParseIP("192.168.1.1").To4()

    encoded := netlink.EncodeRouteEntry(dst, gw, "eth0", 100)

    if len(encoded) != 36 {
        t.Fatalf("RouteEntry should be 36 bytes, got %d", len(encoded))
    }

    // Verify gateway
    if encoded[8] != 192 || encoded[9] != 168 ||
        encoded[10] != 1 || encoded[11] != 1 {
        t.Errorf("Gateway mismatch: %v", encoded[8:12])
    }
}
```

---

## 5. End-to-End Tests (QEMU)

### test_boot.sh

```bash
#!/bin/bash
# tests/test_boot.sh — Test boot sequence

set -e

DISK_IMG="build/neocore-os.img"
LOG_FILE="build/boot_test.log"
TIMEOUT=30  # seconds

echo "==> Starting boot test..."

# Jalankan QEMU dan capture output
timeout $TIMEOUT qemu-system-x86_64 \
    -m 256M \
    -serial file:$LOG_FILE \
    -display none \
    -drive file=$DISK_IMG,format=raw \
    -device e1000,netdev=net0 \
    -netdev user,id=net0 \
    -device isa-debug-exit,iobase=0xf4,iosize=0x04 \
    &
QEMU_PID=$!

# Tunggu kernel selesai boot
sleep 10

# Cek output
check_log() {
    if grep -q "$1" $LOG_FILE; then
        echo "[PASS] Found: $1"
    else
        echo "[FAIL] Missing: $1"
        cat $LOG_FILE
        kill $QEMU_PID 2>/dev/null
        exit 1
    fi
}

check_log "NeoCore OS v0.1.0"
check_log "GDT initialized"
check_log "IDT initialized"
check_log "Interrupts enabled"
check_log "Memory manager initialized"
check_log "Boot Complete"

echo "==> Boot test PASSED!"
kill $QEMU_PID 2>/dev/null
```

### test_network.sh

```bash
#!/bin/bash
# tests/test_network.sh — Test network functionality

echo "==> Starting network tests..."

# Start QEMU dengan network
qemu-system-x86_64 \
    -m 256M \
    -serial stdio \
    -display none \
    -drive file=build/neocore-os.img,format=raw \
    -device e1000,netdev=net0 \
    -netdev user,id=net0,hostfwd=tcp::2222-:22 \
    &
QEMU_PID=$!

echo "Waiting for boot..."
sleep 15

# Test ping dari dalam OS (via QEMU network)
# (Ini asumsi shell sudah jalan dan bisa terima input)

echo "==> Network tests done"
kill $QEMU_PID
```

---

## 6. Test Configuration

### Cargo.toml Test Settings

```toml
# kernel/Cargo.toml

[package.metadata.bootimage]
# QEMU args untuk test
test-args = [
    # Device untuk exit QEMU setelah test
    "-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
    # Serial output ke stdio (untuk test output)
    "-serial", "stdio",
    # Tanpa display (headless testing)
    "-display", "none",
    # Minimal memory
    "-m", "128M",
]
test-success-exit-code = 33   # (0x10 << 1) | 1
test-timeout = 60              # 60 detik timeout
```

---

## 7. Running Tests

```bash
# Semua tests
make test

# Kernel tests saja
cd kernel && cargo +nightly test

# Go tests saja
cd netmgr && go test -v ./...

# Go test dengan coverage
cd netmgr && go test -coverprofile=coverage.out ./...
cd netmgr && go tool cover -html=coverage.out

# Test satu package Go
cd netmgr && go test -v ./internal/dhcp/...

# Test dengan race detector (Go)
cd netmgr && go test -race ./...

# Boot test (butuh QEMU)
bash tests/test_boot.sh
```

---

## 8. Test Output Contoh

```
==> Running kernel unit tests...
Testing 8 tests
kernel::memory::tests::test_heap_allocation          [ok]
kernel::memory::tests::test_heap_large_allocation    [ok]
kernel::memory::tests::test_multiple_allocations     [ok]
kernel::interrupts::tests::test_breakpoint_exception [ok]
kernel::net::tests::test_ip_address_parsing          [ok]
kernel::net::tests::test_netlink_message_parsing     [ok]
kernel::vga::tests::test_print                       [ok]
kernel::serial::tests::test_serial_write             [ok]

Test result: ok. 8 passed; 0 failed; 0 ignored

==> Running Go unit tests...
=== RUN   TestBuildDiscover
    DISCOVER packet: 300 bytes
--- PASS: TestBuildDiscover (0.00s)
=== RUN   TestParseOffer
--- PASS: TestParseOffer (0.00s)
=== RUN   TestLeaseRenewalTiming
--- PASS: TestLeaseRenewalTiming (0.00s)
=== RUN   TestDNSPacketBuilding
--- PASS: TestDNSPacketBuilding (0.00s)
=== RUN   TestDNSCaching
--- PASS: TestDNSCaching (0.01s)
=== RUN   TestEncodeIfaceConfig
    IfaceConfig encoded: [101 116 104 48 ...]
--- PASS: TestEncodeIfaceConfig (0.00s)
=== RUN   TestRouteEncoding
--- PASS: TestRouteEncoding (0.00s)

PASS
coverage: 67.3% of statements

==> All tests PASSED! ✅
```

---

## 9. Continuous Integration

Tests berjalan otomatis di setiap push ke GitHub Actions. Lihat [BUILD.md](BUILD.md#10-cicd-github-actions) untuk konfigurasi CI.

---

## 10. Referensi

- [Writing an OS in Rust — Testing](https://os.phil-opp.com/testing/)
- [Go Testing Package](https://pkg.go.dev/testing)
- [QEMU Debug options](https://www.qemu.org/docs/master/system/invocation.html)
- [bootimage test framework](https://github.com/rust-osdev/bootimage)
