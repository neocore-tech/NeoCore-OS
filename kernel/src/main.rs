// kernel/src/main.rs — NeoCore OS Kernel Entry Point
// SPDX-License-Identifier: MIT

#![no_std]
#![no_main]
#![feature(abi_x86_interrupt)]
#![feature(custom_test_frameworks)]
#![feature(alloc_error_handler)]
#![test_runner(crate::test_runner)]
#![reexport_test_harness_main = "test_main"]

extern crate alloc;

use bootloader_api::{entry_point, BootInfo};
use x86_64::VirtAddr;

// === Submodules ===
pub mod drivers;
pub mod fs;
pub mod interrupts;
pub mod memory;
pub mod net;
pub mod panic;
pub mod serial;
pub mod shell;
pub mod syscall;
pub mod task;

// === Kernel Entry Point ===
pub static BOOTLOADER_CONFIG: bootloader_api::BootloaderConfig = {
    let mut config = bootloader_api::BootloaderConfig::new_default();
    config.mappings.physical_memory = Some(bootloader_api::config::Mapping::Dynamic);
    config
};
entry_point!(kernel_main, config = &BOOTLOADER_CONFIG);

fn kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // ── 1. SERIAL (debug output via COM1) ──────────────────────────────
    serial::init();
    serial_println!("[NEOCORE] Serial initialized (COM1)");

    // ── 2. VGA TEXT MODE / FRAMEBUFFER ─────────────────────────────
    if let Some(fb) = boot_info.framebuffer.as_mut() {
        let mut display = drivers::framebuffer::Display::new(fb);
        display.clear_screen();
        *drivers::framebuffer::DISPLAY.lock() = Some(display);
    }
    drivers::vga::init(); // Hanya untuk shell color compatibility dummy
    print_banner();

    // ── 3. GDT (Global Descriptor Table) ──────────────────────────────
    interrupts::gdt::init();
    println!("neocore: GDT initialized");
    serial_println!("neocore: GDT initialized");

    // ── 4. IDT (Interrupt Descriptor Table) ───────────────────────────
    interrupts::idt::init();
    println!("neocore: IDT initialized");

    // ── 5. PIC 8259 ────────────────────────────────────────────────────
    unsafe { interrupts::pic::PICS.lock().initialize() };
    x86_64::instructions::interrupts::enable();
    println!("cpu0: interrupts enabled");

    // ── 6. PIT Timer (1000 Hz) ─────────────────────────────────────────
    interrupts::pit::init();
    println!("timer0: PIT initialized (1000 Hz)");

    // ── 7. MEMORY MANAGER ──────────────────────────────────────────────
    let phys_mem_offset = VirtAddr::new(
        boot_info.physical_memory_offset.into_option()
            .unwrap_or(0x10000000000) // Fallback ke default mapping bootloader v0.11
    );
    let mut mapper = unsafe { memory::paging::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        memory::frame_allocator::BootInfoFrameAllocator::init(
            &boot_info.memory_regions
        )
    };
    memory::heap::init_heap(&mut mapper, &mut frame_allocator)
        .expect("Heap initialization failed");
    println!("mem0: memory manager initialized");
    memory::print_info(boot_info);

    // ── 8. KEYBOARD ────────────────────────────────────────────────────
    drivers::keyboard::init();
    println!("kbd0: PS/2 keyboard driver attached");

    // ── 8.5 PCI BUS ENUMERATION ─────────────────────────────────────────
    drivers::pci::init();
    println!("pci0: <PCI bus> enumerated");

    // ── 8.6 ATA DRIVER ──────────────────────────────────────────────────
    drivers::ata::init();
    println!("ata0: <ATA PIO> disk driver initialized");

    // ── 8.7 FILESYSTEM ──────────────────────────────────────────────────
    fs::init();
    println!("vfs0: FAT32 root filesystem mounted");

    // ── 8.8 NETWORK ─────────────────────────────────────────────────────
    net::init();

    // ── 9. SCHEDULER ───────────────────────────────────────────────────
    task::scheduler::init();
    println!("sched: cooperative scheduler initialized");

    // ── DONE ───────────────────────────────────────────────────────────
    println!("");
    println!("  NeoCore OS booted successfully!");
    println!("---------------------------------------------------");
    serial_println!("[NEOCORE] Boot complete - starting shell");

    // ── 10. LAUNCH SHELL (via Cooperative Executor) ────────────────────
    let mut executor = task::executor::Executor::new();

    // Task utama: interactive shell
    executor.spawn(task::Task::new("ncsh", shell::run()));

    // Jalankan executor (blocking — mengelola semua tasks)
    executor.run();

    // Tidak seharusnya sampai sini
    panic!("Executor returned unexpectedly!");
}

/// Cetak banner startup NeoCore OS
fn print_banner() {
    use drivers::vga::{set_color, Color};

    set_color(Color::LightCyan, Color::Black);
    println!("---------------------------------------------------");
    set_color(Color::Yellow, Color::Black);
    println!("  NeoCore OS  v0.1.0  |  Rust Kernel + Go Network  ");
    println!("  x86_64  |  Built with Rust nightly               ");
    set_color(Color::LightCyan, Color::Black);
    println!("---------------------------------------------------");
    set_color(Color::White, Color::Black);
    println!("");
}

// ── TEST RUNNER (QEMU) ─────────────────────────────────────────────────
pub trait Testable {
    fn run(&self);
}

impl<T: Fn()> Testable for T {
    fn run(&self) {
        serial_print!("  {} ...\t", core::any::type_name::<T>());
        self();
        serial_println!("[ok]");
    }
}

pub fn test_runner(tests: &[&dyn Testable]) {
    serial_println!("Running {} tests", tests.len());
    for test in tests {
        test.run();
    }
    exit_qemu(QemuExitCode::Success);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum QemuExitCode {
    Success = 0x10,
    Failed  = 0x11,
}

pub fn exit_qemu(exit_code: QemuExitCode) {
    use x86_64::instructions::port::Port;
    unsafe {
        let mut port = Port::new(0xf4);
        port.write(exit_code as u32);
    }
}

#[cfg(test)]
#[no_mangle]
pub extern "C" fn _start(boot_info: &'static mut BootInfo) -> ! {
    interrupts::gdt::init();
    interrupts::idt::init();
    unsafe { interrupts::pic::PICS.lock().initialize() };
    interrupts::pit::init();

    let phys_mem_offset = VirtAddr::new(
        boot_info.physical_memory_offset.into_option().unwrap()
    );
    let mut mapper = unsafe { memory::paging::init(phys_mem_offset) };
    let mut frame_allocator = unsafe {
        memory::frame_allocator::BootInfoFrameAllocator::init(
            &boot_info.memory_regions
        )
    };
    memory::heap::init_heap(&mut mapper, &mut frame_allocator).unwrap();

    test_main();
    loop { x86_64::instructions::hlt(); }
}
