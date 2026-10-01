# 📡 NETWORK_MANAGER — Go Network Manager Daemon (neonetd)

---

## 1. Overview

`neonetd` adalah daemon yang berjalan di **user space**, ditulis dalam **Go**, yang menangani semua konfigurasi jaringan tingkat tinggi. Daemon ini berkomunikasi dengan kernel via custom **Netlink IPC**.

**Mengapa Go?**
- **Goroutines** — concurrency ringan untuk menangani DHCP, DNS, routing secara paralel
- **Standard Library** — `net` package yang kaya untuk network programming
- **Fast startup** — Daemon siap dalam milidetik
- **Easy testing** — Unit test dan mock lebih mudah di Go

---

## 2. Struktur Project neonetd

```
netmgr/
├── go.mod
├── go.sum
├── Makefile
├── cmd/
│   └── neonetd/
│       └── main.go              # Entry point daemon
├── internal/
│   ├── dhcp/
│   │   ├── client.go            # DHCP client state machine
│   │   ├── packet.go            # DHCP packet encoding/decoding
│   │   └── options.go           # DHCP options parser
│   ├── dns/
│   │   ├── resolver.go          # DNS resolver
│   │   ├── cache.go             # DNS cache (TTL-based)
│   │   └── hosts.go             # /etc/hosts equivalent
│   ├── routing/
│   │   ├── table.go             # Routing table manager
│   │   ├── rip.go               # RIP protocol (opsional)
│   │   └── static.go            # Static route management
│   ├── firewall/
│   │   ├── rules.go             # Firewall rule management
│   │   ├── chain.go             # Rule chains (INPUT/OUTPUT/FORWARD)
│   │   └── match.go             # Packet matching logic
│   ├── wifi/
│   │   ├── manager.go           # WiFi interface manager
│   │   ├── scan.go              # Network scanning
│   │   └── wpa.go               # WPA2/WPA3 support
│   ├── iface/
│   │   ├── manager.go           # Interface lifecycle manager
│   │   └── monitor.go           # Interface state monitoring
│   └── ipc/
│       ├── client.go            # IPC client ke kernel
│       ├── protocol.go          # Netlink protocol definitions
│       └── messages.go          # Message types
├── pkg/
│   ├── netlink/
│   │   ├── socket.go            # Custom netlink socket
│   │   └── encoder.go           # Message encoder/decoder
│   └── config/
│       ├── parser.go            # Config file parser
│       └── schema.go            # Config schema
├── api/
│   └── v1/
│       ├── server.go            # REST API server (opsional)
│       └── handlers.go          # HTTP handlers
└── configs/
    └── neonetd.toml             # Konfigurasi default
```

---

## 3. Entry Point — main.go

