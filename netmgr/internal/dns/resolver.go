// internal/dns/resolver.go — DNS Resolver dengan TTL Cache
// Mendukung multiple upstream servers + failover
// SPDX-License-Identifier: MIT

package dns

import (
	"fmt"
	"log"
	"net"
	"sync"
	"time"
)

// ── Cache Entry ───────────────────────────────────────────────────────────

type cacheEntry struct {
	ip        string
	expiresAt time.Time
}

func (e *cacheEntry) expired() bool {
	return time.Now().After(e.expiresAt)
}

// ── Resolver ──────────────────────────────────────────────────────────────

type Resolver struct {
	mu      sync.RWMutex
	cache   map[string]*cacheEntry
	servers []string
	timeout time.Duration
}

func NewResolver(servers []string) *Resolver {
	r := &Resolver{
		cache:   make(map[string]*cacheEntry),
		servers: servers,
		timeout: 5 * time.Second,
	}

	// Tambahkan beberapa entri well-known ke cache
	r.addStatic("localhost", "127.0.0.1")
	r.addStatic("neocore.local", "10.0.0.1")

	return r
}

func (r *Resolver) SetServers(servers []string) {
	r.mu.Lock()
	defer r.mu.Unlock()
	r.servers = servers
	log.Printf("[DNS  ] Updated servers: %v", servers)
}

func (r *Resolver) addStatic(name, ip string) {
	r.cache[name] = &cacheEntry{
		ip:        ip,
		expiresAt: time.Now().Add(24 * time.Hour), // static — tidak expire cepat
	}
}

// Resolve mengembalikan IP untuk hostname
// Coba cache dulu, jika miss → query upstream
func (r *Resolver) Resolve(hostname string) (string, error) {
	// 1. Cek apakah sudah berupa IP
	if ip := net.ParseIP(hostname); ip != nil {
		return hostname, nil
	}

	// 2. Cek cache
	r.mu.RLock()
	entry, found := r.cache[hostname]
	r.mu.RUnlock()

	if found && !entry.expired() {
		log.Printf("[DNS  ] Cache HIT: %s → %s", hostname, entry.ip)
		return entry.ip, nil
	}

	// 3. Query upstream
	ip, ttl, err := r.queryUpstream(hostname)
	if err != nil {
		return "", fmt.Errorf("resolve %s: %w", hostname, err)
	}

	// 4. Simpan ke cache
	r.mu.Lock()
	r.cache[hostname] = &cacheEntry{
		ip:        ip,
		expiresAt: time.Now().Add(ttl),
	}
	r.mu.Unlock()

	log.Printf("[DNS  ] Resolved %s → %s (TTL=%v)", hostname, ip, ttl)
	return ip, nil
}

// queryUpstream mencoba semua server secara berurutan (failover)
func (r *Resolver) queryUpstream(hostname string) (string, time.Duration, error) {
	r.mu.RLock()
	servers := make([]string, len(r.servers))
	copy(servers, r.servers)
	r.mu.RUnlock()

	var lastErr error
	for _, server := range servers {
		ip, ttl, err := r.queryDNS(server, hostname)
		if err != nil {
			log.Printf("[DNS  ] Server %s failed: %v", server, err)
			lastErr = err
			continue
		}
		return ip, ttl, nil
	}
	return "", 0, fmt.Errorf("all DNS servers failed for %s: %v", hostname, lastErr)
}

// queryDNS membuat raw UDP DNS query ke server
func (r *Resolver) queryDNS(server, hostname string) (string, time.Duration, error) {
	addr := net.JoinHostPort(server, "53")

	conn, err := net.DialTimeout("udp", addr, r.timeout)
	if err != nil {
		return "", 0, fmt.Errorf("dial %s: %w", addr, err)
	}
	defer conn.Close()
	conn.SetDeadline(time.Now().Add(r.timeout))

	// Build DNS query packet
	query := buildQuery(hostname)
	if _, err := conn.Write(query); err != nil {
		return "", 0, fmt.Errorf("write query: %w", err)
	}

	// Read response
	buf := make([]byte, 512)
	n, err := conn.Read(buf)
	if err != nil {
		return "", 0, fmt.Errorf("read response: %w", err)
	}

	// Parse response
	return parseResponse(buf[:n])
}

// ── DNS Packet Builder ────────────────────────────────────────────────────

