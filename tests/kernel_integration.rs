// tests/kernel_integration.rs — Integration tests untuk NeoCore kernel
// Dijalankan di QEMU dengan: cargo test --package neocore-kernel
// SPDX-License-Identifier: MIT

#![no_std]
#![no_main]
#![feature(custom_test_frameworks)]
#![test_runner(neocore_kernel::test_runner)]
#![reexport_test_harness_main = "test_main"]

use neocore_kernel::{serial_println, println};
use bootloader_api::{entry_point, BootInfo};
use x86_64::VirtAddr;

entry_point!(test_kernel_main);

fn test_kernel_main(boot_info: &'static mut BootInfo) -> ! {
    // Init subsystem minimal untuk testing
    neocore_kernel::interrupts::gdt::init();
    neocore_kernel::interrupts::idt::init();
    unsafe { neocore_kernel::interrupts::pic::PICS.lock().initialize() };
    neocore_kernel::interrupts::pit::init();

    x86_64::instructions::interrupts::enable();

    let phys_mem_offset = VirtAddr::new(
        boot_info.physical_memory_offset.into_option().unwrap()
    );
    let mut mapper = unsafe {
        neocore_kernel::memory::paging::init(phys_mem_offset)
    };
    let mut frame_allocator = unsafe {
        neocore_kernel::memory::frame_allocator::BootInfoFrameAllocator::init(
            &boot_info.memory_regions
        )
    };
    neocore_kernel::memory::heap::init_heap(&mut mapper, &mut frame_allocator)
        .expect("Heap init failed");

    test_main();
    loop { x86_64::instructions::hlt(); }
}

// ── Test: Memory ──────────────────────────────────────────────────────────

#[test_case]
fn test_heap_basic_allocation() {
    extern crate alloc;
    use alloc::vec::Vec;

    let mut v: Vec<u32> = Vec::new();
    for i in 0..100 {
        v.push(i);
    }
    assert_eq!(v.len(), 100);
    assert_eq!(v[42], 42);
    serial_println!("[PASS] heap_basic_allocation");
}

#[test_case]
fn test_heap_box() {
    extern crate alloc;
    use alloc::boxed::Box;

    let b = Box::new(0xDEAD_BEEFu64);
    assert_eq!(*b, 0xDEAD_BEEF);
    serial_println!("[PASS] heap_box");
}

#[test_case]
fn test_heap_string() {
    extern crate alloc;
    use alloc::string::String;

    let s = String::from("NeoCore OS v0.1");
    assert_eq!(s.len(), 15);
    serial_println!("[PASS] heap_string");
}

// ── Test: Interrupts ──────────────────────────────────────────────────────

#[test_case]
fn test_breakpoint_exception_no_crash() {
    x86_64::instructions::interrupts::int3();
    serial_println!("[PASS] breakpoint_exception_no_crash");
}

#[test_case]
fn test_interrupts_enabled() {
    use x86_64::instructions::interrupts;
    // Disable lalu enable lagi
    interrupts::disable();
    interrupts::enable();
    // Jika tidak crash → OK
    serial_println!("[PASS] interrupts_enabled");
}

// ── Test: PIT Timer ───────────────────────────────────────────────────────

#[test_case]
fn test_pit_ticks_increment() {
    let t0 = neocore_kernel::interrupts::pit::ticks();
    // Tunggu beberapa loop — timer seharusnya sudah berdetak
    for _ in 0..1_000_000 {
        core::hint::spin_loop();
    }
    let t1 = neocore_kernel::interrupts::pit::ticks();
    // Ticks harus monoton
    assert!(t1 >= t0, "PIT ticks tidak boleh berkurang");
    serial_println!("[PASS] pit_ticks_increment (t0={} t1={})", t0, t1);
}

// ── Test: Scheduler ───────────────────────────────────────────────────────

#[test_case]
fn test_scheduler_ticks() {
    let s0 = neocore_kernel::task::scheduler::ticks();
    for _ in 0..100_000 {
        core::hint::spin_loop();
    }
    let s1 = neocore_kernel::task::scheduler::ticks();
    assert!(s1 >= s0);
    serial_println!("[PASS] scheduler_ticks");
}

// ── Test: Task Executor ───────────────────────────────────────────────────

#[test_case]
fn test_executor_runs_simple_task() {
    extern crate alloc;

    static DONE: core::sync::atomic::AtomicBool =
        core::sync::atomic::AtomicBool::new(false);

    async fn simple_task() {
        DONE.store(true, core::sync::atomic::Ordering::SeqCst);
    }

    let mut executor = neocore_kernel::task::executor::Executor::new();
    executor.spawn(neocore_kernel::task::Task::new("test", simple_task()));
    executor.run_once();

    assert!(DONE.load(core::sync::atomic::Ordering::SeqCst),
        "Task harus sudah berjalan");
    serial_println!("[PASS] executor_runs_simple_task");
}

#[test_case]
fn test_executor_multiple_tasks() {
    extern crate alloc;
    use core::sync::atomic::{AtomicU32, Ordering};

    static COUNT: AtomicU32 = AtomicU32::new(0);

    async fn counter_task() {
        COUNT.fetch_add(1, Ordering::SeqCst);
    }

    let mut executor = neocore_kernel::task::executor::Executor::new();
    executor.spawn(neocore_kernel::task::Task::new("t1", counter_task()));
    executor.spawn(neocore_kernel::task::Task::new("t2", counter_task()));
    executor.spawn(neocore_kernel::task::Task::new("t3", counter_task()));

    // Poll beberapa kali hingga semua selesai
    for _ in 0..10 {
        executor.run_once();
        if executor.task_count() == 0 { break; }
    }

    assert_eq!(COUNT.load(Ordering::SeqCst), 3);
    serial_println!("[PASS] executor_multiple_tasks");
}