```go
// cmd/neonetd/main.go
package main

import (
    "context"
    "log"
    "os"
    "os/signal"
    "sync"
    "syscall"

    "neocore-os/netmgr/internal/dhcp"
    "neocore-os/netmgr/internal/dns"
    "neocore-os/netmgr/internal/firewall"
    "neocore-os/netmgr/internal/iface"
    "neocore-os/netmgr/internal/ipc"
    "neocore-os/netmgr/internal/routing"
    "neocore-os/netmgr/pkg/config"
)

const VERSION = "0.1.0"

func main() {
    log.Printf("🐹 neonetd v%s — NeoCore OS Network Manager", VERSION)
    log.Printf("   Starting all network subsystems...")

    // Load konfigurasi
    cfg, err := config.Load("/etc/neonetd.toml")
    if err != nil {
        log.Printf("Warning: config not found, using defaults: %v", err)
        cfg = config.Default()
    }

    // Context dengan cancellation (untuk graceful shutdown)
    ctx, cancel := context.WithCancel(context.Background())
    defer cancel()

    // WaitGroup untuk semua goroutines
    var wg sync.WaitGroup

    // 1. Koneksi ke kernel via IPC
    kernelClient, err := ipc.NewClient("/dev/netctl")
    if err != nil {
        log.Fatalf("Failed to connect to kernel IPC: %v", err)
    }
    defer kernelClient.Close()
    log.Println("[OK] Connected to kernel IPC")

    // 2. Interface Manager
    ifaceMgr := iface.NewManager(kernelClient)
    wg.Add(1)
    go func() {
        defer wg.Done()
        if err := ifaceMgr.Run(ctx); err != nil {
            log.Printf("Interface manager error: %v", err)
        }
    }()

    // 3. DHCP Client (untuk setiap interface yang dikonfigurasi DHCP)
    for _, ifName := range cfg.DHCPInterfaces {
        dhcpClient := dhcp.NewClient(ifName, kernelClient)
        wg.Add(1)
        go func(client *dhcp.Client, name string) {
            defer wg.Done()
            log.Printf("[DHCP] Starting DHCP on %s", name)
            if err := client.Run(ctx); err != nil {
                log.Printf("DHCP error on %s: %v", name, err)
            }
        }(dhcpClient, ifName)
    }

    // 4. DNS Resolver
    dnsResolver := dns.NewResolver(cfg.DNS)
    wg.Add(1)
    go func() {
        defer wg.Done()
        log.Printf("[DNS] Starting resolver (upstream: %v)", cfg.DNS.Upstream)
        if err := dnsResolver.Run(ctx); err != nil {
            log.Printf("DNS resolver error: %v", err)
        }
    }()

    // 5. Routing Manager
    routeMgr := routing.NewManager(kernelClient, cfg.Routes)
    wg.Add(1)
    go func() {
        defer wg.Done()
        if err := routeMgr.Run(ctx); err != nil {
            log.Printf("Routing manager error: %v", err)
        }
    }()

    // 6. Firewall Manager
    fwMgr := firewall.NewManager(kernelClient, cfg.Firewall)
    wg.Add(1)
    go func() {
        defer wg.Done()
        log.Printf("[FW] Applying firewall rules...")
        if err := fwMgr.Run(ctx); err != nil {
            log.Printf("Firewall error: %v", err)
        }
    }()

    log.Println("✅ neonetd fully started and operational")

    // Tunggu signal untuk shutdown
    sigCh := make(chan os.Signal, 1)
    signal.Notify(sigCh, syscall.SIGTERM, syscall.SIGINT)
    <-sigCh

    log.Println("Shutting down neonetd...")
    cancel()
    wg.Wait()
    log.Println("neonetd stopped cleanly")
}
```

---

## 4. DHCP Client

