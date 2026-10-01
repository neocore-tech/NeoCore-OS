# 🔧 BUILD — Cara Build dan Menjalankan NeoCore OS

---

## 1. Prerequisites

### 1.1 Rust Toolchain

```bash
# Install Rust (jika belum ada)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install nightly (diperlukan untuk no_std OS dev)
rustup toolchain install nightly
rustup override set nightly

# Install komponen yang diperlukan
rustup component add rust-src --toolchain nightly
rustup component add llvm-tools-preview --toolchain nightly

# Install cargo tools
cargo install cargo-bootimage
cargo install bootimage
```

### 1.2 Go Toolchain

```bash
# Install Go (versi 1.22+)
# Download dari https://go.dev/dl/

# Atau via package manager (Ubuntu/Debian):
sudo apt install golang-go

# Verify
go version  # go1.22.x linux/amd64
```

### 1.3 QEMU (untuk testing)

```bash
# Ubuntu/Debian
sudo apt install qemu-system-x86

# Fedora/RHEL
sudo dnf install qemu-system-x86

# Arch
sudo pacman -S qemu-system-x86

# Verify
qemu-system-x86_64 --version
```

### 1.4 Build Dependencies

```bash
# Ubuntu/Debian
sudo apt install \
    build-essential \
    nasm \           # Assembler untuk stage1 bootloader
    grub-pc-bin \    # GRUB tools (opsional)
    xorriso \        # Untuk membuat ISO
    mtools           # FAT filesystem tools
```

---

## 2. Struktur Build System

```
neocore-os/
├── Makefile          # Master build file
├── kernel/           # Rust kernel
│   ├── Cargo.toml
│   └── .cargo/config.toml
├── netmgr/           # Go network manager
│   └── go.mod
├── shell/            # Rust shell
│   └── Cargo.toml
└── bootloader/       # Rust bootloader
    └── Cargo.toml
```

---

## 3. Makefile

```makefile
# Makefile — NeoCore OS Build System

ARCH    := x86_64
TARGET  := $(ARCH)-unknown-none

# Directories
KERNEL_DIR    := kernel
NETMGR_DIR    := netmgr
SHELL_DIR     := shell
BUILD_DIR     := build
ISO_DIR       := $(BUILD_DIR)/iso

# Output files
KERNEL_BIN    := $(KERNEL_DIR)/target/$(TARGET)/release/kernel
NETMGRD_BIN   := $(NETMGR_DIR)/bin/neonetd
DISK_IMG      := $(BUILD_DIR)/neocore-os.img
ISO_FILE      := $(BUILD_DIR)/neocore-os.iso

# QEMU settings
QEMU_FLAGS := \
    -m 256M \
    -serial stdio \
    -display gtk \
    -device e1000,netdev=net0 \
    -netdev user,id=net0,hostfwd=tcp::2222-:22 \
    -drive file=$(DISK_IMG),format=raw

QEMU_DEBUG_FLAGS := \
    $(QEMU_FLAGS) \
    -s \
    -S \
    -d int,cpu_reset

.PHONY: all kernel netmgr shell run run-debug iso clean deps test

## Build semua komponen
all: kernel netmgr shell disk

## Install semua dependencies
deps:
	@echo "==> Installing Rust nightly..."
	rustup toolchain install nightly
	rustup override set nightly
	rustup component add rust-src llvm-tools-preview
	cargo install cargo-bootimage
	@echo "==> Go dependencies..."
	cd $(NETMGR_DIR) && go mod download
	@echo "==> Done!"

## Build Rust Kernel
kernel:
	@echo "==> Building kernel (Rust)..."
	cd $(KERNEL_DIR) && cargo +nightly bootimage --release
	@echo "[OK] Kernel built: $(KERNEL_BIN)"

## Build Go Network Manager
netmgr:
	@echo "==> Building neonetd (Go)..."
	mkdir -p $(NETMGR_DIR)/bin
	cd $(NETMGR_DIR) && \
		GOOS=linux GOARCH=amd64 \
		go build -ldflags="-s -w" \
		-o bin/neonetd ./cmd/neonetd
	@echo "[OK] neonetd built: $(NETMGRD_BIN)"

## Build Shell
shell:
	@echo "==> Building ncsh (Rust)..."
	cd $(SHELL_DIR) && cargo +nightly build --release
	@echo "[OK] Shell built"

## Buat disk image
disk: kernel netmgr
	@echo "==> Creating disk image..."
	mkdir -p $(BUILD_DIR)
	
	# Buat blank disk (64MB)
	dd if=/dev/zero of=$(DISK_IMG) bs=1M count=64 status=none
	
	# Copy kernel image
	cp $(KERNEL_DIR)/target/$(TARGET)/release/bootimage-kernel.bin $(DISK_IMG)
	
	@echo "[OK] Disk image: $(DISK_IMG) ($(shell du -sh $(DISK_IMG) | cut -f1))"

## Buat ISO (bootable dengan GRUB)
iso: kernel netmgr
	@echo "==> Creating bootable ISO..."
	mkdir -p $(ISO_DIR)/boot/grub
	
	cp $(KERNEL_BIN) $(ISO_DIR)/boot/kernel.bin
	
	cat > $(ISO_DIR)/boot/grub/grub.cfg << 'EOF'
set timeout=3
set default=0

menuentry "NeoCore OS 0.1.0" {
    multiboot2 /boot/kernel.bin
    boot
}
EOF
	
	grub-mkrescue -o $(ISO_FILE) $(ISO_DIR)
	@echo "[OK] ISO: $(ISO_FILE)"

## Jalankan di QEMU
run: disk
	@echo "==> Starting NeoCore OS in QEMU..."
	qemu-system-x86_64 $(QEMU_FLAGS) -drive file=$(DISK_IMG),format=raw

## Jalankan dengan GDB debug mode
run-debug: disk
	@echo "==> Starting QEMU in debug mode (port 1234)..."
	@echo "==> In another terminal: rust-gdb target/kernel -ex 'target remote :1234'"
	qemu-system-x86_64 $(QEMU_DEBUG_FLAGS) -drive file=$(DISK_IMG),format=raw

## Jalankan tests
test:
	@echo "==> Running kernel tests..."
	cd $(KERNEL_DIR) && cargo +nightly test
	@echo "==> Running netmgr tests (Go)..."
	cd $(NETMGR_DIR) && go test ./...
	@echo "==> All tests passed!"

## Clean build artifacts
clean:
	@echo "==> Cleaning..."
	cd $(KERNEL_DIR) && cargo clean
	cd $(NETMGR_DIR) && rm -f bin/neonetd
	cd $(SHELL_DIR) && cargo clean
	rm -rf $(BUILD_DIR)
	@echo "[OK] Clean done"

## Tampilkan informasi build
info:
	@echo "Rust version: $(shell rustc --version)"
	@echo "Go version:   $(shell go version)"
	@echo "QEMU version: $(shell qemu-system-x86_64 --version | head -1)"
	@echo "Target arch:  $(TARGET)"
	@echo "Build dir:    $(BUILD_DIR)"
```

