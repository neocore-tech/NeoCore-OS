// internal/firewall/firewall.go — Packet Filter / Firewall Engine
// Rule-based stateless firewall dengan stateful connection tracking sederhana
// SPDX-License-Identifier: MIT

package firewall

import (
	"fmt"
	"log"
	"sync"
)

// ── Policy ────────────────────────────────────────────────────────────────

type Policy int

const (
	PolicyAccept Policy = iota
	PolicyDrop
	PolicyReject
)

func (p Policy) String() string {
	switch p {
	case PolicyAccept:
		return "ACCEPT"
	case PolicyDrop:
		return "DROP"
	case PolicyReject:
		return "REJECT"
	default:
		return "UNKNOWN"
	}
}

// ── Protocol ──────────────────────────────────────────────────────────────

type Protocol int

const (
	ProtoAny  Protocol = 0
	ProtoTCP  Protocol = 6
	ProtoUDP  Protocol = 17
	ProtoICMP Protocol = 1
)

func (p Protocol) String() string {
	switch p {
	case ProtoAny:
		return "any"
	case ProtoTCP:
		return "tcp"
	case ProtoUDP:
		return "udp"
	case ProtoICMP:
		return "icmp"
	default:
		return fmt.Sprintf("proto(%d)", int(p))
	}
}

// ── Direction ─────────────────────────────────────────────────────────────

type Direction int

const (
	DirInbound  Direction = iota
	DirOutbound Direction = iota
	DirBoth     Direction = iota
)

// ── Rule ──────────────────────────────────────────────────────────────────

type Rule struct {
	ID        int
	Name      string
	Direction Direction
	Protocol  Protocol
	SrcIP     string // "" = any
	DstIP     string // "" = any
	SrcPort   uint16 // 0 = any
	DstPort   uint16 // 0 = any
	Action    Policy
}

func (r *Rule) String() string {
	return fmt.Sprintf("#%d [%s] %s %s src=%s:%d dst=%s:%d → %s",
		r.ID, r.Name,
		func() string {
			switch r.Direction {
			case DirInbound:
				return "IN"
			case DirOutbound:
				return "OUT"
			default:
				return "BOTH"
			}
		}(),
		r.Protocol,
		r.SrcIP, r.SrcPort,
		r.DstIP, r.DstPort,
		r.Action,
	)
}

// ── Predefined Rule Constructors ──────────────────────────────────────────

func RuleICMP() *Rule {
	return &Rule{
		Name:      "Allow ICMP",
		Direction: DirBoth,
		Protocol:  ProtoICMP,
		Action:    PolicyAccept,
	}
}

func RuleDHCP() *Rule {
	return &Rule{
		Name:      "Allow DHCP",
		Direction: DirBoth,
		Protocol:  ProtoUDP,
		SrcPort:   68,
		DstPort:   67,
		Action:    PolicyAccept,
	}
}

func RuleEstablished() *Rule {
	return &Rule{
		Name:      "Allow Established",
		Direction: DirInbound,
		Protocol:  ProtoAny,
		Action:    PolicyAccept,
		// Note: stateful matching dikerjakan oleh kernel (smoltcp)
		// Rule ini sebagai dokumentasi; enforcement di kernel side
	}
}

func RuleOutbound() *Rule {
	return &Rule{
		Name:      "Allow All Outbound",
		Direction: DirOutbound,
		Protocol:  ProtoAny,
		Action:    PolicyAccept,
	}
}

func RuleAllowTCPPort(port uint16, name string) *Rule {
	return &Rule{
		Name:      name,
		Direction: DirInbound,
		Protocol:  ProtoTCP,
		DstPort:   port,
		Action:    PolicyAccept,
	}
}

func RuleBlockIP(ip string) *Rule {
	return &Rule{
		Name:      fmt.Sprintf("Block %s", ip),
		Direction: DirBoth,
		Protocol:  ProtoAny,
		SrcIP:     ip,
		Action:    PolicyDrop,
	}
}

