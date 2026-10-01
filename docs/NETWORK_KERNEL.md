# 🌐 NETWORK_KERNEL — Network Subsystem Kernel (Rust)

---

## 1. Overview

Network subsystem di kernel bertanggung jawab untuk:
- **NIC Driver**: Komunikasi langsung dengan hardware kartu jaringan
- **Ethernet Layer**: Frame encoding/decoding, ARP
- **TCP/IP Stack**: Menggunakan `smoltcp` (pure Rust, no_std compatible)
- **Socket API**: Interface untuk user space via syscall
- **Netlink IPC**: Komunikasi dengan Go Network Manager

---

## 2. Arsitektur Network Stack Kernel

```
User Space (Go neonetd / Shell)
          │
          │ syscall: socket(), bind(), send(), recv(), netconfig()
          ▼
┌─────────────────────────────────────────────────────┐
│              SOCKET LAYER (Rust)                     │
│  File Descriptor → Socket → smoltcp socket handle   │
└──────────────────────┬──────────────────────────────┘
                        │
┌──────────────────────▼──────────────────────────────┐
│           smoltcp TCP/IP STACK (Rust)                │
│                                                      │
│  ┌───────────┐  ┌───────────┐  ┌───────────────┐   │
│  │ ICMPv4/v6 │  │  TCP      │  │  UDP          │   │
│  └───────────┘  └───────────┘  └───────────────┘   │
│  ┌──────────────────────────────────────────────┐   │
│  │              IPv4 / IPv6                      │   │
│  └──────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────┐   │
│  │              ARP / NDP                        │   │
│  └──────────────────────────────────────────────┘   │
│  ┌──────────────────────────────────────────────┐   │
│  │           Ethernet Frame                      │   │
│  └──────────────────────────────────────────────┘   │
└──────────────────────┬──────────────────────────────┘
                        │
┌──────────────────────▼──────────────────────────────┐
│          NIC DRIVER (Rust)                           │
│  ┌───────────────────┐  ┌──────────────────────┐   │
│  │  VirtIO-net        │  │  Intel e1000/e1000e  │   │
│  │  (QEMU/KVM)        │  │  (Fisik / QEMU -net) │   │
│  └───────────────────┘  └──────────────────────┘   │
└──────────────────────────────────────────────────────┘
          │
          ▼ DMA / MMIO
       HARDWARE NIC
```

---

## 3. NIC Drivers

### 3.1 VirtIO-net Driver (QEMU Testing)

