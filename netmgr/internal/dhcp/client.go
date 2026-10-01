// internal/dhcp/client.go — DHCP Client State Machine
// RFC 2131 compliant: DISCOVER → OFFER → REQUEST → ACK
// SPDX-License-Identifier: MIT

package dhcp

import (
	"encoding/binary"
	"fmt"
	"log"
	"math/rand"
	"net"
	"time"

	"neocore-os/netmgr/internal/ipc"
)

// ── Lease ─────────────────────────────────────────────────────────────────

type Lease struct {
	IP        string
	Netmask   string
	Gateway   string
	DNS       []string
	ServerIP  string
	LeaseTime time.Duration
	AcquiredAt time.Time
}

func (l *Lease) ExpiresAt() time.Time {
	return l.AcquiredAt.Add(l.LeaseTime)
}

func (l *Lease) RenewalTime() time.Time {
	// T1 = LeaseTime / 2
	return l.AcquiredAt.Add(l.LeaseTime / 2)
}

// ── DHCP Message ──────────────────────────────────────────────────────────

const (
	dhcpDiscover = 1
	dhcpOffer    = 2
	dhcpRequest  = 3
	dhcpAck      = 5
	dhcpRelease  = 7

	dhcpPort = 67 // Server port
	clientPort = 68

	// Options
	optSubnetMask = 1
	optRouter     = 3
	optDNS        = 6
	optLeaseTime  = 51
	optMsgType    = 53
	optServerID   = 54
	optEnd        = 255
)

type dhcpMsg struct {
	op      byte
	htype   byte
	hlen    byte
	hops    byte
	xid     uint32
	secs    uint16
	flags   uint16
	ciaddr  [4]byte
	yiaddr  [4]byte
	siaddr  [4]byte
	giaddr  [4]byte
	chaddr  [16]byte
	sname   [64]byte
	file    [128]byte
	magic   [4]byte
	options []byte
}

func (m *dhcpMsg) marshal() []byte {
	buf := make([]byte, 236+len(m.options))
	buf[0] = m.op
	buf[1] = m.htype
	buf[2] = m.hlen
	buf[3] = m.hops
	binary.BigEndian.PutUint32(buf[4:8], m.xid)
	binary.BigEndian.PutUint16(buf[8:10], m.secs)
	binary.BigEndian.PutUint16(buf[10:12], m.flags)
	copy(buf[12:16], m.ciaddr[:])
	copy(buf[16:20], m.yiaddr[:])
	copy(buf[20:24], m.siaddr[:])
	copy(buf[24:28], m.giaddr[:])
	copy(buf[28:44], m.chaddr[:])
	copy(buf[108:236], m.magic[:])
	// magic cookie
	buf[236-4] = 99
	buf[236-3] = 130
	buf[236-2] = 83
	buf[236-1] = 99
	copy(buf[236:], m.options)
	return buf
}

// ── Client ────────────────────────────────────────────────────────────────

type Client struct {
	iface  string
	mac    net.HardwareAddr
	ipc    ipc.Client
	xid    uint32
}

func NewClient(iface string, kernelIPC ipc.Client) *Client {
	// Baca MAC address interface
	mac, err := getMACAddress(iface)
	if err != nil {
		// Fallback: generate random MAC
		mac = make(net.HardwareAddr, 6)
		rand.Read(mac)
		mac[0] &= 0xfe // unicast
		mac[0] |= 0x02 // locally administered
		log.Printf("[DHCP ] WARN: cannot get MAC for %s: %v — using random %v", iface, err, mac)
	}

	return &Client{
		iface: iface,
		mac:   mac,
		ipc:   kernelIPC,
		xid:   rand.Uint32(),
	}
}

func getMACAddress(iface string) (net.HardwareAddr, error) {
	ifi, err := net.InterfaceByName(iface)
	if err != nil {
		return nil, err
	}
	return ifi.HardwareAddr, nil
}

