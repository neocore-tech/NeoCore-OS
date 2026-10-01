# ⚙️ KERNEL — Core Kernel NeoCore OS (Rust)

---

## 1. Overview

Kernel adalah inti dari OS — jembatan antara hardware dan software. NeoCore OS menggunakan pendekatan **monolithic kernel** (seperti Linux) namun dengan network management di user space.

**Environment:**
- `#![no_std]` — tidak ada standard library Rust
- `#![no_main]` — tidak ada C runtime, entry point kustom
- Target: `x86_64-unknown-none`

---

## 2. Struktur File Kernel

```
kernel/
├── Cargo.toml
├── Cargo.lock
├── .cargo/
│   └── config.toml          # Build config (target, rustflags)
├── src/
│   ├── main.rs              # Entry point kernel
│   ├── lib.rs               # Library (testing support)
│   ├── interrupts/
│   │   ├── mod.rs           # Interrupt module
│   │   ├── gdt.rs           # Global Descriptor Table
│   │   ├── idt.rs           # Interrupt Descriptor Table
│   │   ├── handlers.rs      # Interrupt service routines
│   │   └── pic.rs           # PIC 8259 controller
│   ├── memory/
│   │   ├── mod.rs
│   │   ├── frame_allocator.rs  # Physical frame allocator
│   │   ├── paging.rs           # Page table management
│   │   └── heap.rs             # Kernel heap allocator
│   ├── task/
│   │   ├── mod.rs
│   │   ├── scheduler.rs     # Process scheduler
│   │   ├── process.rs       # Process control block
│   │   └── context.rs       # Context switching
│   ├── syscall/
│   │   ├── mod.rs
│   │   └── table.rs         # Syscall dispatch table
│   ├── drivers/
│   │   ├── vga.rs           # VGA text mode
│   │   ├── serial.rs        # UART serial port
│   │   ├── keyboard.rs      # PS/2 keyboard
│   │   └── net/
│   │       ├── mod.rs
│   │       ├── virtio.rs    # VirtIO-net driver
│   │       └── e1000.rs     # Intel e1000 driver
│   ├── net/
│   │   ├── mod.rs
│   │   ├── stack.rs         # smoltcp integration
│   │   ├── socket.rs        # Socket layer
│   │   └── netlink.rs       # IPC dengan Go network manager
│   └── panic.rs             # Panic handler
├── tests/
│   └── integration_tests.rs
└── linker.ld
```

---

## 3. Entry Point Kernel

```rust
// src/main.rs
#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader_api::{entry_point, BootInfo};
use x86_64::VirtAddr;

use crate::memory::BootInfoFrameAllocator;

mod interrupts;
mod memory;
mod task;
mod syscall;
mod drivers;
mod net;
mod panic;

entry_point!(kernel_main);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // 1. Inisialisasi VGA untuk output teks
    drivers::vga::init();
    println!("🦀 NeoCore OS v0.1.0 — Starting...");

    // 2. Inisialisasi GDT (Global Descriptor Table)
    interrupts::gdt::init();
    println!("[OK] GDT initialized");

    // 3. Inisialisasi IDT (Interrupt Descriptor Table)
    interrupts::idt::init();
    println!("[OK] IDT initialized");

    // 4. Inisialisasi PIC (Programmable Interrupt Controller)
    unsafe { interrupts::pic::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();
    println!("[OK] Interrupts enabled");

    // 5. Inisialisasi Memory Manager
    let phys_mem_offset = VirtAddr::new(
        boot_info.physical_memory_offset.into_option().unwrap()
    );
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        BootInfoFrameAllocator::init(&boot_info.memory_regions)
    };
    memory::heap::init_heap(&mut mapper, &mut frame_allocator)
        .expect("Heap initialization failed");
    println!("[OK] Memory manager initialized");

    // 6. Inisialisasi Network Stack
    net::init();
    println!("[OK] Network stack initialized");

    // 7. Inisialisasi Scheduler
    task::scheduler::init();
    println!("[OK] Scheduler initialized");

    // 8. Spawn proses awal
    task::spawn(drivers::keyboard::handle_input);
    task::spawn(net::netlink::ipc_handler);     // IPC untuk Go network manager
    task::spawn(shell::run);                     // Shell

    println!("✅ NeoCore OS Boot Complete!");
    println!("   Type 'help' for available commands\n");

    // 9. Masuk ke main event loop (scheduler mengambil alih)
    task::scheduler::run();
}
```

