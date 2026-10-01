# 🔄 IPC — Inter-Process Communication (Kernel ↔ Go)

---

## 1. Overview

IPC (Inter-Process Communication) adalah mekanisme komunikasi antara **Rust Kernel** dan **Go Network Manager daemon**. NeoCore OS menggunakan custom **Netlink-like protocol** melalui character device `/dev/netctl`.

---

## 2. Desain IPC

```
Go Network Manager (neonetd)           Rust Kernel
┌───────────────────────────┐          ┌────────────────────────────────┐
│                           │          │                                │
│  ipc.Client.SetInterface()│          │  /dev/netctl                   │
│  → encodeIfaceConfig()    │──write──▶│  → netlink::ipc_handler()      │
│                           │          │  → apply_config()              │
│                           │          │  → smoltcp update IP           │
│                           │          │                                │
│  ipc.Client.RecvUDP()     │◀─read───│  kernel_recv_notify()          │
│                           │          │  → UDP packet dari NIC         │
│                           │          │                                │
└───────────────────────────┘          └────────────────────────────────┘

         via /dev/netctl (Character Device)
         atau Shared Memory + Semaphore (advanced)
```

---

## 3. Protocol Format

### 3.1 Netlink Header (12 bytes)

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
├───────────────────────────────────────────────────────────────────┤
│                        nlmsg_len (32 bits)                        │
│                    Total message length                           │
├───────────────────────────────┬───────────────────────────────────┤
│     nlmsg_type (16 bits)      │     nlmsg_flags (16 bits)         │
├───────────────────────────────────────────────────────────────────┤
│                        nlmsg_seq (32 bits)                        │
│                       Sequence number                             │
└───────────────────────────────────────────────────────────────────┘
```

### 3.2 Message Types

```
+--------+------------------+--------------------------------------+
| Type   | Name             | Payload                              |
+--------+------------------+--------------------------------------+
| 0x0001 | SET_INTERFACE    | IfaceConfig (52 bytes)               |
| 0x0002 | GET_INTERFACE    | InterfaceName (16 bytes)             |
| 0x0003 | ADD_ROUTE        | RouteEntry (36 bytes)                |
| 0x0004 | DEL_ROUTE        | DstNetwork (20 bytes)                |
| 0x0005 | GET_ROUTES       | (no payload)                         |
| 0x0006 | ADD_FILTER       | FirewallRule (32 bytes)              |
| 0x0007 | DEL_FILTER       | RuleId (4 bytes)                     |
| 0x0008 | GET_STATS        | InterfaceName (16 bytes)             |
| 0x0009 | SET_DNS          | DnsConfig (variable)                 |
| 0x0014 | SEND_UDP         | UdpSendReq (variable)                |
| 0x0015 | RECV_UDP         | UdpData (variable)                   |
| 0x00FF | ACK              | Status (4 bytes: 0=OK, else errno)   |
+--------+------------------+--------------------------------------+
```

---

## 4. Payload Structures

### 4.1 IfaceConfig (SET_INTERFACE)

```
Offset  Size  Field         Description
──────────────────────────────────────────────────────
0       16    name          Interface name (e.g., "eth0\0")
16      4     ip_addr       IPv4 address (big-endian)
20      4     netmask       Subnet mask (big-endian)
24      4     gateway       Default gateway (big-endian)
28      4     broadcast     Broadcast address
32      2     mtu           MTU (max 9000 for jumbo)
34      2     _pad          Padding
36      4     flags         IFF_UP=1, IFF_RUNNING=2, IFF_PROMISC=4
40      6     mac_addr      MAC address override (0=use NIC default)
46      2     vlan_id       VLAN ID (0=no VLAN)
48      4     _reserved     Reserved for future use
──────────────────────────────────────────────────────
Total: 52 bytes
```

**Rust:**
```rust
#[repr(C, packed)]
pub struct IfaceConfig {
    pub name:      [u8; 16],
    pub ip_addr:   [u8; 4],
    pub netmask:   [u8; 4],
    pub gateway:   [u8; 4],
    pub broadcast: [u8; 4],
    pub mtu:       u16,
    pub _pad:      u16,
    pub flags:     u32,
    pub mac_addr:  [u8; 6],
    pub vlan_id:   u16,
    pub _reserved: u32,
}
```

**Go:**
```go
type IfaceConfig struct {
    Name      [16]byte
    IPAddr    [4]byte
    Netmask   [4]byte
    Gateway   [4]byte
    Broadcast [4]byte
    MTU       uint16
    _pad      uint16
    Flags     uint32
    MACAddr   [6]byte
    VlanID    uint16
    _reserved uint32
}
```

### 4.2 RouteEntry (ADD_ROUTE)

```
Offset  Size  Field         Description
──────────────────────────────────────────────────────
0       4     dst           Destination network (IPv4)
4       4     mask          Netmask
8       4     gateway       Next hop gateway
12      4     src_hint      Preferred source (optional, 0=any)
16      16    iface         Output interface name
32      4     metric        Route metric/priority
──────────────────────────────────────────────────────
Total: 36 bytes
```

### 4.3 FirewallRule (ADD_FILTER)

```
Offset  Size  Field         Description
──────────────────────────────────────────────────────
0       4     rule_id       Unique rule ID
4       1     direction     0=IN, 1=OUT, 2=FORWARD
5       1     protocol      6=TCP, 17=UDP, 1=ICMP, 0=ANY
6       2     src_port      Source port (0=any)
8       2     dst_port      Destination port (0=any)
10      2     _pad          Padding
12      4     src_addr      Source IP (0=any)
16      4     src_mask      Source mask
20      4     dst_addr      Destination IP (0=any)
24      4     dst_mask      Destination mask
28      1     action        0=ACCEPT, 1=DROP, 2=REJECT, 3=LOG
29      3     _reserved
──────────────────────────────────────────────────────
Total: 32 bytes
```

### 4.4 UdpSendReq (SEND_UDP)

```
Offset  Size  Field         Description
──────────────────────────────────────────────────────
0       16    iface         Interface name
16      4     src_addr      Source IPv4 (0=auto)
20      4     dst_addr      Destination IPv4
24      2     src_port      Source port
26      2     dst_port      Destination port
28      4     data_len      Length of following data
32      N     data          UDP payload (N = data_len)
──────────────────────────────────────────────────────
Total: 32 + N bytes
```

---

## 5. Character Device Implementation (Kernel Side)

```rust
// src/net/netlink.rs — Character device /dev/netctl