---

## 4. Kernel — .cargo/config.toml

```toml
# kernel/.cargo/config.toml

[unstable]
build-std = ["core", "compiler_builtins", "alloc"]
build-std-features = ["compiler-builtins-mem"]

[build]
target = "x86_64-unknown-neocore-os.json"

[target.'cfg(target_os = "none")']
runner = "bootimage runner"

rustflags = [
    # Link dengan linker script kustom
    "-C", "link-arg=-Tlinker.ld",
    # Optimasi
    "-C", "opt-level=2",
    # Disable stack canaries (tidak tersedia di no_std)
    "-C", "no-stack-check",
]
```

### Target JSON

```json
{
  "llvm-target": "x86_64-unknown-none",
  "data-layout": "e-m:e-p270:32:32-p271:32:32-p272:64:64-i64:64-f80:128-n8:16:32:64-S128",
  "arch": "x86_64",
  "target-endian": "little",
  "target-pointer-width": "64",
  "target-c-int-width": "32",
  "os": "none",
  "executables": true,
  "linker-flavor": "ld.lld",
  "linker": "rust-lld",
  "panic-strategy": "abort",
  "disable-redzone": true,
  "features": "-mmx,-sse,+soft-float"
}
```

---

## 5. Quick Start Commands

```bash
# Clone
git clone https://github.com/username/neocore-os.git
cd neocore-os

# Setup (sekali saja)
make deps

# Build semua
make all

# Jalankan!
make run
```

---

## 6. Build Individual Components

### Kernel saja:
```bash
cd kernel
cargo +nightly bootimage --release
```

### Network Manager saja:
```bash
cd netmgr
GOOS=linux GOARCH=amd64 go build -o bin/neonetd ./cmd/neonetd
```

### Test kernel:
```bash
cd kernel
cargo +nightly test
```

### Test Go code:
```bash
cd netmgr
go test -v ./...
```

---

## 7. Debugging

### 7.1 Serial Output

Kernel mengirim log ke serial port (COM1). QEMU meneruskannya ke stdout:

```bash
make run
# Log kernel muncul di terminal
```

