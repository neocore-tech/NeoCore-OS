// kernel/src/panic.rs — Kernel Panic Handler

use crate::{println, serial_println};
use core::panic::PanicInfo;


#[panic_handler]
fn panic(info: &PanicInfo) -> ! {
    use crate::drivers::vga::{set_color, Color};

    // Ubah warna terminal ke merah
    set_color(Color::LightRed, Color::Black);

    println!("");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
    println!("  KERNEL PANIC — NeoCore OS has stopped             ");
    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    if let Some(location) = info.location() {
        println!("  File   : {}", location.file());
        println!("  Line   : {}", location.line());
    }

    println!("  Message: {}", info.message());

    println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");

    // Juga kirim ke serial untuk debugging
    serial_println!("[PANIC] {}", info);

    // CPU halt
    loop {
        x86_64::instructions::hlt();
    }
}

/// Panic handler untuk test environment (QEMU)
#[cfg(test)]
pub fn test_panic_handler(info: &PanicInfo) -> ! {
    serial_println!("[FAILED]\nError: {}\n", info);
    crate::exit_qemu(crate::QemuExitCode::Failed);
    loop {}
}