use alloc::{collections::VecDeque, vec::Vec};
use spinning_top::Spinlock;
use lazy_static::lazy_static;

lazy_static! {
    /// RX buffer: Go → Kernel (pesan masuk dari neonetd)
    static ref RX_BUFFER: Spinlock<VecDeque<Vec<u8>>> =
        Spinlock::new(VecDeque::new());

    /// TX buffer: Kernel → Go (pesan keluar ke neonetd)
    static ref TX_BUFFER: Spinlock<VecDeque<Vec<u8>>> =
        Spinlock::new(VecDeque::new());
}

/// Dipanggil saat neonetd menulis ke /dev/netctl
pub fn dev_write(data: &[u8]) -> isize {
    if data.len() < 12 {
        return -22; // EINVAL
    }

    let mut buf = Vec::with_capacity(data.len());
    buf.extend_from_slice(data);
    RX_BUFFER.lock().push_back(buf);

    data.len() as isize
}

/// Dipanggil saat neonetd membaca dari /dev/netctl
pub fn dev_read(buf: &mut [u8]) -> isize {
    let mut tx = TX_BUFFER.lock();
    if let Some(msg) = tx.pop_front() {
        let len = msg.len().min(buf.len());
        buf[..len].copy_from_slice(&msg[..len]);
        return len as isize;
    }
    0 // No data available
}

/// Kirim notifikasi ke neonetd (kernel-initiated)
pub fn notify_netmgr(msg_type: NetlinkMsgType, payload: &[u8]) {
    let total_len = 12 + payload.len();
    let mut msg = Vec::with_capacity(total_len);

    // Header
    msg.extend_from_slice(&(total_len as u32).to_le_bytes()); // nlmsg_len
    msg.extend_from_slice(&(msg_type as u16).to_le_bytes());  // nlmsg_type
    msg.extend_from_slice(&0u16.to_le_bytes());               // nlmsg_flags
    msg.extend_from_slice(&0u32.to_le_bytes());               // nlmsg_seq (kernel-generated)

    msg.extend_from_slice(payload);

    TX_BUFFER.lock().push_back(msg);
}

/// Main IPC processing loop
pub fn ipc_handler() {
    loop {
        let msg = {
            let mut rx = RX_BUFFER.lock();
            rx.pop_front()
        };

        if let Some(data) = msg {
            process_message(&data);
        } else {
            x86_64::instructions::hlt();
        }
    }
}

fn process_message(data: &[u8]) {
    if data.len() < 12 {
        return;
    }

    let nlmsg_len  = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let nlmsg_type = u16::from_le_bytes([data[4], data[5]]);
    let payload    = &data[12..];

    let result = match nlmsg_type {
        0x0001 => apply_iface_config(payload),
        0x0003 => apply_route_add(payload),
        0x0004 => apply_route_del(payload),
        0x0006 => apply_firewall_add(payload),
        0x0007 => apply_firewall_del(payload),
        0x0014 => handle_udp_send(payload),
        _ => Err(-95), // EOPNOTSUPP
    };

    // Kirim ACK
    let status = match result {
        Ok(_)  => 0u32,
        Err(e) => e as u32,
    };

    notify_netmgr(NetlinkMsgType::Ack, &status.to_le_bytes());
}
```

---

## 6. IPC Client Implementation (Go Side)

```go
// pkg/netlink/socket.go