// ── Firewall ──────────────────────────────────────────────────────────────

type Firewall struct {
	mu            sync.RWMutex
	rules         []*Rule
	defaultPolicy Policy
	nextID        int
	stats         Stats
}

type Stats struct {
	Accepted uint64
	Dropped  uint64
	Rejected uint64
}

func New() *Firewall {
	return &Firewall{
		defaultPolicy: PolicyDrop,
		nextID:        1,
	}
}

func (f *Firewall) SetDefaultPolicy(p Policy) {
	f.mu.Lock()
	f.defaultPolicy = p
	f.mu.Unlock()
	log.Printf("[FW   ] Default policy: %s", p)
}

// Allow menambahkan allow rule ke akhir chain
func (f *Firewall) Allow(r *Rule) {
	r.Action = PolicyAccept
	f.addRule(r)
}

// Block menambahkan drop rule ke akhir chain
func (f *Firewall) Block(r *Rule) {
	r.Action = PolicyDrop
	f.addRule(r)
}

func (f *Firewall) addRule(r *Rule) {
	f.mu.Lock()
	defer f.mu.Unlock()
	r.ID = f.nextID
	f.nextID++
	f.rules = append(f.rules, r)
	log.Printf("[FW   ] Rule added: %s", r)
}

// DelRule menghapus rule berdasarkan ID
func (f *Firewall) DelRule(id int) error {
	f.mu.Lock()
	defer f.mu.Unlock()
	for i, r := range f.rules {
		if r.ID == id {
			f.rules = append(f.rules[:i], f.rules[i+1:]...)
			log.Printf("[FW   ] Rule #%d removed", id)
			return nil
		}
	}
	return fmt.Errorf("rule #%d not found", id)
}

// ListRules mengembalikan semua rules
func (f *Firewall) ListRules() []*Rule {
	f.mu.RLock()
	defer f.mu.RUnlock()
	result := make([]*Rule, len(f.rules))
	copy(result, f.rules)
	return result
}

// Match memeriksa apakah paket cocok dengan rule
// Returns PolicyAccept/PolicyDrop sesuai rule pertama yang cocok
// atau defaultPolicy jika tidak ada rule yang cocok
func (f *Firewall) Match(proto Protocol, dir Direction, srcIP, dstIP string, srcPort, dstPort uint16) Policy {
	f.mu.RLock()
	defer f.mu.RUnlock()

	for _, r := range f.rules {
		if r.Direction != DirBoth && r.Direction != dir {
			continue
		}
		if r.Protocol != ProtoAny && r.Protocol != proto {
			continue
		}
		if r.SrcIP != "" && r.SrcIP != srcIP {
			continue
		}
		if r.DstIP != "" && r.DstIP != dstIP {
			continue
		}
		if r.SrcPort != 0 && r.SrcPort != srcPort {
			continue
		}
		if r.DstPort != 0 && r.DstPort != dstPort {
			continue
		}

		// Match!
		switch r.Action {
		case PolicyAccept:
			f.stats.Accepted++
		case PolicyDrop:
			f.stats.Dropped++
		case PolicyReject:
			f.stats.Rejected++
		}
		return r.Action
	}

	// Default policy
	switch f.defaultPolicy {
	case PolicyAccept:
		f.stats.Accepted++
	case PolicyDrop:
		f.stats.Dropped++
	}
	return f.defaultPolicy
}

// GetStats mengembalikan statistik
func (f *Firewall) GetStats() Stats {
	f.mu.RLock()
	defer f.mu.RUnlock()
	return f.stats
}

// PrintRules mencetak semua rules ke log
func (f *Firewall) PrintRules() {
	rules := f.ListRules()
	log.Printf("[FW   ] Firewall rules (%d) — default: %s", len(rules), f.defaultPolicy)
	for _, r := range rules {
		log.Printf("[FW   ]   %s", r)
	}
}