```rust
// src/drivers/net/virtio.rs
use volatile::Volatile;
use x86_64::instructions::port::Port;

const VIRTIO_PCI_VENDOR_ID: u16 = 0x1AF4;
const VIRTIO_PCI_NET_DEVICE: u16 = 0x1000;

/// VirtQueue untuk komunikasi dengan VirtIO device
pub struct VirtQueue {
    desc:  *mut VirtqDesc,   // Descriptor table
    avail: *mut VirtqAvail,  // Available ring
    used:  *mut VirtqUsed,   // Used ring
    size:  u16,
    last_used_idx: u16,
}

#[repr(C)]
struct VirtqDesc {
    addr:  u64,  // Alamat buffer
    len:   u32,  // Panjang buffer
    flags: u16,  // VIRTQ_DESC_F_NEXT, VIRTQ_DESC_F_WRITE
    next:  u16,  // Index descriptor berikutnya (jika chained)
}

#[repr(C)]
struct VirtqAvail {
    flags: u16,
    idx:   u16,
    ring:  [u16; 256],
}

#[repr(C)]
struct VirtqUsed {
    flags: u16,
    idx:   u16,
    ring:  [VirtqUsedElem; 256],
}

#[repr(C)]
struct VirtqUsedElem {
    id:  u32, // Index di descriptor table
    len: u32, // Bytes yang digunakan
}

pub struct VirtioNet {
    pci_base: u16,    // PCI I/O base address
    tx_queue: VirtQueue,
    rx_queue: VirtQueue,
    mac: [u8; 6],
}

impl VirtioNet {
    pub fn new(pci_base: u16) -> Option<Self> {
        // 1. Reset device
        unsafe {
            Port::<u8>::new(pci_base + 0x12).write(0); // Status = 0 (reset)
        }
        
        // 2. Acknowledge + driver
        unsafe {
            Port::<u8>::new(pci_base + 0x12).write(3); // ACK | DRIVER
        }

        // 3. Baca MAC address dari config space
        let mut mac = [0u8; 6];
        for i in 0..6 {
            mac[i] = unsafe {
                Port::<u8>::new(pci_base + 0x14 + i as u16).read()
            };
        }

        println!("VirtIO-net MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
                 mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);

        Some(VirtioNet {
            pci_base,
            tx_queue: VirtQueue::new(64),
            rx_queue: VirtQueue::new(64),
            mac,
        })
    }

    pub fn send(&mut self, data: &[u8]) -> Result<(), &'static str> {
        // Tambahkan VirtIO net header (12 bytes)
        let mut buf = [0u8; 1514 + 12];
        buf[12..12 + data.len()].copy_from_slice(data);
        
        // Masukkan ke TX queue
        self.tx_queue.add_buffer(&buf[..12 + data.len()], false)?;
        
        // Notify device
        unsafe {
            Port::<u16>::new(self.pci_base + 0x10).write(1); // Queue 1 = TX
        }
        Ok(())
    }

    pub fn receive(&mut self) -> Option<&[u8]> {
        // Cek apakah ada frame di RX queue
        if let Some((buffer, len)) = self.rx_queue.get_used() {
            // Skip VirtIO net header (12 bytes)
            Some(&buffer[12..len])
        } else {
            None
        }
    }

    pub fn mac_address(&self) -> [u8; 6] {
        self.mac
    }
}
```

### 3.2 Intel e1000 Driver

```rust
// src/drivers/net/e1000.rs

const E1000_MMIO_BASE: u64 = 0xFEBC0000; // MMIO address (dari PCI BAR0)

// Register offsets
const CTRL:    u32 = 0x00000; // Device Control
const STATUS:  u32 = 0x00008; // Device Status
const RCTL:    u32 = 0x00100; // Receive Control
const TCTL:    u32 = 0x00400; // Transmit Control
const RDBAL:   u32 = 0x02800; // RX Descriptor Base Low
const RDBAH:   u32 = 0x02804; // RX Descriptor Base High
const RDLEN:   u32 = 0x02808; // RX Descriptor Length
const RDH:     u32 = 0x02810; // RX Descriptor Head
const RDT:     u32 = 0x02818; // RX Descriptor Tail
const TDBAL:   u32 = 0x03800; // TX Descriptor Base Low
const TDBAH:   u32 = 0x03804; // TX Descriptor Base High
const TDLEN:   u32 = 0x03808; // TX Descriptor Length
const TDH:     u32 = 0x03810; // TX Descriptor Head
const TDT:     u32 = 0x03818; // TX Descriptor Tail
const RAL:     u32 = 0x05400; // Receive Address Low
const RAH:     u32 = 0x05404; // Receive Address High

#[repr(C)]
struct RxDescriptor {
    addr:   u64,   // Buffer address
    length: u16,
    csum:   u16,
    status: u8,
    errors: u8,
    special: u16,
}

#[repr(C)]
struct TxDescriptor {
    addr:    u64,  // Buffer address
    length:  u16,
    cso:     u8,   // Checksum offset
    cmd:     u8,   // Command flags
    status:  u8,
    css:     u8,   // Checksum start
    special: u16,
}

pub struct E1000 {
    mmio_base: *mut u32,
    tx_descs: &'static mut [TxDescriptor],
    rx_descs: &'static mut [RxDescriptor],
    tx_buffers: &'static mut [[u8; 2048]],
    rx_buffers: &'static mut [[u8; 2048]],
    tx_cur: usize,
    rx_cur: usize,
    mac: [u8; 6],
}

impl E1000 {
    fn read_reg(&self, reg: u32) -> u32 {
        unsafe { *self.mmio_base.add(reg as usize / 4) }
    }

    fn write_reg(&self, reg: u32, val: u32) {
        unsafe { *self.mmio_base.add(reg as usize / 4) = val; }
    }

    fn read_mac(&mut self) {
        let low  = self.read_reg(RAL);
        let high = self.read_reg(RAH);
        self.mac[0] = (low & 0xFF) as u8;
        self.mac[1] = ((low >> 8) & 0xFF) as u8;
        self.mac[2] = ((low >> 16) & 0xFF) as u8;
        self.mac[3] = ((low >> 24) & 0xFF) as u8;
        self.mac[4] = (high & 0xFF) as u8;
        self.mac[5] = ((high >> 8) & 0xFF) as u8;
    }

    pub fn send(&mut self, data: &[u8]) -> bool {
        let idx = self.tx_cur % 32;
        self.tx_buffers[idx][..data.len()].copy_from_slice(data);
        self.tx_descs[idx].addr   = self.tx_buffers[idx].as_ptr() as u64;
        self.tx_descs[idx].length = data.len() as u16;
        self.tx_descs[idx].cmd    = 0x0B; // EOP | IFCS | RS
        self.tx_descs[idx].status = 0;
        self.tx_cur += 1;
        self.write_reg(TDT, (self.tx_cur % 32) as u32);
        true
    }
}
```

