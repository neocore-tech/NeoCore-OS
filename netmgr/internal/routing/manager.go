// internal/routing/manager.go — Routing Table Manager
// Mengelola routing entries dan sinkronisasi ke kernel via IPC
// SPDX-License-Identifier: MIT

package routing

import (
	"fmt"
	"log"
	"net"
	"sync"
	"time"

	"neocore-os/netmgr/internal/ipc"
)

// ── Route Entry ───────────────────────────────────────────────────────────

type Route struct {
	Destination string // CIDR, e.g. "192.168.1.0/24" atau "0.0.0.0/0"
	Gateway     string // Next hop IP
	Interface   string // e.g. "eth0"
	Metric      int    // Lower = preferred
	Static      bool   // true = user-configured, tidak di-hapus saat DHCP renew
}

func (r *Route) String() string {
	return fmt.Sprintf("%s via %s dev %s metric %d",
		r.Destination, r.Gateway, r.Interface, r.Metric)
}

// ── Manager ───────────────────────────────────────────────────────────────

type Manager struct {
	mu     sync.RWMutex
	routes []*Route
	ipc    ipc.Client
}

func NewManager(client ipc.Client) *Manager {
	return &Manager{
		ipc: client,
	}
}

// AddRoute menambahkan route baru
func (m *Manager) AddRoute(dst, gw, iface string, metric int, static bool) error {
	// Validasi
	_, _, err := net.ParseCIDR(dst)
	if dst != "0.0.0.0/0" && err != nil {
		return fmt.Errorf("invalid destination CIDR: %s", dst)
	}
	if net.ParseIP(gw) == nil {
		return fmt.Errorf("invalid gateway IP: %s", gw)
	}

	route := &Route{
		Destination: dst,
		Gateway:     gw,
		Interface:   iface,
		Metric:      metric,
		Static:      static,
	}

	m.mu.Lock()
	// Hapus route yang sama jika ada
	m.removeDuplicate(dst)
	m.routes = append(m.routes, route)
	m.mu.Unlock()

	// Sinkronisasi ke kernel
	if err := m.ipc.SendAddRoute(dst, gw, iface); err != nil {
		log.Printf("[ROUTE] WARN: kernel sync failed: %v", err)
	}

	log.Printf("[ROUTE] Added: %s", route)
	return nil
}

// SetDefaultGateway mengatur default route (0.0.0.0/0)
func (m *Manager) SetDefaultGateway(gw string) error {
	return m.AddRoute("0.0.0.0/0", gw, "eth0", 100, false)
}

// DelRoute menghapus route berdasarkan destination
func (m *Manager) DelRoute(dst string) error {
	m.mu.Lock()
	before := len(m.routes)
	m.removeDuplicate(dst)
	after := len(m.routes)
	m.mu.Unlock()

	if before == after {
		return fmt.Errorf("route not found: %s", dst)
	}

	if err := m.ipc.SendDelRoute(dst); err != nil {
		log.Printf("[ROUTE] WARN: kernel del route failed: %v", err)
	}

	log.Printf("[ROUTE] Removed: %s", dst)
	return nil
}

// ListRoutes mengembalikan semua route saat ini
func (m *Manager) ListRoutes() []*Route {
	m.mu.RLock()
	defer m.mu.RUnlock()
	result := make([]*Route, len(m.routes))
	copy(result, m.routes)
	return result
}

// removeDuplicate menghapus route dengan destination yang sama (tanpa lock)
func (m *Manager) removeDuplicate(dst string) {
	filtered := m.routes[:0]
	for _, r := range m.routes {
		if r.Destination != dst {
			filtered = append(filtered, r)
		}
	}
	m.routes = filtered
}

// MonitorLoop memantau perubahan routing setiap 30 detik
func (m *Manager) MonitorLoop() {
	ticker := time.NewTicker(30 * time.Second)
	defer ticker.Stop()

	for range ticker.C {
		routes := m.ListRoutes()
		log.Printf("[ROUTE] Active routes (%d):", len(routes))
		for _, r := range routes {
			log.Printf("[ROUTE]   %s", r)
		}
	}
}

// FlushDynamic menghapus semua route dinamis (dari DHCP)
// Static routes tidak terpengaruh
func (m *Manager) FlushDynamic() {
	m.mu.Lock()
	defer m.mu.Unlock()

	var kept []*Route
	for _, r := range m.routes {
		if r.Static {
			kept = append(kept, r)
		} else {
			log.Printf("[ROUTE] Flushing dynamic route: %s", r)
			m.ipc.SendDelRoute(r.Destination)
		}
	}
	m.routes = kept
}
