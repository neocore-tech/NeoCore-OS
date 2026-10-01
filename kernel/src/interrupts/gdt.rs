// kernel/src/interrupts/gdt.rs — Global Descriptor Table

use lazy_static::lazy_static;
use x86_64::{
    structures::{
        gdt::{Descriptor, GlobalDescriptorTable, SegmentSelector},
        tss::TaskStateSegment,
    },
    VirtAddr,
};

/// Stack index untuk Double Fault IST
pub const DOUBLE_FAULT_IST_INDEX: u16 = 0;

/// Stack khusus untuk double fault handler (mencegah triple fault)
static mut DOUBLE_FAULT_STACK: [u8; STACK_SIZE] = [0u8; STACK_SIZE];
const STACK_SIZE: usize = 4096 * 5; // 20KB

lazy_static! {
    static ref TSS: TaskStateSegment = {
        let mut tss = TaskStateSegment::new();

        tss.interrupt_stack_table[DOUBLE_FAULT_IST_INDEX as usize] = {
            let stack_start = VirtAddr::from_ptr(unsafe { &DOUBLE_FAULT_STACK });
            stack_start + STACK_SIZE as u64 // Stack tumbuh ke bawah
        };

        tss
    };
}

/// Selector register untuk GDT entries
pub struct Selectors {
    pub kernel_code: SegmentSelector,
    pub kernel_data: SegmentSelector,
    pub user_code:   SegmentSelector,
    pub user_data:   SegmentSelector,
    pub tss:         SegmentSelector,
}

lazy_static! {
    static ref GDT: (GlobalDescriptorTable, Selectors) = {
        let mut gdt = GlobalDescriptorTable::new();

        // Ring 0 (Kernel)
        let kernel_code = gdt.append(Descriptor::kernel_code_segment());
        let kernel_data = gdt.append(Descriptor::kernel_data_segment());

        // Ring 3 (User — akan dipakai nanti untuk user space)
        let user_data = gdt.append(Descriptor::user_data_segment());
        let user_code = gdt.append(Descriptor::user_code_segment());

        // TSS (diperlukan untuk IST)
        let tss = gdt.append(Descriptor::tss_segment(&TSS));

        (gdt, Selectors { kernel_code, kernel_data, user_code, user_data, tss })
    };
}

/// Inisialisasi dan load GDT
pub fn init() {
    use x86_64::instructions::{
        segmentation::{CS, DS, Segment},
        tables::load_tss,
    };

    GDT.0.load();

    unsafe {
        // Set segment registers
        CS::set_reg(GDT.1.kernel_code);
        DS::set_reg(GDT.1.kernel_data);

        // Load TSS
        load_tss(GDT.1.tss);
    }
}

/// Accessor untuk segment selectors (dipakai IDT dll)
pub fn kernel_code_selector() -> SegmentSelector {
    GDT.1.kernel_code
}

// ── Tests ────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    #[test_case]
    fn test_gdt_loads() {
        // GDT harus sudah diload di setup sebelum test ini
        // Test ini memverifikasi bahwa kita tidak crash
        crate::serial_println!("GDT: OK");
    }
}