```go
// internal/dhcp/client.go
package dhcp

import (
    "context"
    "fmt"
    "log"
    "math/rand"
    "net"
    "time"

    "neocore-os/netmgr/internal/ipc"
)

// State machine DHCP
type State int

const (
    StateInit      State = iota
    StateSelecting       // Menunggu DHCP Offer
    StateRequesting      // Mengirim DHCP Request
    StateBound           // IP sudah dapat
    StateRenewing        // Memperpanjang lease
    StateRebinding       // Lease hampir habis
    StateExpired         // Lease habis
)

type Client struct {
    ifaceName  string
    kernel     *ipc.Client
    state      State
    xid        uint32       // Transaction ID
    clientMAC  net.HardwareAddr
    offeredIP  net.IP
    serverIP   net.IP
    leaseTime  time.Duration
    renewTime  time.Duration
    rebindTime time.Duration
    boundAt    time.Time
}

func NewClient(ifaceName string, kernel *ipc.Client) *Client {
    return &Client{
        ifaceName: ifaceName,
        kernel:    kernel,
        state:     StateInit,
        xid:       rand.Uint32(),
    }
}

func (c *Client) Run(ctx context.Context) error {
    for {
        select {
        case <-ctx.Done():
            return nil
        default:
            if err := c.step(ctx); err != nil {
                log.Printf("[DHCP:%s] Error: %v, retrying in 5s...", c.ifaceName, err)
                select {
                case <-time.After(5 * time.Second):
                case <-ctx.Done():
                    return nil
                }
                c.state = StateInit
            }
        }
    }
}

func (c *Client) step(ctx context.Context) error {
    switch c.state {
    case StateInit:
        return c.discover(ctx)
    case StateSelecting:
        return c.waitOffer(ctx)
    case StateRequesting:
        return c.request(ctx)
    case StateBound:
        return c.waitRenewal(ctx)
    case StateRenewing:
        return c.renew(ctx)
    case StateExpired:
        c.state = StateInit
        return nil
    }
    return nil
}

func (c *Client) discover(ctx context.Context) error {
    log.Printf("[DHCP:%s] Sending DISCOVER...", c.ifaceName)

    pkt := BuildDiscover(c.xid, c.clientMAC)
    // Kirim broadcast UDP ke 255.255.255.255:67
    if err := c.sendPacket(pkt, net.IPv4bcast, 67); err != nil {
        return fmt.Errorf("send discover: %w", err)
    }

    c.state = StateSelecting
    return nil
}

func (c *Client) waitOffer(ctx context.Context) error {
    log.Printf("[DHCP:%s] Waiting for OFFER...", c.ifaceName)

    timeout := time.After(10 * time.Second)
    for {
        select {
        case <-ctx.Done():
            return nil
        case <-timeout:
            c.state = StateInit
            return fmt.Errorf("no DHCP offer received")
        default:
            pkt, err := c.receivePacket(ctx)
            if err != nil {
                continue
            }

            msg, err := ParsePacket(pkt)
            if err != nil || msg.XID != c.xid {
                continue
            }

            if msg.Type == MsgTypeOffer {
                c.offeredIP = msg.YourIP
                c.serverIP  = msg.ServerIP
                log.Printf("[DHCP:%s] Got OFFER: %s from %s",
                    c.ifaceName, c.offeredIP, c.serverIP)
                c.state = StateRequesting
                return nil
            }
        }
    }
}

func (c *Client) request(ctx context.Context) error {
    log.Printf("[DHCP:%s] Sending REQUEST for %s...", c.ifaceName, c.offeredIP)

    pkt := BuildRequest(c.xid, c.clientMAC, c.offeredIP, c.serverIP)
    if err := c.sendPacket(pkt, net.IPv4bcast, 67); err != nil {
        return fmt.Errorf("send request: %w", err)
    }

    // Tunggu ACK
    timeout := time.After(10 * time.Second)
    for {
        select {
        case <-ctx.Done():
            return nil
        case <-timeout:
            c.state = StateInit
            return fmt.Errorf("no DHCP ACK")
        default:
            pkt, err := c.receivePacket(ctx)
            if err != nil {
                continue
            }

            msg, err := ParsePacket(pkt)
            if err != nil || msg.XID != c.xid {
                continue
            }

            if msg.Type == MsgTypeAck {
                c.leaseTime  = time.Duration(msg.LeaseTime) * time.Second
                c.renewTime  = c.leaseTime / 2
                c.rebindTime = c.leaseTime * 7 / 8
                c.boundAt    = time.Now()

                // Terapkan konfigurasi ke kernel
                if err := c.applyConfig(msg); err != nil {
                    return fmt.Errorf("apply config: %w", err)
                }

                log.Printf("[DHCP:%s] ✅ Bound! IP=%s GW=%s DNS=%v Lease=%v",
                    c.ifaceName, c.offeredIP, msg.Gateway, msg.DNS, c.leaseTime)
                c.state = StateBound
                return nil
            }

            if msg.Type == MsgTypeNak {
                c.state = StateInit
                return fmt.Errorf("DHCP NAK received")
            }
        }
    }
}

func (c *Client) applyConfig(msg *DHCPMessage) error {
    // Kirim ke kernel via IPC untuk konfigurasi interface
    return c.kernel.SetInterface(&ipc.IfaceConfig{
        Name:    c.ifaceName,
        IPAddr:  c.offeredIP,
        Netmask: msg.SubnetMask,
        Gateway: msg.Gateway,
        MTU:     1500,
    })
}

func (c *Client) waitRenewal(ctx context.Context) error {
    renewAt  := c.boundAt.Add(c.renewTime)
    rebindAt := c.boundAt.Add(c.rebindTime)
    expireAt := c.boundAt.Add(c.leaseTime)

    select {
    case <-ctx.Done():
        return nil
    case <-time.After(time.Until(renewAt)):
        log.Printf("[DHCP:%s] Lease renewal time", c.ifaceName)
        c.state = StateRenewing
    case <-time.After(time.Until(expireAt)):
        log.Printf("[DHCP:%s] Lease expired!", c.ifaceName)
        c.state = StateExpired
    }
    _ = rebindAt
    return nil
}

func (c *Client) renew(ctx context.Context) error {
    log.Printf("[DHCP:%s] Renewing lease for %s...", c.ifaceName, c.offeredIP)
    // Unicast request ke server
    c.xid = rand.Uint32()
    pkt := BuildRequest(c.xid, c.clientMAC, c.offeredIP, c.serverIP)
    return c.sendPacket(pkt, c.serverIP, 67)
}

func (c *Client) sendPacket(pkt []byte, dst net.IP, port int) error {
    // Kirim via kernel socket syscall
    return c.kernel.SendUDP(c.ifaceName, net.IPv4zero, dst, 68, port, pkt)
}

func (c *Client) receivePacket(ctx context.Context) ([]byte, error) {
    return c.kernel.RecvUDP(ctx, c.ifaceName, 68)
}
```

