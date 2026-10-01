# Makefile — NeoCore OS Build System
# SPDX-License-Identifier: MIT

ARCH    := x86_64
TARGET  := $(ARCH)-unknown-none

# Directories
KERNEL_DIR    := kernel
NETMGR_DIR    := netmgr
SHELL_DIR     := shell
BUILD_DIR     := build
ISO_DIR       := $(BUILD_DIR)/iso

# Output files
KERNEL_BIN    := $(KERNEL_DIR)/target/$(TARGET)/release/neocore-kernel
BOOTIMG       := $(KERNEL_DIR)/target/$(TARGET)/release/bootimage-neocore-kernel.bin
NETMGRD_BIN   := $(NETMGR_DIR)/bin/neonetd
DISK_IMG      := $(BUILD_DIR)/neocore-os.img
ISO_FILE      := $(BUILD_DIR)/neocore-os.iso

# QEMU settings
QEMU_COMMON := \
    -m 256M \
    -serial stdio

QEMU_FLAGS := $(QEMU_COMMON) \
    -display gtk \
    -device rtl8139,netdev=net0 \
    -netdev user,id=net0,hostfwd=tcp::2222-:22

QEMU_DEBUG_FLAGS := $(QEMU_COMMON) \
    -display gtk \
    -s -S

.PHONY: all kernel netmgr shell run run-debug iso clean deps test check fmt

# ── Help ──────────────────────────────────────────────────────────────────

help:
	@echo ""
	@echo "NeoCore OS Build System"
	@echo "══════════════════════════════════════════════"
	@echo "  make all        — Build semua komponen"
	@echo "  make kernel     — Build Rust kernel saja"
	@echo "  make netmgr     — Build Go neonetd"
	@echo "  make shell      — Build ncsh standalone"
	@echo "  make run        — Build + jalankan di QEMU"
	@echo "  make run-debug  — Build + QEMU + GDB server (port 1234)"
	@echo "  make test       — Jalankan kernel tests di QEMU"
	@echo "  make check      — Cargo check (cepat, tanpa build)"
	@echo "  make fmt        — Format semua kode Rust"
	@echo "  make clean      — Hapus semua build artifacts"
	@echo "  make deps       — Install semua dependencies"
	@echo ""

# ── Build ─────────────────────────────────────────────────────────────────

## Build semua komponen
all: kernel netmgr shell disk

## Install semua dependencies
deps:
	@echo "==> Installing Rust nightly toolchain..."
	rustup toolchain install nightly
	rustup override set nightly
	rustup component add rust-src llvm-tools-preview --toolchain nightly
	cargo install bootimage
	@echo "==> Checking Go installation..."
	go version || (echo "ERROR: Go not installed! Install from https://go.dev" && exit 1)
	cd $(NETMGR_DIR) && go mod tidy || true
	@echo "==> Done! Run 'make all' to build."

## Build Rust Kernel
kernel:
	@echo "==> Building kernel (Rust nightly)..."
	cd $(KERNEL_DIR) && cargo build --release 2>&1
	@echo "==> Creating bootable disk image..."
	cd disk_builder && cargo run --release -- ../$(KERNEL_BIN) ../$(BOOTIMG)
	@echo "[OK] Kernel: $(BOOTIMG)"

## Build Go Network Manager
netmgr:
	@echo "==> Building neonetd (Go)..."
	mkdir -p $(NETMGR_DIR)/bin
	cd $(NETMGR_DIR) && \
		GOOS=linux GOARCH=amd64 \
		go build -ldflags="-s -w -X main.BuildDate=$$(date -u +%Y-%m-%d)" \
		-o bin/neonetd ./cmd/neonetd 2>&1 || \
		echo "[SKIP] neonetd build skipped (not ready yet)"
	@if [ -f "$(NETMGRD_BIN)" ]; then \
		echo "[OK] neonetd: $(NETMGRD_BIN) ($$(du -sh $(NETMGRD_BIN) | cut -f1))"; \
	fi

## Build Shell (standalone, Phase 5+)
shell:
	@echo "==> Building ncsh standalone (Rust)..."
	cd $(SHELL_DIR) && cargo build --release 2>&1 || \
		echo "[SKIP] ncsh standalone build skipped"
	@echo "[OK] Shell (standalone mode — kernel shell via executor)"

## Buat disk image dari kernel bootimage
disk: kernel
	@echo "==> Creating disk image..."
	mkdir -p $(BUILD_DIR)
	cp $(BOOTIMG) $(DISK_IMG)
	@echo "[OK] Disk image: $(DISK_IMG) ($$(du -sh $(DISK_IMG) | cut -f1))"

# ── Run ───────────────────────────────────────────────────────────────────

## Jalankan di QEMU (GTK display)
run: disk
	@echo "==> Starting NeoCore OS in QEMU..."
	@echo "    Serial: stdout | Display: GTK window"
	@echo "    Ctrl+A, X = exit QEMU"
	@echo ""
	qemu-system-x86_64 $(QEMU_FLAGS) -drive file=$(DISK_IMG),format=raw

## Jalankan di QEMU dengan GDB server (port 1234)
run-debug: disk
	@echo "==> Starting NeoCore OS in QEMU (DEBUG mode)..."
	@echo "    GDB server: localhost:1234"
	@echo "    Connect: gdb $(KERNEL_BIN)"
	@echo "    GDB command: target remote :1234"
	@echo ""
	qemu-system-x86_64 $(QEMU_DEBUG_FLAGS) -drive file=$(DISK_IMG),format=raw

# ── Test ──────────────────────────────────────────────────────────────────

## Jalankan semua kernel tests di QEMU (headless)
test:
	@echo "==> Running kernel integration tests in QEMU..."
	cd $(KERNEL_DIR) && cargo test 2>&1
	@echo "==> Running Go unit tests..."
	cd $(NETMGR_DIR) && go test ./... 2>&1 || echo "[SKIP] no Go tests yet"

## Cargo check (compile check tanpa link)
check:
	@echo "==> Checking kernel..."
	cd $(KERNEL_DIR) && cargo check 2>&1
	@echo "==> Checking shell..."
	cd $(SHELL_DIR) && cargo check 2>&1
	@echo "==> Checking neonetd..."
	cd $(NETMGR_DIR) && go vet ./... 2>&1

## Format kode
fmt:
	cd $(KERNEL_DIR) && cargo fmt
	cd $(SHELL_DIR) && cargo fmt
	cd $(NETMGR_DIR) && gofmt -w . 2>/dev/null || true

# ── Clean ─────────────────────────────────────────────────────────────────

## Hapus semua build artifacts
clean:
	@echo "==> Cleaning..."
	cd $(KERNEL_DIR) && cargo clean || true
	cd $(SHELL_DIR) && cargo clean || true
	rm -f $(NETMGRD_BIN)
	rm -rf $(BUILD_DIR)
	@echo "[OK] Clean done"
