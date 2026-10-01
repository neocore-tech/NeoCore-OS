// pkg/types/types.go — Shared types untuk neocore-os/netmgr
// SPDX-License-Identifier: MIT

package types

import "time"

// InterfaceConfig merepresentasikan konfigurasi network interface
type InterfaceConfig struct {
	Name    string
	IP      string
	Netmask string
	MTU     int
	Up      bool
}

// RouteEntry merepresentasikan sebuah routing table entry
type RouteEntry struct {
	Destination string
	Gateway     string
	Interface   string
	Metric      int
}

// DNSConfig menyimpan konfigurasi DNS server
type DNSConfig struct {
	Servers []string
	Search  []string
	Timeout time.Duration
}

// NetworkStats statistik jaringan
type NetworkStats struct {
	RxBytes   uint64
	TxBytes   uint64
	RxPackets uint64
	TxPackets uint64
	RxErrors  uint64
	TxErrors  uint64
}