---

## 5. DNS Resolver

```go
// internal/dns/resolver.go
package dns

import (
    "context"
    "fmt"
    "log"
    "net"
    "strings"
    "sync"
    "time"
)

type Config struct {
    Upstream    []string      // DNS servers: ["8.8.8.8:53", "1.1.1.1:53"]
    CacheTTL    time.Duration
    Timeout     time.Duration
    MaxRetries  int
}

type CacheEntry struct {
    Addresses []net.IP
    ExpiresAt time.Time
}

type Resolver struct {
    config   Config
    cache    map[string]*CacheEntry
    mu       sync.RWMutex
    upstreams []*net.UDPConn
}

func NewResolver(cfg Config) *Resolver {
    return &Resolver{
        config: cfg,
        cache:  make(map[string]*CacheEntry),
    }
}

func (r *Resolver) Run(ctx context.Context) error {
    // Connect ke upstream servers
    for _, upstream := range r.config.Upstream {
        conn, err := net.DialUDP("udp", nil, resolveAddr(upstream))
        if err != nil {
            log.Printf("[DNS] Warning: cannot connect to %s: %v", upstream, err)
            continue
        }
        r.upstreams = append(r.upstreams, conn)
        log.Printf("[DNS] Using upstream: %s", upstream)
    }

    if len(r.upstreams) == 0 {
        return fmt.Errorf("no DNS upstreams available")
    }

    // Cache cleanup goroutine
    go r.cleanupCache(ctx)

    log.Println("[DNS] Resolver ready")
    <-ctx.Done()
    return nil
}

// Resolve nama domain ke IP addresses
func (r *Resolver) Resolve(ctx context.Context, name string) ([]net.IP, error) {
    name = strings.ToLower(strings.TrimSuffix(name, "."))

    // 1. Cek cache
    r.mu.RLock()
    if entry, ok := r.cache[name]; ok && time.Now().Before(entry.ExpiresAt) {
        r.mu.RUnlock()
        log.Printf("[DNS] Cache hit: %s → %v", name, entry.Addresses)
        return entry.Addresses, nil
    }
    r.mu.RUnlock()

    // 2. Query upstream
    addrs, ttl, err := r.queryUpstream(ctx, name)
    if err != nil {
        return nil, fmt.Errorf("resolve %s: %w", name, err)
    }

    // 3. Simpan ke cache
    r.mu.Lock()
    r.cache[name] = &CacheEntry{
        Addresses: addrs,
        ExpiresAt: time.Now().Add(ttl),
    }
    r.mu.Unlock()

    log.Printf("[DNS] Resolved: %s → %v (TTL: %v)", name, addrs, ttl)
    return addrs, nil
}

func (r *Resolver) queryUpstream(ctx context.Context, name string) ([]net.IP, time.Duration, error) {
    // Build DNS query packet
    query := buildQuery(name, TypeA)

    for _, conn := range r.upstreams {
        conn.SetDeadline(time.Now().Add(r.config.Timeout))

        _, err := conn.Write(query)
        if err != nil {
            continue
        }

        buf := make([]byte, 512)
        n, err := conn.Read(buf)
        if err != nil {
            continue
        }

        addrs, ttl, err := parseResponse(buf[:n])
        if err != nil {
            continue
        }

        return addrs, ttl, nil
    }

    return nil, 0, fmt.Errorf("all upstreams failed")
}

func (r *Resolver) cleanupCache(ctx context.Context) {
    ticker := time.NewTicker(60 * time.Second)
    defer ticker.Stop()

    for {
        select {
        case <-ctx.Done():
            return
        case <-ticker.C:
            now := time.Now()
            r.mu.Lock()
            for name, entry := range r.cache {
                if now.After(entry.ExpiresAt) {
                    delete(r.cache, name)
                }
            }
            r.mu.Unlock()
        }
    }
}

// DNS Record Types
const (
    TypeA    = 1   // IPv4
    TypeAAAA = 28  // IPv6
    TypeMX   = 15  // Mail
    TypeNS   = 2   // Nameserver
    TypeCNAME = 5  // Alias
)

func buildQuery(name string, qtype uint16) []byte {
    // Build DNS query packet (RFC 1035)
    var pkt []byte
    
    // Transaction ID (random)
    pkt = append(pkt, 0xAB, 0xCD)
    // Flags: Standard query, recursion desired
    pkt = append(pkt, 0x01, 0x00)
    // QDCOUNT = 1
    pkt = append(pkt, 0x00, 0x01)
    // ANCOUNT, NSCOUNT, ARCOUNT = 0
    pkt = append(pkt, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00)
    
    // QNAME: encode domain name
    for _, label := range strings.Split(name, ".") {
        pkt = append(pkt, byte(len(label)))
        pkt = append(pkt, []byte(label)...)
    }
    pkt = append(pkt, 0x00) // Root label
    
    // QTYPE
    pkt = append(pkt, byte(qtype>>8), byte(qtype))
    // QCLASS = IN (1)
    pkt = append(pkt, 0x00, 0x01)
    
    return pkt
}

func parseResponse(data []byte) ([]net.IP, time.Duration, error) {
    // Parse DNS response (simplified)
    if len(data) < 12 {
        return nil, 0, fmt.Errorf("response too short")
    }
    
    // Check response code
    rcode := data[3] & 0x0F
    if rcode != 0 {
        return nil, 0, fmt.Errorf("DNS error code: %d", rcode)
    }

    // Parse answers (simplified - skip full RFC 1035 parsing)
    var addrs []net.IP
    var minTTL time.Duration = 300 * time.Second

    // TODO: Full DNS response parsing
    _ = minTTL
    
    return addrs, 300 * time.Second, nil
}

func resolveAddr(addr string) *net.UDPAddr {
    if !strings.Contains(addr, ":") {
        addr = addr + ":53"
    }
    udpAddr, _ := net.ResolveUDPAddr("udp", addr)
    return udpAddr
}
```