---

## 4. smoltcp Integration

```rust
// src/net/stack.rs
use smoltcp::{
    iface::{Config, Interface, SocketSet},
    phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken},
    time::Instant,
    wire::{EthernetAddress, IpAddress, IpCidr, Ipv4Address},
};
use alloc::vec;
use lazy_static::lazy_static;
use spinning_top::Spinlock;

/// Wrapper NIC driver untuk smoltcp Device trait
pub struct NicDevice<'a> {
    driver: &'a mut dyn NicDriver,
}

pub trait NicDriver: Send {
    fn send(&mut self, data: &[u8]) -> bool;
    fn receive(&mut self) -> Option<alloc::vec::Vec<u8>>;
    fn mac_address(&self) -> [u8; 6];
}

// Implement smoltcp Device untuk NicDevice
impl<'a> Device for NicDevice<'a> {
    type RxToken<'b> = NicRxToken where Self: 'b;
    type TxToken<'b> = NicTxToken<'b> where Self: 'b;

    fn receive(&mut self, _ts: Instant) -> Option<(Self::RxToken<'_>, Self::TxToken<'_>)> {
        if let Some(data) = self.driver.receive() {
            Some((
                NicRxToken { data },
                NicTxToken { driver: self.driver },
            ))
        } else {
            None
        }
    }

    fn transmit(&mut self, _ts: Instant) -> Option<Self::TxToken<'_>> {
        Some(NicTxToken { driver: self.driver })
    }

    fn capabilities(&self) -> DeviceCapabilities {
        let mut caps = DeviceCapabilities::default();
        caps.medium = Medium::Ethernet;
        caps.max_transmission_unit = 1514;
        caps.max_burst_size = None;
        caps
    }
}

pub struct NicRxToken { data: alloc::vec::Vec<u8> }
pub struct NicTxToken<'a> { driver: &'a mut dyn NicDriver }

impl RxToken for NicRxToken {
    fn consume<R, F>(mut self, f: F) -> R
    where F: FnOnce(&mut [u8]) -> R {
        f(&mut self.data)
    }
}

impl<'a> TxToken for NicTxToken<'a> {
    fn consume<R, F>(self, len: usize, f: F) -> R
    where F: FnOnce(&mut [u8]) -> R {
        let mut buf = vec![0u8; len];
        let result = f(&mut buf);
        self.driver.send(&buf);
        result
    }
}

/// Network Stack global state
pub struct NetStack {
    pub iface: Interface,
    pub sockets: SocketSet<'static>,
}

lazy_static! {
    pub static ref NET_STACK: Spinlock<Option<NetStack>> = Spinlock::new(None);
}

pub fn init() {
    // Inisialisasi VirtIO-net driver
    let mut nic = crate::drivers::net::virtio::VirtioNet::new(0xC000)
        .expect("VirtIO-net not found");

    let mac = nic.mac_address();
    let eth_addr = EthernetAddress(mac);

    let mut device = NicDevice { driver: &mut nic };

    // Konfigurasi smoltcp interface
    let config = Config::new(eth_addr.into());
    let mut iface = Interface::new(config, &mut device, get_time());

    // Set IP address (akan di-overwrite oleh DHCP dari Go netmgr)
    iface.update_ip_addrs(|addrs| {
        addrs.push(IpCidr::new(IpAddress::v4(0, 0, 0, 0), 0)).unwrap();
    });

    let sockets = SocketSet::new(vec![]);

    *NET_STACK.lock() = Some(NetStack { iface, sockets });
    println!("[NET] Network stack initialized (MAC: {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X})",
             mac[0], mac[1], mac[2], mac[3], mac[4], mac[5]);
}

fn get_time() -> Instant {
    // Gunakan HPET atau PIT untuk timestamp
    Instant::from_millis(0) // TODO: implement proper timestamp
}

/// Poll network stack (dipanggil secara periodik dari scheduler)
pub fn poll() {
    let mut stack_lock = NET_STACK.lock();
    if let Some(ref mut stack) = *stack_lock {
        // TODO: Poll device dan proses packets
    }
}
```