---

## 4. Cargo.toml

```toml
[package]
name = "kernel"
version = "0.1.0"
edition = "2021"

[[bin]]
name = "kernel"
test = false

[dependencies]
# Bootloader API
bootloader_api = "0.11"

# x86_64 hardware abstraction
x86_64 = "0.15"

# Interrupt management
pic8259 = "0.11"

# Serial port
uart_16550 = "0.3"

# PS/2 Keyboard
pc-keyboard = "0.7"

# TCP/IP stack
smoltcp = { version = "0.11", default-features = false,
            features = [
                "socket-tcp",
                "socket-udp",
                "socket-icmp",
                "socket-raw",
                "proto-ipv4",
                "proto-ipv6",
                "proto-dhcpv4",
            ]}

# Heap allocator
linked_list_allocator = "0.10"

# Spinlock (no_std compatible)
spinning_top = "0.3"

# Lazy static (no_std)
lazy_static = { version = "1.4", features = ["spin_no_std"] }

# Bitflags
bitflags = "2.4"

# Volatile memory access
volatile = "0.4"

[dependencies.bootloader]
version = "0.11"
features = ["map_physical_memory"]

[profile.release]
panic = "abort"
opt-level = "s"   # Optimasi ukuran

[profile.dev]
panic = "abort"

[package.metadata.bootimage]
test-args = ["-device", "isa-debug-exit,iobase=0xf4,iosize=0x04",
             "-serial", "stdio",
             "-display", "none"]
test-success-exit-code = 33
```

### `.cargo/config.toml`

```toml
[unstable]
build-std = ["core", "compiler_builtins", "alloc"]
build-std-features = ["compiler-builtins-mem"]

[build]
target = "x86_64-unknown-none.json"

[target.'cfg(target_os = "none")']
runner = "bootimage runner"

rustflags = [
    "-C", "link-arg=-Tlinker.ld",
]
```

---

## 5. GDT (Global Descriptor Table)

```rust
// src/interrupts/gdt.rs
use lazy_static::lazy_static;
use x86_64::structures::gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector};
use x86_64::structures::tss::TaskStateSegment;
use x86_64::VirtAddr;

pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();
        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            const STACK_SIZE: usize = 4096 * 5;
            static mut STACK: [u8; STACK_SIZE] = [0; STACK_SIZE];
            let stack_start = VirtAddr::from_ptr(unsafe { &STACK });
            stack_start + STACK_SIZE as u64
        };
        tss
    };
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();
        let code_selector = gdt.append(Descriptor::kernel_code_segment());
        let data_selector = gdt.append(Descriptor::kernel_data_segment());
        let user_code_selector = gdt.append(Descriptor::user_code_segment());
        let user_data_selector = gdt.append(Descriptor::user_data_segment());
        let tss_selector = gdt.append(Descriptor::tss_segment(&TSS));
        (gdt, Selectors {
            code_selector,
            data_selector,
            user_code_selector,
            user_data_selector,
            tss_selector,
        })
    };
}

pub struct Selectors {
    pub code_selector: SegmentSelector,
    pub data_selector: SegmentSelector,
    pub user_code_selector: SegmentSelector,
    pub user_data_selector: SegmentSelector,
    pub tss_selector: SegmentSelector,
}

pub fn init() {
    use x86_64::instructions::segmentation::{CS, DS, SS, Segment};
    use x86_64::instructions::tables::load_tss;

    GDT.0.load();
    unsafe {
        CS::set_reg(GDT.1.code_selector);
        DS::set_reg(GDT.1.data_selector);
        SS::set_reg(SegmentSelector(0)); // Stack tidak pakai segment
        load_tss(GDT.1.tss_selector);
    }
}
```