---

## 6. Routing Manager

```go
// internal/routing/table.go
package routing

import (
    "context"
    "fmt"
    "log"
    "net"
    "sync"

    "neocore-os/netmgr/internal/ipc"
)

type Route struct {
    Destination *net.IPNet
    Gateway     net.IP
    Interface   string
    Metric      uint32
    IsDefault   bool
}

type Manager struct {
    kernel  *ipc.Client
    routes  []Route
    mu      sync.RWMutex
    static  []Route // Static routes dari config
}

func NewManager(kernel *ipc.Client, static []Route) *Manager {
    return &Manager{
        kernel: kernel,
        static: static,
    }
}

func (m *Manager) Run(ctx context.Context) error {
    // Apply static routes pertama
    for _, route := range m.static {
        if err := m.AddRoute(route); err != nil {
            log.Printf("[ROUTE] Failed to add static route %v: %v", route, err)
        }
    }

    log.Printf("[ROUTE] Manager ready, %d static routes", len(m.static))
    <-ctx.Done()
    return nil
}

func (m *Manager) AddRoute(route Route) error {
    m.mu.Lock()
    defer m.mu.Unlock()

    // Kirim ke kernel via IPC
    err := m.kernel.AddRoute(&ipc.RouteEntry{
        Destination: route.Destination,
        Gateway:     route.Gateway,
        Interface:   route.Interface,
        Metric:      route.Metric,
    })
    if err != nil {
        return fmt.Errorf("kernel add route: %w", err)
    }

    m.routes = append(m.routes, route)
    log.Printf("[ROUTE] Added: %s via %s dev %s metric %d",
        route.Destination, route.Gateway, route.Interface, route.Metric)
    return nil
}

func (m *Manager) DelRoute(dst *net.IPNet) error {
    m.mu.Lock()
    defer m.mu.Unlock()

    // Hapus dari kernel
    if err := m.kernel.DelRoute(dst); err != nil {
        return err
    }

    // Hapus dari daftar lokal
    newRoutes := m.routes[:0]
    for _, r := range m.routes {
        if r.Destination.String() != dst.String() {
            newRoutes = append(newRoutes, r)
        }
    }
    m.routes = newRoutes
    return nil
}

func (m *Manager) GetRoutes() []Route {
    m.mu.RLock()
    defer m.mu.RUnlock()
    result := make([]Route, len(m.routes))
    copy(result, m.routes)
    return result
}

// SetDefaultGateway - dipanggil oleh DHCP setelah mendapat IP
func (m *Manager) SetDefaultGateway(gw net.IP, iface string) error {
    _, defaultNet, _ := net.ParseCIDR("0.0.0.0/0")
    return m.AddRoute(Route{
        Destination: defaultNet,
        Gateway:     gw,
        Interface:   iface,
        Metric:      100,
        IsDefault:   true,
    })
}
```