// Acquire menjalankan DHCP state machine dan mengembalikan Lease
func (c *Client) Acquire() (*Lease, error) {
	conn, err := net.ListenPacket("udp4", fmt.Sprintf("0.0.0.0:%d", clientPort))
	if err != nil {
		return nil, fmt.Errorf("listen UDP: %w", err)
	}
	defer conn.Close()

	serverAddr := &net.UDPAddr{IP: net.IPv4bcast, Port: dhcpPort}
	conn.SetDeadline(time.Now().Add(10 * time.Second))

	// ── DISCOVER ─────────────────────────────────────────────────────
	log.Printf("[DHCP ] DISCOVER → broadcast (xid=0x%08x)", c.xid)
	discover := c.buildDiscover()
	if _, err := conn.WriteTo(discover.marshal(), serverAddr); err != nil {
		return nil, fmt.Errorf("send DISCOVER: %w", err)
	}

	// ── OFFER ────────────────────────────────────────────────────────
	offer, err := c.receiveOffer(conn)
	if err != nil {
		return nil, fmt.Errorf("receive OFFER: %w", err)
	}
	log.Printf("[DHCP ] OFFER from %s → IP=%s", offer.ServerIP, offer.IP)

	// ── REQUEST ──────────────────────────────────────────────────────
	log.Printf("[DHCP ] REQUEST → server=%s for IP=%s", offer.ServerIP, offer.IP)
	request := c.buildRequest(offer)
	if _, err := conn.WriteTo(request.marshal(), serverAddr); err != nil {
		return nil, fmt.Errorf("send REQUEST: %w", err)
	}

	// ── ACK ──────────────────────────────────────────────────────────
	lease, err := c.receiveAck(conn, offer.IP)
	if err != nil {
		return nil, fmt.Errorf("receive ACK: %w", err)
	}

	lease.AcquiredAt = time.Now()
	log.Printf("[DHCP ] ACK received — lease valid for %v", lease.LeaseTime)
	return lease, nil
}

func (c *Client) buildDiscover() *dhcpMsg {
	msg := &dhcpMsg{
		op:    1, // BOOTREQUEST
		htype: 1, // Ethernet
		hlen:  6,
		xid:   c.xid,
		flags: 0x8000, // Broadcast flag
	}
	copy(msg.chaddr[:], c.mac)
	msg.options = []byte{
		optMsgType, 1, dhcpDiscover,
		55, 4, optSubnetMask, optRouter, optLeaseTime, optDNS, // Parameter request
		optEnd,
	}
	return msg
}

func (c *Client) buildRequest(offer *Lease) *dhcpMsg {
	msg := &dhcpMsg{
		op:    1,
		htype: 1,
		hlen:  6,
		xid:   c.xid,
		flags: 0x8000,
	}
	copy(msg.chaddr[:], c.mac)

	serverIP := net.ParseIP(offer.ServerIP).To4()
	requestedIP := net.ParseIP(offer.IP).To4()

	msg.options = []byte{
		optMsgType, 1, dhcpRequest,
		optServerID, 4, serverIP[0], serverIP[1], serverIP[2], serverIP[3],
		50, 4, requestedIP[0], requestedIP[1], requestedIP[2], requestedIP[3],
		optEnd,
	}
	return msg
}

func (c *Client) receiveOffer(conn net.PacketConn) (*Lease, error) {
	buf := make([]byte, 1024)
	for {
		n, _, err := conn.ReadFrom(buf)
		if err != nil {
			return nil, err
		}
		if n < 240 {
			continue
		}
		// Check XID dan message type
		xid := binary.BigEndian.Uint32(buf[4:8])
		if xid != c.xid {
			continue
		}
		msgType, ok := findOption(buf[240:n], optMsgType)
		if !ok || len(msgType) < 1 || msgType[0] != dhcpOffer {
			continue
		}
		return parseLeaseFromMsg(buf[:n]), nil
	}
}

func (c *Client) receiveAck(conn net.PacketConn, requestedIP string) (*Lease, error) {
	buf := make([]byte, 1024)
	conn.SetDeadline(time.Now().Add(10 * time.Second))
	for {
		n, _, err := conn.ReadFrom(buf)
		if err != nil {
			return nil, err
		}
		if n < 240 {
			continue
		}
		xid := binary.BigEndian.Uint32(buf[4:8])
		if xid != c.xid {
			continue
		}
		msgType, ok := findOption(buf[240:n], optMsgType)
		if !ok || len(msgType) < 1 || msgType[0] != dhcpAck {
			continue
		}
		lease := parseLeaseFromMsg(buf[:n])
		return lease, nil
	}
}