---

## 6. IDT & Interrupt Handlers

```rust
// src/interrupts/idt.rs
use lazy_static::lazy_static;
use x86_64::structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode};
use pic8259::ChainedPics;
use spinning_top::Spinlock;

// PIC remapping: IRQ 0-15 → Interrupt 32-47
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: Spinlock<ChainedPics> = Spinlock::new(
    unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) }
);

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer     = PIC_1_OFFSET,      // IRQ 0 → INT 32
    Keyboard  = PIC_1_OFFSET + 1,  // IRQ 1 → INT 33
    Serial2   = PIC_1_OFFSET + 3,  // IRQ 3 → INT 35
    Serial1   = PIC_1_OFFSET + 4,  // IRQ 4 → INT 36
    Mouse     = PIC_1_OFFSET + 12, // IRQ 12 → INT 44
    PrimaryATA= PIC_1_OFFSET + 14, // IRQ 14 → INT 46
}

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        // CPU Exceptions
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(super::gdt::DOUBLE_FAULT_IST_INDEX);
        }
        idt.general_protection_fault.set_handler_fn(general_protection_fault_handler);

        // Hardware Interrupts (IRQ)
        idt[InterruptIndex::Timer as usize].set_handler_fn(timer_interrupt_handler);
        idt[InterruptIndex::Keyboard as usize].set_handler_fn(keyboard_interrupt_handler);

        // Syscall (INT 0x80)
        unsafe {
            idt[0x80]
                .set_handler_fn(syscall_handler)
                .set_privilege_level(x86_64::PrivilegeLevel::Ring3);
        }

        idt
    };
}

pub fn init() {
    IDT.load();
}

// === Interrupt Handlers ===

extern "x86-interrupt" fn breakpoint_handler(stack_frame: InterruptStackFrame) {
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;
    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    panic!("Page fault - cannot continue");
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn general_protection_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: u64,
) {
    panic!("EXCEPTION: GENERAL PROTECTION FAULT\nError: {}\n{:#?}",
           error_code, stack_frame);
}

extern "x86-interrupt" fn timer_interrupt_handler(_stack_frame: InterruptStackFrame) {
    // Trigger scheduler preemption
    crate::task::scheduler::tick();

    // EOI (End of Interrupt)
    unsafe {
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Timer as u8);
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(_stack_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };
    crate::drivers::keyboard::add_scancode(scancode);

    unsafe {
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Keyboard as u8);
    }
}

extern "x86-interrupt" fn syscall_handler(_stack_frame: InterruptStackFrame) {
    crate::syscall::dispatch();
}
```

---

## 7. Scheduler

```rust
// src/task/scheduler.rs
use alloc::collections::VecDeque;
use spinning_top::Spinlock;
use lazy_static::lazy_static;

use super::process::{Process, ProcessId, ProcessState};
use super::context::Context;

lazy_static! {
    pub static ref SCHEDULER: Spinlock<Scheduler> = Spinlock::new(Scheduler::new());
}

pub struct Scheduler {
    ready_queue: VecDeque<Process>,
    current: Option<Process>,
    next_pid: ProcessId,
    tick_count: u64,
}

const TIME_SLICE_TICKS: u64 = 10; // 10ms per proses (jika timer 1ms)

impl Scheduler {
    pub fn new() -> Self {
        Scheduler {
            ready_queue: VecDeque::new(),
            current: None,
            next_pid: ProcessId(1),
            tick_count: 0,
        }
    }

    pub fn spawn(&mut self, entry: fn()) -> ProcessId {
        let pid = self.next_pid;
        self.next_pid.0 += 1;

        let process = Process::new(pid, entry);
        self.ready_queue.push_back(process);
        pid
    }

    pub fn tick(&mut self) {
        self.tick_count += 1;
        if self.tick_count % TIME_SLICE_TICKS == 0 {
            self.schedule();
        }
    }

    fn schedule(&mut self) {
        // Round-Robin scheduling
        if let Some(mut current) = self.current.take() {
            if current.state == ProcessState::Running {
                current.state = ProcessState::Ready;
                self.ready_queue.push_back(current);
            }
        }

        if let Some(mut next) = self.ready_queue.pop_front() {
            next.state = ProcessState::Running;
            self.current = Some(next);
            // Context switch ke proses berikutnya
        }
    }
}

pub fn init() {
    // Scheduler sudah diinisialisasi via lazy_static
    // Setup timer interrupt (PIT: 1000 Hz = 1ms per tick)
    setup_pit_timer(1000);
}

pub fn tick() {
    SCHEDULER.lock().tick();
}

pub fn spawn(entry: fn()) -> ProcessId {
    SCHEDULER.lock().spawn(entry)
}

fn setup_pit_timer(frequency_hz: u32) {
    use x86_64::instructions::port::Port;
    let divisor = 1_193_180u32 / frequency_hz;
    let mut command_port: Port<u8> = Port::new(0x43);
    let mut data_port: Port<u8> = Port::new(0x40);
    unsafe {
        command_port.write(0x36); // Channel 0, lobyte/hibyte, square wave
        data_port.write((divisor & 0xFF) as u8);
        data_port.write((divisor >> 8) as u8);
    }
}
```