---

## 5. Socket Layer

```rust
// src/net/socket.rs
use alloc::vec::Vec;
use smoltcp::socket::{tcp, udp, icmp};
use core::sync::atomic::{AtomicU32, Ordering};

static NEXT_FD: AtomicU32 = AtomicU32::new(10); // FD 0-9 reserved

// Socket domain
pub const AF_INET:  u64 = 2;
pub const AF_INET6: u64 = 10;

// Socket type
pub const SOCK_STREAM: u64 = 1; // TCP
pub const SOCK_DGRAM:  u64 = 2; // UDP
pub const SOCK_RAW:    u64 = 3; // Raw

pub fn create(domain: u64, sock_type: u64, _protocol: u64) -> u64 {
    let fd = NEXT_FD.fetch_add(1, Ordering::SeqCst) as u64;

    let mut stack_lock = super::stack::NET_STACK.lock();
    let stack = match stack_lock.as_mut() {
        Some(s) => s,
        None => return u64::MAX, // ENETDOWN
    };

    match sock_type {
        SOCK_STREAM => {
            // TCP socket
            let tcp_rx_buf = tcp::SocketBuffer::new(vec![0u8; 65535]);
            let tcp_tx_buf = tcp::SocketBuffer::new(vec![0u8; 65535]);
            let tcp_socket = tcp::Socket::new(tcp_rx_buf, tcp_tx_buf);
            let _handle = stack.sockets.add(tcp_socket);
            fd
        }
        SOCK_DGRAM => {
            // UDP socket
            let udp_rx_buf = udp::PacketBuffer::new(
                vec![udp::PacketMetadata::EMPTY; 64],
                vec![0u8; 65535],
            );
            let udp_tx_buf = udp::PacketBuffer::new(
                vec![udp::PacketMetadata::EMPTY; 64],
                vec![0u8; 65535],
            );
            let udp_socket = udp::Socket::new(udp_rx_buf, udp_tx_buf);
            let _handle = stack.sockets.add(udp_socket);
            fd
        }
        _ => u64::MAX, // EINVAL
    }
}
```