// Release mengirimkan DHCP RELEASE ke server
func (c *Client) Release(lease *Lease) {
	log.Printf("[DHCP ] Releasing lease %s...", lease.IP)
	// Kirim DHCPRELEASE ke server (best effort)
	conn, err := net.Dial("udp4", fmt.Sprintf("%s:%d", lease.ServerIP, dhcpPort))
	if err != nil {
		return
	}
	defer conn.Close()

	serverIP := net.ParseIP(lease.ServerIP).To4()
	ciaddr   := net.ParseIP(lease.IP).To4()
	msg := &dhcpMsg{
		op:    1,
		htype: 1,
		hlen:  6,
		xid:   c.xid,
	}
	copy(msg.chaddr[:], c.mac)
	copy(msg.ciaddr[:], ciaddr)

	msg.options = []byte{
		optMsgType, 1, dhcpRelease,
		optServerID, 4, serverIP[0], serverIP[1], serverIP[2], serverIP[3],
		optEnd,
	}
	conn.Write(msg.marshal())
	log.Printf("[DHCP ] Lease released")
}

// RenewLoop memperbarui lease sebelum expire (T1 timer)
func (c *Client) RenewLoop(lease *Lease) {
	for {
		renewAt := lease.RenewalTime()
		now := time.Now()
		if renewAt.After(now) {
			time.Sleep(renewAt.Sub(now))
		}

		log.Printf("[DHCP ] Renewing lease (expires %v)...", lease.ExpiresAt())
		newLease, err := c.Acquire()
		if err != nil {
			log.Printf("[DHCP ] Renewal failed: %v — will retry in 30s", err)
			time.Sleep(30 * time.Second)
			continue
		}
		*lease = *newLease
		log.Printf("[DHCP ] Lease renewed: %s for %v", lease.IP, lease.LeaseTime)
	}
}

// ── Helpers ───────────────────────────────────────────────────────────────

func parseLeaseFromMsg(buf []byte) *Lease {
	lease := &Lease{}

	if len(buf) >= 20 {
		lease.IP = fmt.Sprintf("%d.%d.%d.%d",
			buf[16], buf[17], buf[18], buf[19])
	}
	if len(buf) >= 24 {
		lease.ServerIP = fmt.Sprintf("%d.%d.%d.%d",
			buf[20], buf[21], buf[22], buf[23])
	}

	if len(buf) < 240 {
		return lease
	}
	opts := buf[240:]

	if mask, ok := findOption(opts, optSubnetMask); ok && len(mask) >= 4 {
		lease.Netmask = fmt.Sprintf("%d.%d.%d.%d", mask[0], mask[1], mask[2], mask[3])
	}
	if gw, ok := findOption(opts, optRouter); ok && len(gw) >= 4 {
		lease.Gateway = fmt.Sprintf("%d.%d.%d.%d", gw[0], gw[1], gw[2], gw[3])
	}
	if lt, ok := findOption(opts, optLeaseTime); ok && len(lt) >= 4 {
		secs := binary.BigEndian.Uint32(lt[:4])
		lease.LeaseTime = time.Duration(secs) * time.Second
	}
	if dnsData, ok := findOption(opts, optDNS); ok {
		for i := 0; i+3 < len(dnsData); i += 4 {
			lease.DNS = append(lease.DNS,
				fmt.Sprintf("%d.%d.%d.%d",
					dnsData[i], dnsData[i+1], dnsData[i+2], dnsData[i+3]))
		}
	}

	if lease.Netmask == "" {
		lease.Netmask = "255.255.255.0"
	}
	if lease.LeaseTime == 0 {
		lease.LeaseTime = 3600 * time.Second
	}

	return lease
}

func findOption(opts []byte, code byte) ([]byte, bool) {
	i := 0
	for i < len(opts) {
		if opts[i] == optEnd {
			break
		}
		if opts[i] == 0 { // PAD
			i++
			continue
		}
		if i+1 >= len(opts) {
			break
		}
		length := int(opts[i+1])
		if opts[i] == code {
			if i+2+length <= len(opts) {
				return opts[i+2 : i+2+length], true
			}
		}
		i += 2 + length
	}
	return nil, false
}
