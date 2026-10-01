# 📐 ARCHITECTURE — Arsitektur Sistem NeoCore OS

---

## 1. Gambaran Umum

NeoCore OS menggunakan pendekatan **hybrid architecture** di mana:

- **Rust** menangani kernel space: bare metal, memory safety, zero GC overhead
- **Go** menangani network management di user space: goroutines, concurrency

Filosofi ini terinspirasi dari:

- **Linux** (monolithic kernel + user space daemons)
- **Redox OS** (Rust-based OS)
- **NetBSD** (clean network stack separation)

---

## 2. Diagram Arsitektur Lengkap

```
╔══════════════════════════════════════════════════════════════╗
║                        HARDWARE                              ║
║   CPU (x86_64) │ RAM │ NIC (Ethernet) │ Keyboard │ VGA      ║
╚══════════════════════════════════════════════════════════════╝
                           │
╔══════════════════════════════════════════════════════════════╗
║                      BOOTLOADER (Rust)                       ║
║   • UEFI/BIOS → Protected Mode → Long Mode (64-bit)          ║
║   • Setup GDT, stack, paging dasar                           ║
║   • Load kernel dari disk                                     ║
╚══════════════════════════════════════════════════════════════╝
                           │
╔══════════════════════════════════════════════════════════════╗
║                    KERNEL SPACE (Rust)                       ║
║                                                              ║
║  ┌─────────────────────────────────────────────────────┐    ║
║  │               CORE SUBSYSTEMS                        │    ║
║  │  ┌───────────┐  ┌───────────┐  ┌─────────────────┐  │    ║
║  │  │  Memory   │  │ Interrupt │  │    Scheduler     │  │    ║
║  │  │  Manager  │  │  Handler  │  │  (Round-Robin)   │  │    ║
║  │  │  (Paging) │  │ (GDT/IDT) │  │  Preemptive      │  │    ║
║  │  └───────────┘  └───────────┘  └─────────────────┘  │    ║
║  └─────────────────────────────────────────────────────┘    ║
║                                                              ║
║  ┌─────────────────────────────────────────────────────┐    ║
║  │           NETWORK SUBSYSTEM (Rust + smoltcp)         │    ║
║  │  ┌──────────┐  ┌──────────┐  ┌──────────────────┐   │    ║
║  │  │ Ethernet │  │  TCP/IP  │  │   Socket Layer   │   │    ║
║  │  │  Driver  │  │  Stack   │  │   (Syscall API)  │   │    ║
║  │  │ (VirtIO/ │  │(smoltcp) │  │                  │   │    ║
║  │  │  e1000)  │  │          │  │                  │   │    ║
║  │  └──────────┘  └──────────┘  └──────────────────┘   │    ║
║  │                                                       │    ║
║  │  ┌────────────────────────────────────────────────┐  │    ║
║  │  │           NETLINK IPC Layer                    │  │    ║
║  │  │   Kernel ↔ Go Network Manager communication    │  │    ║
║  │  └────────────────────────────────────────────────┘  │    ║
║  └─────────────────────────────────────────────────────┘    ║
║                                                              ║
║  ┌─────────────────────────────────────────────────────┐    ║
║  │              HARDWARE DRIVERS                        │    ║
║  │  ┌──────────┐  ┌──────────┐  ┌────────────────┐    │    ║
║  │  │ Keyboard │  │   VGA    │  │   Serial/UART  │    │    ║
║  │  │  (PS/2)  │  │  Driver  │  │   (Debugging)  │    │    ║
║  │  └──────────┘  └──────────┘  └────────────────┘    │    ║
║  └─────────────────────────────────────────────────────┘    ║
║                                                              ║
║  ┌─────────────────────────────────────────────────────┐    ║
║  │              SYSCALL INTERFACE                       │    ║
║  │   read │ write │ open │ close │ fork │ exec          │    ║
║  │   socket │ bind │ connect │ send │ recv │ netconfig  │    ║
║  └─────────────────────────────────────────────────────┘    ║
╚══════════════════════════════════════════════════════════════╝
                    │ Syscall / IPC
╔══════════════════════════════════════════════════════════════╗
║                    USER SPACE                                ║
║                                                              ║
║  ┌──────────────────────────────────────────────────────┐   ║
║  │         NETWORK MANAGER DAEMON (Go)  neonetd          │   ║
║  │                                                        │   ║
║  │  ┌──────────┐ ┌──────────┐ ┌─────────┐ ┌─────────┐  │   ║
║  │  │  DHCP    │ │   DNS    │ │Routing  │ │Firewall │  │   ║
║  │  │  Client  │ │Resolver  │ │Manager  │ │ Rules   │  │   ║
║  │  └──────────┘ └──────────┘ └─────────┘ └─────────┘  │   ║
║  │  ┌──────────┐ ┌──────────────────────────────────┐  │   ║
║  │  │  WiFi    │ │    Interface Manager              │  │   ║
║  │  │  Manager │ │  (eth0, lo, wlan0 config)         │  │   ║
║  │  └──────────┘ └──────────────────────────────────┘  │   ║
║  └──────────────────────────────────────────────────────┘   ║
║                                                              ║
║  ┌──────────────────────────────────────────────────────┐   ║
║  │              SHELL (Rust)   ncsh                      │   ║
║  │   ping │ ifconfig │ ip │ netstat │ route │ nslookup   │   ║
║  └──────────────────────────────────────────────────────┘   ║
╚══════════════════════════════════════════════════════════════╝
```