package netlink

import (
    "encoding/binary"
    "fmt"
    "os"
    "sync"
    "sync/atomic"
)

const DevicePath = "/dev/netctl"

type Conn struct {
    f      *os.File
    mu     sync.Mutex
    seqNum atomic.Uint32
}

func Open() (*Conn, error) {
    f, err := os.OpenFile(DevicePath, os.O_RDWR|os.O_SYNC, 0600)
    if err != nil {
        return nil, fmt.Errorf("open %s: %w", DevicePath, err)
    }
    return &Conn{f: f}, nil
}

func (c *Conn) Close() error {
    return c.f.Close()
}

// Send mengirim pesan ke kernel
func (c *Conn) Send(msgType uint16, payload []byte) error {
    c.mu.Lock()
    defer c.mu.Unlock()

    totalLen := 12 + len(payload)
    buf := make([]byte, totalLen)

    binary.LittleEndian.PutUint32(buf[0:], uint32(totalLen))
    binary.LittleEndian.PutUint16(buf[4:], msgType)
    binary.LittleEndian.PutUint16(buf[6:], 0) // flags
    binary.LittleEndian.PutUint32(buf[8:], c.seqNum.Add(1))
    copy(buf[12:], payload)

    _, err := c.f.Write(buf)
    return err
}

// Recv membaca pesan dari kernel
func (c *Conn) Recv() (msgType uint16, payload []byte, err error) {
    // Baca header dulu
    hdr := make([]byte, 12)
    n, err := c.f.Read(hdr)
    if err != nil || n < 12 {
        return 0, nil, fmt.Errorf("read header: %w", err)
    }

    totalLen := binary.LittleEndian.Uint32(hdr[0:])
    msgType   = binary.LittleEndian.Uint16(hdr[4:])

    if totalLen <= 12 {
        return msgType, nil, nil
    }

    payload = make([]byte, totalLen-12)
    _, err = c.f.Read(payload)
    return msgType, payload, err
}

// SendAndWaitAck - kirim pesan dan tunggu ACK
func (c *Conn) SendAndWaitAck(msgType uint16, payload []byte) error {
    if err := c.Send(msgType, payload); err != nil {
        return err
    }

    respType, respPayload, err := c.Recv()
    if err != nil {
        return fmt.Errorf("recv ack: %w", err)
    }

    if respType != 0x00FF { // Not ACK
        return fmt.Errorf("unexpected response type: %d", respType)
    }

    if len(respPayload) >= 4 {
        status := binary.LittleEndian.Uint32(respPayload)
        if status != 0 {
            return fmt.Errorf("kernel error: %d", status)
        }
    }

    return nil
}
```

```go
// pkg/netlink/encoder.go

package netlink

import (
    "bytes"
    "encoding/binary"
    "net"
)

// EncodeIfaceConfig - encode interface config ke bytes
func EncodeIfaceConfig(name string, ip, mask, gw net.IP, mtu uint16, flags uint32) []byte {
    buf := make([]byte, 52)
    
    // name (16 bytes)
    nameBytes := []byte(name)
    copy(buf[0:16], nameBytes)
    
    // ip_addr (4 bytes)
    ip4 := ip.To4()
    copy(buf[16:20], ip4)
    
    // netmask (4 bytes)
    mask4 := mask.To4()
    copy(buf[20:24], mask4)
    
    // gateway (4 bytes)
    gw4 := gw.To4()
    copy(buf[24:28], gw4)
    
    // broadcast (4 bytes)
    broadcast := calcBroadcast(ip4, mask4)
    copy(buf[28:32], broadcast)
    
    // mtu (2 bytes)
    binary.LittleEndian.PutUint16(buf[32:34], mtu)
    
    // flags (4 bytes)
    binary.LittleEndian.PutUint32(buf[36:40], flags)
    
    return buf
}

func calcBroadcast(ip, mask []byte) []byte {
    broadcast := make([]byte, 4)
    for i := range ip {
        broadcast[i] = ip[i] | ^mask[i]
    }
    return broadcast
}