---

## 8. Syscall Interface

```rust
// src/syscall/table.rs
use x86_64::registers::rflags::RFlags;

// Syscall numbers
pub const SYS_READ:      u64 = 0;
pub const SYS_WRITE:     u64 = 1;
pub const SYS_OPEN:      u64 = 2;
pub const SYS_CLOSE:     u64 = 3;
pub const SYS_FORK:      u64 = 10;
pub const SYS_EXEC:      u64 = 11;
pub const SYS_SOCKET:    u64 = 20;
pub const SYS_BIND:      u64 = 21;
pub const SYS_CONNECT:   u64 = 22;
pub const SYS_SEND:      u64 = 23;
pub const SYS_RECV:      u64 = 24;
pub const SYS_NETCONFIG: u64 = 30; // Custom: konfigurasi network
pub const SYS_NETROUTE:  u64 = 31; // Custom: routing table
pub const SYS_NETFILTER: u64 = 32; // Custom: firewall rules

pub fn dispatch() {
    // Register RAX berisi syscall number
    // RDI, RSI, RDX, R10, R8, R9 = argumen
    let syscall_num: u64;
    let arg1: u64;
    let arg2: u64;
    let arg3: u64;

    unsafe {
        core::arch::asm!(
            "mov {}, rax",
            "mov {}, rdi",
            "mov {}, rsi",
            "mov {}, rdx",
            out(reg) syscall_num,
            out(reg) arg1,
            out(reg) arg2,
            out(reg) arg3,
        );
    }

    let result = match syscall_num {
        SYS_WRITE     => sys_write(arg1, arg2 as *const u8, arg3 as usize),
        SYS_READ      => sys_read(arg1, arg2 as *mut u8, arg3 as usize),
        SYS_SOCKET    => sys_socket(arg1, arg2, arg3),
        SYS_NETCONFIG => sys_netconfig(arg1 as *const u8, arg2 as usize),
        SYS_NETROUTE  => sys_netroute(arg1 as *const u8, arg2 as usize),
        _ => u64::MAX, // ENOSYS
    };

    // Kembalikan result di RAX
    unsafe {
        core::arch::asm!("mov rax, {}", in(reg) result);
    }
}

fn sys_write(fd: u64, buf: *const u8, len: usize) -> u64 {
    let slice = unsafe { core::slice::from_raw_parts(buf, len) };
    match fd {
        1 => { // stdout
            if let Ok(s) = core::str::from_utf8(slice) {
                print!("{}", s);
            }
            len as u64
        }
        _ => u64::MAX,
    }
}

fn sys_read(fd: u64, buf: *mut u8, len: usize) -> u64 {
    // TODO: Implement per fd type
    0
}

fn sys_socket(domain: u64, sock_type: u64, protocol: u64) -> u64 {
    crate::net::socket::create(domain, sock_type, protocol)
}

fn sys_netconfig(config_ptr: *const u8, config_len: usize) -> u64 {
    // Digunakan oleh Go network manager untuk konfigurasi interface
    let config = unsafe { core::slice::from_raw_parts(config_ptr, config_len) };
    crate::net::netlink::apply_config(config)
}

fn sys_netroute(route_ptr: *const u8, route_len: usize) -> u64 {
    let route = unsafe { core::slice::from_raw_parts(route_ptr, route_len) };
    crate::net::netlink::apply_route(route)
}
```