---

## 7. Firewall Manager

```go
// internal/firewall/rules.go
package firewall

import (
    "context"
    "log"
    "net"
    "sync"

    "neocore-os/netmgr/internal/ipc"
)

type Action uint8

const (
    ActionAccept Action = iota
    ActionDrop
    ActionReject
    ActionLog
)

type Direction uint8

const (
    DirectionInbound  Direction = iota
    DirectionOutbound
    DirectionForward
)

type Rule struct {
    ID        uint32
    Direction Direction
    SrcNet    *net.IPNet
    DstNet    *net.IPNet
    SrcPort   uint16
    DstPort   uint16
    Protocol  string // "tcp", "udp", "icmp", "any"
    Action    Action
    Comment   string
}

type Manager struct {
    kernel *ipc.Client
    rules  []Rule
    mu     sync.RWMutex
}

func NewManager(kernel *ipc.Client, defaultRules []Rule) *Manager {
    return &Manager{
        kernel: kernel,
        rules:  defaultRules,
    }
}

func (m *Manager) Run(ctx context.Context) error {
    // Apply default rules
    for _, rule := range m.rules {
        if err := m.AddRule(rule); err != nil {
            log.Printf("[FW] Failed to add rule: %v", err)
        }
    }

    // Default policies
    m.applyDefaultPolicies()

    log.Printf("[FW] Firewall ready, %d rules loaded", len(m.rules))
    <-ctx.Done()
    return nil
}

func (m *Manager) applyDefaultPolicies() {
    // Allow loopback
    lo, _ := net.ParseCIDR("127.0.0.0/8")
    m.AddRule(Rule{
        Direction: DirectionInbound,
        SrcNet:    lo,
        Action:    ActionAccept,
        Comment:   "Allow loopback",
    })

    // Allow established connections
    m.AddRule(Rule{
        Direction: DirectionInbound,
        Protocol:  "tcp",
        Action:    ActionAccept,
        Comment:   "Allow established TCP",
        // TODO: state tracking
    })

    // Allow ICMP (ping)
    m.AddRule(Rule{
        Direction: DirectionInbound,
        Protocol:  "icmp",
        Action:    ActionAccept,
        Comment:   "Allow ICMP",
    })

    // Block everything else (default drop)
    m.AddRule(Rule{
        Direction: DirectionInbound,
        Action:    ActionDrop,
        Comment:   "Default drop",
    })

    log.Println("[FW] Default policies applied")
}

func (m *Manager) AddRule(rule Rule) error {
    m.mu.Lock()
    defer m.mu.Unlock()

    // Assign ID
    rule.ID = uint32(len(m.rules) + 1)

    // Kirim ke kernel
    if err := m.kernel.AddFirewallRule(&ipc.FirewallRule{
        ID:        rule.ID,
        Direction: uint8(rule.Direction),
        Protocol:  rule.Protocol,
        DstPort:   rule.DstPort,
        Action:    uint8(rule.Action),
    }); err != nil {
        return err
    }

    m.rules = append(m.rules, rule)
    log.Printf("[FW] Rule #%d added: %s %s port %d → %v [%s]",
        rule.ID, dirStr(rule.Direction), rule.Protocol,
        rule.DstPort, actionStr(rule.Action), rule.Comment)
    return nil
}

func dirStr(d Direction) string {
    switch d {
    case DirectionInbound:  return "IN"
    case DirectionOutbound: return "OUT"
    case DirectionForward:  return "FWD"
    }
    return "?"
}

func actionStr(a Action) string {
    switch a {
    case ActionAccept: return "ACCEPT"
    case ActionDrop:   return "DROP"
    case ActionReject: return "REJECT"
    case ActionLog:    return "LOG"
    }
    return "?"
}
```

