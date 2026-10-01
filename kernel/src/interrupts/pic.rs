// kernel/src/interrupts/pic.rs — PIC 8259 Controller

use pic8259::ChainedPics;
use spinning_top::Spinlock;

/// Remap IRQ 0-15 ke interrupt 32-47
/// (menghindari tabrakan dengan CPU exceptions 0-31)
pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static PICS: Spinlock<ChainedPics> = Spinlock::new(unsafe {
    ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET)
});

/// Nomor interrupt (setelah remap)
#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum IrqIndex {
    Timer    = PIC_1_OFFSET,       // IRQ 0  → INT 32
    Keyboard = PIC_1_OFFSET + 1,   // IRQ 1  → INT 33
    Serial2  = PIC_1_OFFSET + 3,   // IRQ 3  → INT 35
    Serial1  = PIC_1_OFFSET + 4,   // IRQ 4  → INT 36
    Mouse    = PIC_1_OFFSET + 12,  // IRQ 12 → INT 44
    IdeAta0  = PIC_1_OFFSET + 14,  // IRQ 14 → INT 46
    IdeAta1  = PIC_1_OFFSET + 15,  // IRQ 15 → INT 47
}

/// Kirim EOI (End of Interrupt) ke PIC
pub fn send_eoi(irq: IrqIndex) {
    unsafe {
        PICS.lock().notify_end_of_interrupt(irq as u8);
    }
}
