// cmd/neonetd/main.go — NeoCore Network Manager Daemon (neonetd)
// Berjalan di user space, berkomunikasi dengan kernel via /dev/netctl
// SPDX-License-Identifier: MIT

package main

import (
	"flag"
	"fmt"
	"log"
	"os"
	"os/signal"
	"syscall"
	"time"

	"neocore-os/netmgr/internal/dhcp"
	"neocore-os/netmgr/internal/dns"
	"neocore-os/netmgr/internal/firewall"
	"neocore-os/netmgr/internal/ipc"
	"neocore-os/netmgr/internal/routing"
)

// ── Version ───────────────────────────────────────────────────────────────

const (
	Version   = "0.1.0"
	BuildDate = "2026-10-01"
)

// ── Flags ─────────────────────────────────────────────────────────────────

var (
	flagDevice  = flag.String("dev", "/dev/netctl", "Path ke kernel IPC device")
	flagIface   = flag.String("iface", "eth0", "Network interface utama")
	flagDNS1    = flag.String("dns1", "8.8.8.8", "Primary DNS server")
	flagDNS2    = flag.String("dns2", "1.1.1.1", "Secondary DNS server")
	flagVerbose = flag.Bool("v", false, "Verbose logging")
	flagVersion = flag.Bool("version", false, "Tampilkan versi")
)

// ── Main ──────────────────────────────────────────────────────────────────

func main() {
	flag.Parse()

	if *flagVersion {
		fmt.Printf("neonetd v%s (built %s)\n", Version, BuildDate)
		os.Exit(0)
	}

	log.SetPrefix("[neonetd] ")
	log.SetFlags(log.Ltime | log.Lmicroseconds)

	log.Printf("NeoCore Network Manager v%s starting...", Version)
	log.Printf("Interface: %s | IPC device: %s", *flagIface, *flagDevice)

	// ── 1. IPC Client — sambungkan ke kernel ──────────────────────────
	client, err := ipc.NewClient(*flagDevice)
	if err != nil {
		log.Printf("WARNING: Cannot open kernel IPC (%v) — running in simulation mode", err)
		client = ipc.NewSimulatedClient()
	}
	defer client.Close()

	log.Printf("[IPC  ] Connected to kernel IPC channel")

	// ── 2. Komponen-komponen jaringan ─────────────────────────────────

	// Firewall — inisialisasi policy default dulu (sebelum DHCP)
	fw := firewall.New()
	fw.SetDefaultPolicy(firewall.PolicyDrop)
	fw.Allow(firewall.RuleICMP())
	fw.Allow(firewall.RuleDHCP())
	fw.Allow(firewall.RuleEstablished())
	fw.Allow(firewall.RuleOutbound())
	log.Printf("[FW   ] Firewall initialized with default policies")

	// DNS Resolver
	resolver := dns.NewResolver([]string{*flagDNS1, *flagDNS2})
	log.Printf("[DNS  ] Resolver initialized: %s, %s", *flagDNS1, *flagDNS2)

	// Routing Manager
	rt := routing.NewManager(client)
	log.Printf("[ROUTE] Routing manager initialized")

	// DHCP Client
	dhcpClient := dhcp.NewClient(*flagIface, client)
	log.Printf("[DHCP ] DHCP client initialized for %s", *flagIface)

	// ── 3. DHCP — dapatkan IP otomatis ────────────────────────────────
	log.Printf("[DHCP ] Starting DHCP discovery on %s...", *flagIface)
	lease, err := dhcpClient.Acquire()
	if err != nil {
		log.Printf("[DHCP ] WARN: DHCP failed: %v — using fallback 192.168.1.100/24", err)
		lease = &dhcp.Lease{
			IP:      "192.168.1.100",
			Netmask: "255.255.255.0",
			Gateway: "192.168.1.1",
			DNS:     []string{*flagDNS1, *flagDNS2},
			LeaseTime: 3600 * time.Second,
		}
	}

	log.Printf("[DHCP ] Got lease: IP=%s GW=%s DNS=%v TTL=%v",
		lease.IP, lease.Gateway, lease.DNS, lease.LeaseTime)

	// Apply IP ke kernel via IPC
	if err := client.SendSetInterface(*flagIface, lease.IP, lease.Netmask); err != nil {
		log.Printf("[IPC  ] WARN: set interface failed: %v", err)
	}

	// Set default gateway
	if err := rt.SetDefaultGateway(lease.Gateway); err != nil {
		log.Printf("[ROUTE] WARN: set gateway failed: %v", err)
	}

	// Set DNS ke resolver
	if len(lease.DNS) > 0 {
		resolver.SetServers(lease.DNS)
		if err := client.SendSetDNS(lease.DNS); err != nil {
			log.Printf("[DNS  ] WARN: send DNS to kernel failed: %v", err)
		}
	}

	log.Printf("[NET  ] Network configured: %s via %s", lease.IP, lease.Gateway)
	log.Printf("[NET  ] neonetd ready — network is UP")

	// ── 4. Background tasks ───────────────────────────────────────────
	go dhcpClient.RenewLoop(lease)
	go rt.MonitorLoop()
	go resolver.CacheCleanLoop()

	// Contoh test DNS resolution
	go func() {
		time.Sleep(2 * time.Second)
		if *flagVerbose {
			ip, err := resolver.Resolve("google.com")
			if err != nil {
				log.Printf("[DNS  ] Resolve google.com: ERROR %v", err)
			} else {
				log.Printf("[DNS  ] Resolve google.com → %s", ip)
			}
		}
	}()

	// ── 5. Tunggu sinyal shutdown ─────────────────────────────────────
	sigs := make(chan os.Signal, 1)
	signal.Notify(sigs, syscall.SIGINT, syscall.SIGTERM)

	sig := <-sigs
	log.Printf("Received signal %v — shutting down neonetd...", sig)

	dhcpClient.Release(lease)
	client.Close()
	log.Printf("neonetd stopped.")
}