---

## 8. IPC Client (Kernel Communication)

```go
// internal/ipc/client.go
package ipc

import (
    "context"
    "encoding/binary"
    "fmt"
    "net"
    "os"
    "sync"
    "sync/atomic"
)

// Message types (harus match dengan kernel netlink.rs)
const (
    MsgSetInterface  uint16 = 1
    MsgGetInterface  uint16 = 2
    MsgAddRoute      uint16 = 3
    MsgDelRoute      uint16 = 4
    MsgGetRoute      uint16 = 5
    MsgAddFilter     uint16 = 6
    MsgDelFilter     uint16 = 7
    MsgGetStats      uint16 = 8
    MsgSetDNS        uint16 = 9
    MsgSendUDP       uint16 = 20
    MsgRecvUDP       uint16 = 21
)

type Header struct {
    Length  uint32
    Type    uint16
    Flags   uint16
    Seq     uint32
    PID     uint32
}

type Client struct {
    conn   *os.File
    mu     sync.Mutex
    seq    atomic.Uint32
    pid    uint32
}

func NewClient(device string) (*Client, error) {
    f, err := os.OpenFile(device, os.O_RDWR, 0600)
    if err != nil {
        return nil, fmt.Errorf("open %s: %w", device, err)
    }
    return &Client{
        conn: f,
        pid:  uint32(os.Getpid()),
    }, nil
}

func (c *Client) Close() error {
    return c.conn.Close()
}

func (c *Client) sendMsg(msgType uint16, payload []byte) error {
    c.mu.Lock()
    defer c.mu.Unlock()

    hdr := Header{
        Length: uint32(12 + len(payload)),
        Type:   msgType,
        Flags:  0,
        Seq:    c.seq.Add(1),
        PID:    c.pid,
    }

    buf := make([]byte, 12+len(payload))
    binary.LittleEndian.PutUint32(buf[0:], hdr.Length)
    binary.LittleEndian.PutUint16(buf[4:], hdr.Type)
    binary.LittleEndian.PutUint16(buf[6:], hdr.Flags)
    binary.LittleEndian.PutUint32(buf[8:], hdr.Seq)
    // Note: PID tidak dikirim untuk hemat space, kernel sudah tahu
    copy(buf[12:], payload)

    _, err := c.conn.Write(buf)
    return err
}

// SetInterface - Konfigurasi network interface
func (c *Client) SetInterface(cfg *IfaceConfig) error {
    payload := encodeIfaceConfig(cfg)
    return c.sendMsg(MsgSetInterface, payload)
}

// AddRoute - Tambah routing entry
func (c *Client) AddRoute(entry *RouteEntry) error {
    payload := encodeRouteEntry(entry)
    return c.sendMsg(MsgAddRoute, payload)
}

// DelRoute - Hapus routing entry
func (c *Client) DelRoute(dst *net.IPNet) error {
    payload := encodeDstNetwork(dst)
    return c.sendMsg(MsgDelRoute, payload)
}

// AddFirewallRule - Tambah firewall rule
func (c *Client) AddFirewallRule(rule *FirewallRule) error {
    payload := encodeFirewallRule(rule)
    return c.sendMsg(MsgAddFilter, payload)
}

// SendUDP - Kirim UDP packet via kernel
func (c *Client) SendUDP(iface string, src, dst net.IP, srcPort, dstPort int, data []byte) error {
    // Encode request
    payload := encodeUDPSend(iface, src, dst, srcPort, dstPort, data)
    return c.sendMsg(MsgSendUDP, payload)
}

// RecvUDP - Terima UDP packet dari kernel
func (c *Client) RecvUDP(ctx context.Context, iface string, port int) ([]byte, error) {
    // TODO: Implement async receive dengan epoll/kqueue
    buf := make([]byte, 65536)
    n, err := c.conn.Read(buf)
    if err != nil {
        return nil, err
    }
    return buf[:n], nil
}
```