---

## 6. Netlink IPC Layer

```rust
// src/net/netlink.rs
// Komunikasi antara kernel dan Go network manager

use alloc::vec::Vec;
use spinning_top::Spinlock;
use lazy_static::lazy_static;

/// Tipe pesan Netlink
#[repr(u16)]
#[derive(Debug, Clone, Copy)]
pub enum NetlinkMsgType {
    SetInterface  = 1,  // Konfigurasi interface (IP, MTU, flags)
    GetInterface  = 2,  // Ambil info interface
    AddRoute      = 3,  // Tambah routing entry
    DelRoute      = 4,  // Hapus routing entry
    GetRoute      = 5,  // Ambil routing table
    AddFilter     = 6,  // Tambah firewall rule
    DelFilter     = 7,  // Hapus firewall rule
    GetStats      = 8,  // Statistik network
    SetDns        = 9,  // Set DNS server
    DhcpRequest   = 10, // DHCP request dari netmgr
    DhcpAck       = 11, // DHCP response ke netmgr
}

/// Header pesan Netlink
#[repr(C)]
pub struct NetlinkHeader {
    pub nlmsg_len:   u32,          // Total panjang pesan
    pub nlmsg_type:  NetlinkMsgType,
    pub nlmsg_flags: u16,
    pub nlmsg_seq:   u32,
    pub nlmsg_pid:   u32,          // PID pengirim (Go process)
}

/// Pesan SetInterface
#[repr(C)]
pub struct IfaceConfig {
    pub name:    [u8; 16],   // "eth0\0..."
    pub ip_addr: [u8; 4],    // IPv4 address
    pub netmask: [u8; 4],    // Subnet mask
    pub gateway: [u8; 4],    // Default gateway
    pub mtu:     u16,
    pub flags:   u32,        // IFF_UP, IFF_RUNNING, dll
}

/// Pesan AddRoute
#[repr(C)]
pub struct RouteEntry {
    pub dst:     [u8; 4],    // Destination network
    pub mask:    [u8; 4],    // Netmask
    pub gateway: [u8; 4],   // Next hop
    pub metric:  u32,
    pub iface:   [u8; 16],  // Interface name
}

lazy_static! {
    static ref NETLINK_RX_QUEUE: Spinlock<Vec<Vec<u8>>> = Spinlock::new(Vec::new());
    static ref NETLINK_TX_QUEUE: Spinlock<Vec<Vec<u8>>> = Spinlock::new(Vec::new());
}

/// Terapkan konfigurasi interface dari Go netmgr
pub fn apply_config(data: &[u8]) -> u64 {
    if data.len() < core::mem::size_of::<IfaceConfig>() {
        return u64::MAX; // EINVAL
    }

    let config = unsafe {
        &*(data.as_ptr() as *const IfaceConfig)
    };

    let ip = Ipv4Addr::from(config.ip_addr);
    let mask_bits = u32::from_be_bytes(config.netmask).leading_ones() as u8;

    // Update smoltcp interface IP
    let mut stack_lock = super::stack::NET_STACK.lock();
    if let Some(ref mut stack) = *stack_lock {
        stack.iface.update_ip_addrs(|addrs| {
            addrs.clear();
            addrs.push(IpCidr::new(
                IpAddress::v4(ip.0[0], ip.0[1], ip.0[2], ip.0[3]),
                mask_bits,
            )).unwrap();
        });

        // Set gateway
        if config.gateway != [0u8; 4] {
            let gw = config.gateway;
            stack.iface.routes_mut().add_default_ipv4_route(
                Ipv4Address::new(gw[0], gw[1], gw[2], gw[3])
            ).ok();
        }
    }

    println!("[NET] Interface configured: {}.{}.{}.{}/{}", 
             config.ip_addr[0], config.ip_addr[1],
             config.ip_addr[2], config.ip_addr[3],
             mask_bits);
    0 // Success
}

/// Terapkan routing entry
pub fn apply_route(data: &[u8]) -> u64 {
    if data.len() < core::mem::size_of::<RouteEntry>() {
        return u64::MAX;
    }
    let route = unsafe { &*(data.as_ptr() as *const RouteEntry) };
    // TODO: Tambahkan ke routing table smoltcp
    println!("[NET] Route added: {}.{}.{}.{}/{}", 
             route.dst[0], route.dst[1], route.dst[2], route.dst[3],
             u32::from_be_bytes(route.mask).leading_ones());
    0
}

/// IPC handler task - menangani pesan dari Go netmgr
pub fn ipc_handler() {
    loop {
        let mut rx_queue = NETLINK_RX_QUEUE.lock();
        while let Some(msg) = rx_queue.pop() {
            drop(rx_queue);

            if msg.len() < core::mem::size_of::<NetlinkHeader>() {
                rx_queue = NETLINK_RX_QUEUE.lock();
                continue;
            }

            let header = unsafe {
                &*(msg.as_ptr() as *const NetlinkHeader)
            };

            let payload = &msg[core::mem::size_of::<NetlinkHeader>()..];

            match header.nlmsg_type {
                NetlinkMsgType::SetInterface => { apply_config(payload); }
                NetlinkMsgType::AddRoute     => { apply_route(payload); }
                NetlinkMsgType::GetStats     => { /* TODO: send stats back */ }
                _ => {}
            }

            rx_queue = NETLINK_RX_QUEUE.lock();
        }

        x86_64::instructions::hlt(); // Tunggu interrupt berikutnya
    }
}
```