Output contoh:
```
🦀 NeoCore OS v0.1.0 — Starting...
[OK] GDT initialized
[OK] IDT initialized
[OK] Interrupts enabled
[OK] Memory manager initialized
[OK] Network stack initialized
[OK] Scheduler initialized
✅ NeoCore OS Boot Complete!
[NET] VirtIO-net MAC: 52:54:00:12:34:56
[DHCP] Starting DHCP on eth0
[DHCP] Sending DISCOVER...
[DHCP] Got OFFER: 10.0.2.15 from 10.0.2.2
[DHCP] ✅ Bound! IP=10.0.2.15 GW=10.0.2.2 DNS=[10.0.2.3]
```

### 7.2 GDB Debugging

```bash
# Terminal 1: Start QEMU dengan debug mode
make run-debug

# Terminal 2: Attach GDB
rust-gdb kernel/target/x86_64-unknown-none/release/kernel \
    -ex "target remote :1234" \
    -ex "break kernel_main" \
    -ex "continue"
```

### 7.3 QEMU Monitor

Saat QEMU berjalan, tekan `Ctrl+Alt+2` untuk masuk ke QEMU Monitor:
```
(qemu) info mem      # Lihat memory mapping
(qemu) info cpus     # Info CPU
(qemu) info network  # Network stats
(qemu) x/10i $rip   # Disassemble dari RIP
(qemu) quit          # Quit QEMU
```

---

## 8. Common Build Errors & Fixes

### Error: `error[E0463]: can't find crate for std`
```bash
# Fix: Pastikan nightly digunakan
rustup override set nightly
rustup component add rust-src
```

### Error: `bootimage: No such file or directory`
```bash
# Fix: Install bootimage
cargo install bootimage
```

### Error: `QEMU: No bootable device`
```bash
# Fix: Pastikan kernel di-build dengan bootimage
cd kernel && cargo +nightly bootimage --release
# bukan: cargo build
```

### Error: Go build `undefined: ipc.IfaceConfig`
```bash
# Fix: Pastikan semua file di internal/ipc/ ada
ls netmgr/internal/ipc/
# Harus ada: client.go, protocol.go, messages.go
```

### Error: `can't find linker script`
```bash
# Fix: Pastikan linker.ld ada di kernel/
ls kernel/linker.ld
# Jika tidak ada, buat dari template di BUILD.md section 9
```

---

## 9. Linker Script

```ld
/* kernel/linker.ld */
ENTRY(_start)
OUTPUT_FORMAT(elf64-x86-64)

SECTIONS
{
    /* Kernel dimuat di higher half: 0xFFFFFFFF80000000 */
    . = 0xFFFFFFFF80000000;

    .text ALIGN(4K) :
    {
        KEEP(*(.text.boot))   /* Bootloader entry */
        *(.text .text.*)      /* Kernel code */
    }

    .rodata ALIGN(4K) :
    {
        *(.rodata .rodata.*)  /* Read-only data */
    }

    .data ALIGN(4K) :
    {
        *(.data .data.*)      /* Writable data */
    }

    .bss ALIGN(4K) :
    {
        _bss_start = .;
        *(.bss .bss.*)        /* Zero-initialized data */
        *(COMMON)
        _bss_end = .;
    }

    /DISCARD/ :
    {
        *(.eh_frame)
        *(.note .note.*)
        *(.comment)
    }
}
```

---

## 10. CI/CD (GitHub Actions)

```yaml
# .github/workflows/build.yml
name: Build NeoCore OS

on:
  push:
    branches: [main, develop]
  pull_request:

jobs:
  build-kernel:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Install Rust nightly
        uses: dtolnay/rust-toolchain@nightly
        with:
          components: rust-src, llvm-tools-preview

      - name: Install cargo-bootimage
        run: cargo install cargo-bootimage

      - name: Build Kernel
        run: cd kernel && cargo +nightly bootimage --release

      - name: Test Kernel
        run: |
          sudo apt-get install -y qemu-system-x86
          cd kernel && cargo +nightly test

  build-netmgr:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Go
        uses: actions/setup-go@v4
        with:
          go-version: '1.22'

      - name: Build neonetd
        run: |
          cd netmgr
          GOOS=linux GOARCH=amd64 go build -o bin/neonetd ./cmd/neonetd

      - name: Test netmgr
        run: cd netmgr && go test ./...
```

---

## 11. Referensi

- [cargo-bootimage](https://github.com/rust-osdev/bootimage)
- [QEMU Documentation](https://www.qemu.org/docs/master/)
- [rust-osdev/bootloader](https://github.com/rust-osdev/bootloader)
- [Writing an OS in Rust — Setup](https://os.phil-opp.com/minimal-rust-kernel/)