---

## 3. Komponen Utama

### 3.1 Bootloader (Rust)

- Berjalan dalam mode real (16-bit) → protected mode (32-bit) → long mode (64-bit)
- Setup GDT awal, paging minimal
- Load kernel ELF dari disk
- Referensi: [BOOTLOADER.md](BOOTLOADER.md)

### 3.2 Kernel Core (Rust)

- `no_std` environment
- Manajemen memori: frame allocator, page table, heap
- Interrupt handling: GDT, IDT, PIC
- Preemptive scheduler: Round-Robin
- Referensi: [KERNEL.md](KERNEL.md)

### 3.3 Network Subsystem - Kernel Layer (Rust)

- Driver: VirtIO-net (QEMU), e1000 (Intel NIC)
- TCP/IP Stack: smoltcp (pure Rust)
- Socket API melalui syscall
- Netlink IPC layer untuk komunikasi dengan Go daemon
- Referensi: [NETWORK_KERNEL.md](NETWORK_KERNEL.md)

### 3.4 Network Manager Daemon (Go)

- Berjalan di user space sebagai daemon (`neonetd`)
- Berkomunikasi dengan kernel via custom netlink IPC
- Modul: DHCP, DNS, Routing, Firewall, WiFi
- Referensi: [NETWORK_MANAGER.md](NETWORK_MANAGER.md)

### 3.5 Shell (Rust)

- Interactive CLI
- Built-in commands: ls, cat, ping, ifconfig, ip, route
- Referensi: [SHELL.md](SHELL.md)

---

## 4. Aliran Data: Network Request

```
User mengetik: ping 8.8.8.8
        │
        ▼
  Shell (Rust)
  → parse "ping" command
  → buat ICMP packet
        │
        ▼ syscall: socket(), sendto()
  Kernel Syscall Handler (Rust)
  → validasi parameter
  → kirim ke Socket Layer
        │
        ▼
  smoltcp TCP/IP Stack (Rust)
  → buat ICMP Echo Request
  → buat IP header (src: 192.168.1.x, dst: 8.8.8.8)
  → buat Ethernet frame
        │
        ▼
  NIC Driver / VirtIO-net (Rust)
  → kirim packet ke hardware
        │
        ▼
  HARDWARE / NETWORK
```

---

## 5. Aliran Data: Network Configuration (DHCP)

```
Boot OS
  │
  ▼
neonetd (Go daemon) start
  │
  ▼
DHCP Module (Go)
→ buat DHCP Discover packet
→ kirim via IPC ke kernel
  │
  ▼ IPC: netlink message
Kernel Netlink Handler (Rust)
→ route request ke smoltcp UDP socket
→ kirim ke network
  │
  ▼ Network
DHCP Server response
  │
  ▼
Kernel menerima packet
→ kirim ke neonetd via IPC
  │
  ▼
DHCP Module (Go)
→ parse DHCP Offer
→ kirim DHCP Request
→ terima DHCP Ack
→ konfigurasi IP via syscall netconfig
  │
  ▼
Interface eth0 mendapat IP: 192.168.1.100/24
```

---

## 6. Memory Map x86_64

```
Virtual Address Space (48-bit = 256TB total)

0x0000_0000_0000_0000 ─┬─ NULL / Unmapped
                        │
0x0000_0000_0010_0000 ─┼─ Kernel Code & Data (low 1MB legacy)
                        │
0xFFFF_8000_0000_0000 ─┼─ KERNEL SPACE START (higher half)
                        │
0xFFFF_8000_0000_0000 ─┼─ Physical Memory Map (direct mapping)
                        │  Max 512GB physical RAM
                        │
0xFFFF_C000_0000_0000 ─┼─ Kernel Heap
                        │  (dynamic allocation)
                        │
0xFFFF_E000_0000_0000 ─┼─ Kernel Stack
                        │
0xFFFF_FFFF_8000_0000 ─┼─ Kernel Binary (.text, .data, .bss)
                        │
0xFFFF_FFFF_FFFF_FFFF ─┘
```

---

## 7. Syscall Table

| Nomor | Nama          | Deskripsi                                       |
| ----- | ------------- | ----------------------------------------------- |
| 0     | `read`      | Baca dari file descriptor                       |
| 1     | `write`     | Tulis ke file descriptor                        |
| 2     | `open`      | Buka file                                       |
| 3     | `close`     | Tutup file descriptor                           |
| 10    | `fork`      | Buat proses baru                                |
| 11    | `exec`      | Jalankan program                                |
| 20    | `socket`    | Buat socket                                     |
| 21    | `bind`      | Bind socket ke alamat                           |
| 22    | `connect`   | Hubungkan socket                                |
| 23    | `send`      | Kirim data                                      |
| 24    | `recv`      | Terima data                                     |
| 30    | `netconfig` | Konfigurasi network interface (khusus NeoCore OS) |
| 31    | `netroute`  | Manage routing table                            |
| 32    | `netfilter` | Manage firewall rules                           |

---

## 8. Referensi Eksternal

- [Writing an OS in Rust (phil-opp)](https://os.phil-opp.com/)
- [OSDev Wiki](https://wiki.osdev.org/)
- [smoltcp - Rust TCP/IP Stack](https://github.com/smoltcp-rs/smoltcp)
- [Redox OS](https://gitlab.redox-os.org/redox-os/redox)
- [x86_64 crate](https://docs.rs/x86_64)