---

## 7. Network Interface Stats

```rust
// src/net/stats.rs
use core::sync::atomic::{AtomicU64, Ordering};

pub struct NetStats {
    pub rx_packets: AtomicU64,
    pub tx_packets: AtomicU64,
    pub rx_bytes:   AtomicU64,
    pub tx_bytes:   AtomicU64,
    pub rx_errors:  AtomicU64,
    pub tx_errors:  AtomicU64,
    pub rx_dropped: AtomicU64,
}

impl NetStats {
    pub const fn new() -> Self {
        NetStats {
            rx_packets: AtomicU64::new(0),
            tx_packets: AtomicU64::new(0),
            rx_bytes:   AtomicU64::new(0),
            tx_bytes:   AtomicU64::new(0),
            rx_errors:  AtomicU64::new(0),
            tx_errors:  AtomicU64::new(0),
            rx_dropped: AtomicU64::new(0),
        }
    }

    pub fn print(&self) {
        println!("RX: {} packets, {} bytes, {} errors, {} dropped",
            self.rx_packets.load(Ordering::Relaxed),
            self.rx_bytes.load(Ordering::Relaxed),
            self.rx_errors.load(Ordering::Relaxed),
            self.rx_dropped.load(Ordering::Relaxed),
        );
        println!("TX: {} packets, {} bytes, {} errors",
            self.tx_packets.load(Ordering::Relaxed),
            self.tx_bytes.load(Ordering::Relaxed),
            self.tx_errors.load(Ordering::Relaxed),
        );
    }
}

pub static ETH0_STATS: NetStats = NetStats::new();
```

---

## 8. Referensi

- [smoltcp Documentation](https://docs.rs/smoltcp)
- [smoltcp GitHub](https://github.com/smoltcp-rs/smoltcp)
- [OSDev — Network Stack](https://wiki.osdev.org/Network_Stack)
- [VirtIO Specification](https://docs.oasis-open.org/virtio/virtio/v1.2/virtio-v1.2.html)
- [Intel e1000 Developer Manual](https://www.intel.com/content/dam/doc/manual/pci-pci-x-family-gbe-controllers-software-dev-manual.pdf)