func buildQuery(hostname string) []byte {
	buf := make([]byte, 0, 512)

	// Transaction ID
	buf = append(buf, 0xAB, 0xCD)
	// Flags: standard query, recursion desired
	buf = append(buf, 0x01, 0x00)
	// Questions: 1
	buf = append(buf, 0x00, 0x01)
	// Answers, Authority, Additional: 0
	buf = append(buf, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00)

	// Encode hostname (labels)
	for _, label := range splitDomain(hostname) {
		buf = append(buf, byte(len(label)))
		buf = append(buf, []byte(label)...)
	}
	buf = append(buf, 0x00) // Root label

	// Type A (1), Class IN (1)
	buf = append(buf, 0x00, 0x01, 0x00, 0x01)

	return buf
}

func splitDomain(hostname string) []string {
	var labels []string
	start := 0
	for i, c := range hostname {
		if c == '.' {
			if i > start {
				labels = append(labels, hostname[start:i])
			}
			start = i + 1
		}
	}
	if start < len(hostname) {
		labels = append(labels, hostname[start:])
	}
	return labels
}

// ── DNS Response Parser ────────────────────────────────────────────────────

func parseResponse(buf []byte) (string, time.Duration, error) {
	if len(buf) < 12 {
		return "", 0, fmt.Errorf("response too short (%d bytes)", len(buf))
	}

	// Check RCODE
	flags := uint16(buf[2])<<8 | uint16(buf[3])
	rcode := flags & 0x000F
	if rcode != 0 {
		return "", 0, fmt.Errorf("DNS error rcode=%d", rcode)
	}

	anCount := int(uint16(buf[6])<<8 | uint16(buf[7]))
	if anCount == 0 {
		return "", 0, fmt.Errorf("no answer records")
	}

	// Skip pertanyaan
	offset := 12
	for buf[offset] != 0 {
		if buf[offset]&0xC0 == 0xC0 {
			offset += 2
			break
		}
		offset += int(buf[offset]) + 1
	}
	if buf[offset] == 0 {
		offset++ // root label
	}
	offset += 4 // type + class

	// Parse answer records
	for i := 0; i < anCount && offset < len(buf); i++ {
		// Skip name (bisa pointer)
		if buf[offset]&0xC0 == 0xC0 {
			offset += 2
		} else {
			for buf[offset] != 0 {
				offset += int(buf[offset]) + 1
			}
			offset++
		}

		if offset+10 > len(buf) {
			break
		}

		rtype  := uint16(buf[offset])<<8 | uint16(buf[offset+1])
		ttlVal := uint32(buf[offset+4])<<24 | uint32(buf[offset+5])<<16 |
			uint32(buf[offset+6])<<8 | uint32(buf[offset+7])
		rdlen  := int(uint16(buf[offset+8])<<8 | uint16(buf[offset+9]))
		offset += 10

		if rtype == 1 && rdlen == 4 && offset+4 <= len(buf) {
			// Type A record
			ip := fmt.Sprintf("%d.%d.%d.%d",
				buf[offset], buf[offset+1], buf[offset+2], buf[offset+3])
			ttl := time.Duration(ttlVal) * time.Second
			if ttl < 30*time.Second {
				ttl = 30 * time.Second // minimum TTL
			}
			if ttl > 3600*time.Second {
				ttl = 3600 * time.Second // maximum cache
			}
			return ip, ttl, nil
		}
		offset += rdlen
	}

	return "", 0, fmt.Errorf("no A record found in response")
}

// ── Cache Cleanup ─────────────────────────────────────────────────────────

// CacheCleanLoop membersihkan entri cache yang expired setiap menit
func (r *Resolver) CacheCleanLoop() {
	ticker := time.NewTicker(60 * time.Second)
	defer ticker.Stop()

	for range ticker.C {
		r.mu.Lock()
		evicted := 0
		for k, v := range r.cache {
			if v.expired() {
				delete(r.cache, k)
				evicted++
			}
		}
		if evicted > 0 {
			log.Printf("[DNS  ] Cache cleanup: evicted %d expired entries (%d remaining)",
				evicted, len(r.cache))
		}
		r.mu.Unlock()
	}
}

// CacheSize mengembalikan jumlah entri dalam cache
func (r *Resolver) CacheSize() int {
	r.mu.RLock()
	defer r.mu.RUnlock()
	return len(r.cache)
}
