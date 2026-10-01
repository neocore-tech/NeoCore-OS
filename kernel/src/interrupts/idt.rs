// kernel/src/interrupts/idt.rs — Interrupt Descriptor Table

use crate::{serial_println};
use lazy_static::lazy_static;
use x86_64::structures::idt::{
    InterruptDescriptorTable,
    InterruptStackFrame,
    PageFaultErrorCode,
};

use super::gdt::DOUBLE_FAULT_IST_INDEX;
use super::pic::{IrqIndex, send_eoi};

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();

        // ── CPU Exceptions (0-31) ──────────────────────────────────────

        idt.divide_error.set_handler_fn(divide_error_handler);
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        idt.overflow.set_handler_fn(overflow_handler);
        idt.invalid_opcode.set_handler_fn(invalid_opcode_handler);
        idt.general_protection_fault.set_handler_fn(gpf_handler);
        idt.page_fault.set_handler_fn(page_fault_handler);
        idt.stack_segment_fault.set_handler_fn(stack_segment_fault_handler);

        // Double fault pakai IST stack khusus (mencegah triple fault)
        unsafe {
            idt.double_fault
                .set_handler_fn(double_fault_handler)
                .set_stack_index(DOUBLE_FAULT_IST_INDEX);
        }

        // ── Hardware Interrupts (IRQ 0-15 → INT 32-47) ────────────────

        // Timer (IRQ 0)
        idt[IrqIndex::Timer as u8]
            .set_handler_fn(timer_handler);

        // Keyboard (IRQ 1)
        idt[IrqIndex::Keyboard as u8]
            .set_handler_fn(keyboard_handler);

        // ── Syscall (INT 0x80) ─────────────────────────────────────────
        // Akan diaktifkan saat user space siap
        // unsafe {
        //     idt[0x80]
        //         .set_handler_fn(syscall_handler)
        //         .set_privilege_level(x86_64::PrivilegeLevel::Ring3);
        // }

        idt
    };
}

/// Load IDT ke CPU
pub fn init() {
    IDT.load();
}

// ── Exception Handlers ────────────────────────────────────────────────────

extern "x86-interrupt" fn divide_error_handler(frame: InterruptStackFrame) {
    panic!("EXCEPTION: DIVIDE ERROR\n{:#?}", frame);
}

extern "x86-interrupt" fn breakpoint_handler(frame: InterruptStackFrame) {
    serial_println!("[INT] Breakpoint at {:#x}",
        frame.instruction_pointer.as_u64());
    // Tidak panic — hanya log dan lanjutkan
}

extern "x86-interrupt" fn overflow_handler(frame: InterruptStackFrame) {
    panic!("EXCEPTION: OVERFLOW\n{:#?}", frame);
}

extern "x86-interrupt" fn invalid_opcode_handler(frame: InterruptStackFrame) {
    panic!("EXCEPTION: INVALID OPCODE at {:#x}\n{:#?}",
        frame.instruction_pointer.as_u64(), frame);
}

extern "x86-interrupt" fn gpf_handler(
    frame: InterruptStackFrame,
    error_code: u64,
) {
    panic!("EXCEPTION: GENERAL PROTECTION FAULT\nError code: {:#x}\n{:#?}",
        error_code, frame);
}

extern "x86-interrupt" fn stack_segment_fault_handler(
    frame: InterruptStackFrame,
    error_code: u64,
) {
    panic!("EXCEPTION: STACK-SEGMENT FAULT\nError code: {:#x}\n{:#?}",
        error_code, frame);
}

extern "x86-interrupt" fn page_fault_handler(
    frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    // Cr2 berisi alamat yang menyebabkan page fault
    let fault_addr = Cr2::read_raw();

    serial_println!("[PAGE FAULT] addr={:#x} error={:?}",
        fault_addr, error_code);

    panic!(
        "EXCEPTION: PAGE FAULT\n  Address: {:#x}\n  Error: {:?}\n{:#?}",
        fault_addr, error_code, frame
    );
}

extern "x86-interrupt" fn double_fault_handler(
    frame: InterruptStackFrame,
    _error_code: u64,
) -> ! {
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", frame);
}

// ── Hardware Interrupt Handlers ───────────────────────────────────────────

/// Timer IRQ 0 — dipanggil setiap tick (1 ms)
extern "x86-interrupt" fn timer_handler(_frame: InterruptStackFrame) {
    // Increment tick counter global (PIT)
    crate::interrupts::pit::tick();

    // Trigger scheduler — nanti preemptive, sekarang hanya counter
    crate::task::scheduler::tick();

    send_eoi(IrqIndex::Timer);
}

/// Keyboard IRQ 1 — baca scancode dan masukkan ke queue
extern "x86-interrupt" fn keyboard_handler(_frame: InterruptStackFrame) {
    use x86_64::instructions::port::Port;

    let mut port: Port<u8> = Port::new(0x60); // Data port PS/2
    let scancode: u8 = unsafe { port.read() };

    crate::drivers::keyboard::add_scancode(scancode);

    send_eoi(IrqIndex::Keyboard);
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #[test_case]
    fn test_breakpoint_does_not_crash() {
        // Breakpoint exception harus tidak crash kernel
        x86_64::instructions::interrupts::int3();
        // Jika sampai sini = berhasil
        crate::serial_println!("Breakpoint handler: OK");
    }
}