---

## 9. go.mod

```
module neocore-os/netmgr

go 1.22

require (
    github.com/BurntSushi/toml v1.3.2
    github.com/miekg/dns v1.1.58
)

require (
    golang.org/x/mod v0.14.0 // indirect
    golang.org/x/net v0.21.0 // indirect
    golang.org/x/sys v0.17.0 // indirect
    golang.org/x/tools v0.17.0 // indirect
)
```

---

## 10. Konfigurasi neonetd.toml

```toml
# /etc/neonetd.toml

[daemon]
log_level = "info"
pid_file  = "/run/neonetd.pid"

# Interface yang menggunakan DHCP
[dhcp]
interfaces = ["eth0", "wlan0"]
timeout    = 30  # detik

# DNS Configuration
[dns]
upstream = ["8.8.8.8:53", "1.1.1.1:53", "9.9.9.9:53"]
cache_ttl = 300  # detik

# Static Routes
[[routes]]
destination = "10.0.0.0/8"
gateway     = "192.168.1.1"
interface   = "eth0"
metric      = 200

# Firewall Rules
[[firewall.rules]]
direction = "inbound"
protocol  = "tcp"
dst_port  = 22
action    = "accept"
comment   = "Allow SSH"

[[firewall.rules]]
direction = "inbound"
protocol  = "tcp"
dst_port  = 80
action    = "accept"
comment   = "Allow HTTP"
```

---

## 11. Build & Run

```bash
# Build neonetd
cd netmgr
go build -o bin/neonetd ./cmd/neonetd

# Cross-compile untuk target OS (jika perlu)
GOOS=linux GOARCH=amd64 go build -o bin/neonetd ./cmd/neonetd

# Test
go test ./...

# Run (perlu akses ke /dev/netctl yang disediakan kernel)
sudo ./bin/neonetd -config /etc/neonetd.toml
```

---

## 12. Referensi

- [Go net package](https://pkg.go.dev/net)
- [DHCP RFC 2131](https://www.rfc-editor.org/rfc/rfc2131)
- [DNS RFC 1035](https://www.rfc-editor.org/rfc/rfc1035)
- [Linux Netlink](https://www.man7.org/linux/man-pages/man7/netlink.7.html)
- [NetworkManager (inspirasi)](https://networkmanager.dev/)