// EncodeRouteEntry - encode route entry ke bytes
func EncodeRouteEntry(dst *net.IPNet, gw net.IP, iface string, metric uint32) []byte {
    buf := make([]byte, 36)
    
    dst4 := dst.IP.To4()
    mask := []byte(dst.Mask)
    gw4 := gw.To4()
    
    copy(buf[0:4], dst4)
    copy(buf[4:8], mask)
    copy(buf[8:12], gw4)
    // src_hint = 0
    copy(buf[16:32], []byte(iface))
    binary.LittleEndian.PutUint32(buf[32:36], metric)
    
    return buf
}

// EncodeFirewallRule - encode firewall rule ke bytes
func EncodeFirewallRule(id uint32, direction, proto uint8,
    srcPort, dstPort uint16, action uint8) []byte {
    
    buf := make([]byte, 32)
    binary.LittleEndian.PutUint32(buf[0:4], id)
    buf[4] = direction
    buf[5] = proto
    binary.LittleEndian.PutUint16(buf[6:8], srcPort)
    binary.LittleEndian.PutUint16(buf[8:10], dstPort)
    buf[28] = action
    
    return buf
}
```

---

## 7. Contoh Aliran Komunikasi Lengkap

### Boot sequence: DHCP

```
neonetd (Go)                                    Kernel (Rust)
    │                                               │
    │  1. Startup                                   │
    │  Open("/dev/netctl")                          │
    │──────────────────────────────────────────────▶│
    │                                               │
    │  2. Send: SEND_UDP (DHCP Discover broadcast)  │
    │──────────────────────────────────────────────▶│
    │                                        kernel routes UDP
    │                                        via smoltcp → NIC
    │                                               │──▶ Network
    │                                               │
    │  3. Recv: ACK (status=0)                      │
    │◀──────────────────────────────────────────────│
    │                                               │
    │  ... (wait for DHCP Offer from network) ...   │
    │                                               │
    │                                    DHCP Offer packet arrives
    │                                               │◀── Network
    │                                               │
    │  4. Recv: RECV_UDP (DHCP Offer data)          │
    │◀──────────────────────────────────────────────│
    │                                               │
    │  5. Parse DHCP Offer                          │
    │  Send: SEND_UDP (DHCP Request)                │
    │──────────────────────────────────────────────▶│
    │                                               │
    │  ... (wait for DHCP ACK) ...                  │
    │                                               │
    │  6. Recv: RECV_UDP (DHCP ACK)                 │
    │◀──────────────────────────────────────────────│
    │                                               │
    │  7. Send: SET_INTERFACE                       │
    │  {name:"eth0", ip:192.168.1.100,              │
    │   mask:255.255.255.0, gw:192.168.1.1}         │
    │──────────────────────────────────────────────▶│
    │                                     apply_iface_config()
    │                                     smoltcp update IP
    │                                               │
    │  8. Recv: ACK (status=0)                      │
    │◀──────────────────────────────────────────────│
    │                                               │
    │  9. Send: ADD_ROUTE (default via 192.168.1.1) │
    │──────────────────────────────────────────────▶│
    │                                     add default route
    │                                               │
    │  10. Recv: ACK (status=0)                     │
    │◀──────────────────────────────────────────────│
    │                                               │
    │  ✅ Network configured! eth0: 192.168.1.100   │
```

---

## 8. Error Codes

| Code | Errno | Deskripsi |
|------|-------|-----------|
| 0    | OK    | Sukses |
| 1    | EPERM | Operasi tidak diizinkan |
| 5    | EIO   | I/O error |
| 12   | ENOMEM | Out of memory |
| 22   | EINVAL | Invalid argument |
| 28   | ENOSPC | No space left |
| 95   | EOPNOTSUPP | Operation not supported |
| 100  | ENETDOWN | Network is down |
| 101  | ENETUNREACH | Network unreachable |
| 110  | ETIMEDOUT | Connection timed out |
| 111  | ECONNREFUSED | Connection refused |

---

## 9. Security Considerations

> [!WARNING]
> `/dev/netctl` harus hanya accessible oleh root (mode 0600).
> Kernel HARUS validasi semua input dari neonetd karena user space tidak trusted.

**Validasi yang dilakukan kernel:**
- Cek panjang pesan minimal sesuai message type
- Validate IP addresses (tidak boleh 0.0.0.0 untuk SET_INTERFACE)
- MTU range check (68 - 9000)
- Interface name sanitization (no path traversal, max 15 chars)
- Firewall rule ID uniqueness check

---

## 10. Referensi

- [Linux Netlink](https://www.man7.org/linux/man-pages/man7/netlink.7.html)
- [Netlink Protocol Library](https://www.infradead.org/~tgr/libnl/)
- [Linux IPC Mechanisms](https://tldp.org/LDP/tlk/ipc/ipc.html)
- [Character Device Driver](https://tldp.org/LDP/lkmpg/2.6/html/x892.html)
