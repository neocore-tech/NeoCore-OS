// internal/ipc/client.go — IPC Client: Go neonetd ↔ Kernel /dev/netctl
// Protokol: Header 12 bytes + payload
// SPDX-License-Identifier: MIT

package ipc

import (
	"encoding/binary"
	"fmt"
	"io"
	"log"
	"os"
	"sync"
	"sync/atomic"
	"time"
)

// ── Protocol Constants ────────────────────────────────────────────────────

const (
	HeaderSize = 12 // bytes: length(4) + msg_type(2) + flags(2) + seq(4)

	MsgSetInterface = 0x0001
	MsgGetInterface = 0x0002
	MsgAddRoute     = 0x0003
	MsgDelRoute     = 0x0004
	MsgGetRoutes    = 0x0005
	MsgAddFilter    = 0x0006
	MsgDelFilter    = 0x0007
	MsgGetStats     = 0x0008
	MsgSetDNS       = 0x0009
	MsgAck          = 0x00FF

	FlagRequest  = 0x0001
	FlagResponse = 0x0002
	FlagError    = 0x0004

	DefaultTimeout = 5 * time.Second
)

// ── Header ────────────────────────────────────────────────────────────────

type Header struct {
	Length  uint32
	MsgType uint16
	Flags   uint16
	Seq     uint32
}

func (h *Header) Marshal() []byte {
	buf := make([]byte, HeaderSize)
	binary.LittleEndian.PutUint32(buf[0:4], h.Length)
	binary.LittleEndian.PutUint16(buf[4:6], h.MsgType)
	binary.LittleEndian.PutUint16(buf[6:8], h.Flags)
	binary.LittleEndian.PutUint32(buf[8:12], h.Seq)
	return buf
}

func unmarshalHeader(buf []byte) Header {
	return Header{
		Length:  binary.LittleEndian.Uint32(buf[0:4]),
		MsgType: binary.LittleEndian.Uint16(buf[4:6]),
		Flags:   binary.LittleEndian.Uint16(buf[6:8]),
		Seq:     binary.LittleEndian.Uint32(buf[8:12]),
	}
}

// ── Message ───────────────────────────────────────────────────────────────

type Message struct {
	Header  Header
	Payload []byte
}

// ── Client ────────────────────────────────────────────────────────────────

type Client interface {
	SendSetInterface(iface, ip, netmask string) error
	SendSetDNS(servers []string) error
	SendAddRoute(dst, gw, iface string) error
	SendDelRoute(dst string) error
	Close() error
}

// ── Real Client (kernel /dev/netctl) ─────────────────────────────────────

type realClient struct {
	f       *os.File
	mu      sync.Mutex
	seqNext uint32
}

func NewClient(devPath string) (Client, error) {
	f, err := os.OpenFile(devPath, os.O_RDWR, 0)
	if err != nil {
		return nil, fmt.Errorf("open %s: %w", devPath, err)
	}
	return &realClient{f: f}, nil
}

func (c *realClient) nextSeq() uint32 {
	return atomic.AddUint32(&c.seqNext, 1)
}

func (c *realClient) send(msgType uint16, payload []byte) error {
	c.mu.Lock()
	defer c.mu.Unlock()

	hdr := Header{
		Length:  uint32(HeaderSize + len(payload)),
		MsgType: msgType,
		Flags:   FlagRequest,
		Seq:     c.nextSeq(),
	}

	_, err := c.f.Write(hdr.Marshal())
	if err != nil {
		return fmt.Errorf("write header: %w", err)
	}
	if len(payload) > 0 {
		_, err = c.f.Write(payload)
		if err != nil {
			return fmt.Errorf("write payload: %w", err)
		}
	}

	// Baca ACK dari kernel
	return c.readAck(hdr.Seq)
}

func (c *realClient) readAck(expectedSeq uint32) error {
	c.f.SetReadDeadline(time.Now().Add(DefaultTimeout))
	defer c.f.SetReadDeadline(time.Time{})

	buf := make([]byte, HeaderSize)
	if _, err := io.ReadFull(c.f, buf); err != nil {
		return fmt.Errorf("read ack header: %w", err)
	}

	ack := unmarshalHeader(buf)
	if ack.Flags&FlagError != 0 {
		return fmt.Errorf("kernel returned error for seq=%d", expectedSeq)
	}
	if ack.MsgType != MsgAck {
		return fmt.Errorf("expected ACK (0x%04x), got 0x%04x", MsgAck, ack.MsgType)
	}
	return nil
}

func (c *realClient) SendSetInterface(iface, ip, netmask string) error {
	// Format payload: "iface\0ip\0netmask\0"
	payload := []byte(fmt.Sprintf("%s\x00%s\x00%s\x00", iface, ip, netmask))
	log.Printf("[IPC  ] → SetInterface iface=%s ip=%s mask=%s", iface, ip, netmask)
	return c.send(MsgSetInterface, payload)
}

func (c *realClient) SendSetDNS(servers []string) error {
	var payload string
	for _, s := range servers {
		payload += s + "\x00"
	}
	log.Printf("[IPC  ] → SetDNS servers=%v", servers)
	return c.send(MsgSetDNS, []byte(payload))
}

func (c *realClient) SendAddRoute(dst, gw, iface string) error {
	payload := []byte(fmt.Sprintf("%s\x00%s\x00%s\x00", dst, gw, iface))
	log.Printf("[IPC  ] → AddRoute dst=%s gw=%s iface=%s", dst, gw, iface)
	return c.send(MsgAddRoute, payload)
}

func (c *realClient) SendDelRoute(dst string) error {
	payload := []byte(fmt.Sprintf("%s\x00", dst))
	log.Printf("[IPC  ] → DelRoute dst=%s", dst)
	return c.send(MsgDelRoute, payload)
}

func (c *realClient) Close() error {
	if c.f != nil {
		return c.f.Close()
	}
	return nil
}

// ── Simulated Client (untuk development di host Linux) ───────────────────

type simulatedClient struct{}

func NewSimulatedClient() Client {
	log.Printf("[IPC  ] Using SIMULATED kernel IPC (no /dev/netctl)")
	return &simulatedClient{}
}

func (s *simulatedClient) SendSetInterface(iface, ip, netmask string) error {
	log.Printf("[IPC-SIM] SetInterface iface=%s ip=%s mask=%s", iface, ip, netmask)
	return nil
}

func (s *simulatedClient) SendSetDNS(servers []string) error {
	log.Printf("[IPC-SIM] SetDNS servers=%v", servers)
	return nil
}

func (s *simulatedClient) SendAddRoute(dst, gw, iface string) error {
	log.Printf("[IPC-SIM] AddRoute dst=%s gw=%s iface=%s", dst, gw, iface)
	return nil
}

func (s *simulatedClient) SendDelRoute(dst string) error {
	log.Printf("[IPC-SIM] DelRoute dst=%s", dst)
	return nil
}

func (s *simulatedClient) Close() error { return nil }
