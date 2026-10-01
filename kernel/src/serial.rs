// kernel/src/serial.rs — UART Serial Driver (COM1)
// Digunakan untuk debug output via serial port

use lazy_static::lazy_static;
use spinning_top::Spinlock;
use uart_16550::SerialPort;

lazy_static! {
    pub static ref SERIAL1: Spinlock<SerialPort> = {
        let mut serial = unsafe { SerialPort::new(0x3F8) }; // COM1
        serial.init();
        Spinlock::new(serial)
    };
}

/// Print ke serial port (tidak lock interrupt)
pub fn _print(args: core::fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        SERIAL1
            .lock()
            .write_fmt(args)
            .expect("Serial write failed");
    });
}

pub fn init() {
    // Trigger lazy_static initialization
    let _ = SERIAL1.lock();
}

// ── Macros ──────────────────────────────────────────────────────────────

#[macro_export]
macro_rules! serial_print {
    ($($arg:tt)*) => {
        $crate::serial::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! serial_println {
    () => ($crate::serial_print!("\n"));
    ($fmt:expr) => ($crate::serial_print!(concat!($fmt, "\n")));
    ($fmt:expr, $($arg:tt)*) => ($crate::serial_print!(
        concat!($fmt, "\n"), $($arg)*
    ));
}