---

## 9. Panic Handler

```rust
// src/panic.rs
use core::panic::PanicInfo;

#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use crate::drivers::vga::Color;

    // Set warna merah untuk pesan panic
    crate::drivers::vga::set_color(Color::Red, Color::Black);
    println!("\n━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  💀 KERNEL PANIC — System Halted");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    if let Some(location) = info.location() {
        println!("  Location: {}:{}:{}", 
                 location.file(), 
                 location.line(), 
                 location.column());
    }
    if let Some(msg) = info.message() {
        println!("  Message: {}", msg);
    }
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Halt CPU
    loop {
        x86_64::instructions::hlt();
    }
}
```

---

## 10. VGA Driver

```rust
// src/drivers/vga.rs
use core::fmt;
use lazy_static::lazy_static;
use spinning_top::Spinlock;
use volatile::Volatile;

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH:  usize = 80;
const VGA_BUFFER:    usize = 0xb8000;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black       = 0,
    Blue        = 1,
    Green       = 2,
    Cyan        = 3,
    Red         = 4,
    Magenta     = 5,
    Brown       = 6,
    LightGray   = 7,
    DarkGray    = 8,
    LightBlue   = 9,
    LightGreen  = 10,
    LightCyan   = 11,
    LightRed    = 12,
    Pink        = 13,
    Yellow      = 14,
    White       = 15,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    fn new(fg: Color, bg: Color) -> ColorCode {
        ColorCode((bg as u8) << 4 | (fg as u8))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii: u8,
    color_code: ColorCode,
}

#[repr(transparent)]
struct Buffer {
    chars: [[Volatile<ScreenChar>; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct Writer {
    col_pos: usize,
    color: ColorCode,
    buffer: &'static mut Buffer,
}

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            byte => {
                if self.col_pos >= BUFFER_WIDTH {
                    self.new_line();
                }
                let row = BUFFER_HEIGHT - 1;
                let col = self.col_pos;
                let color_code = self.color;
                self.buffer.chars[row][col].write(ScreenChar {
                    ascii: byte,
                    color_code,
                });
                self.col_pos += 1;
            }
        }
    }

    fn new_line(&mut self) {
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let ch = self.buffer.chars[row][col].read();
                self.buffer.chars[row - 1][col].write(ch);
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.col_pos = 0;
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii: b' ',
            color_code: self.color,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col].write(blank);
        }
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for byte in s.bytes() {
            match byte {
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                _ => self.write_byte(0xfe), // Unknown char
            }
        }
        Ok(())
    }
}

lazy_static! {
    pub static ref WRITER: Spinlock<Writer> = Spinlock::new(Writer {
        col_pos: 0,
        color: ColorCode::new(Color::White, Color::Black),
        buffer: unsafe { &mut *(VGA_BUFFER as *mut Buffer) },
    });
}

pub fn init() {
    // Clear screen
    WRITER.lock().clear_row(BUFFER_HEIGHT - 1);
}

pub fn set_color(fg: Color, bg: Color) {
    WRITER.lock().color = ColorCode::new(fg, bg);
}

// Macro untuk print
#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::drivers::vga::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        WRITER.lock().write_fmt(args).unwrap();
    });
}
```

---

## 11. Referensi

- [Writing an OS in Rust - phil-opp](https://os.phil-opp.com/)
- [x86_64 crate docs](https://docs.rs/x86_64)
- [OSDev GDT Tutorial](https://wiki.osdev.org/GDT_Tutorial)
- [OSDev IDT](https://wiki.osdev.org/Interrupt_Descriptor_Table)
