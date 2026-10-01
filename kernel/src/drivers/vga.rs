// kernel/src/drivers/vga.rs — VGA Text Mode Driver
// 80×25 character display, 16 colors
// SPDX-License-Identifier: MIT

use core::fmt;
use lazy_static::lazy_static;
use spinning_top::Spinlock;

// ── Constants ────────────────────────────────────────────────────────────

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH:  usize = 80;
const VGA_BUFFER:    usize = 0xb8000;

// ── Color Enum ───────────────────────────────────────────────────────────

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
    fn new(fg: Color, bg: Color) -> Self {
        ColorCode((bg as u8) << 4 | (fg as u8))
    }
}

// ── Screen Character ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii:      u8,
    color_code: ColorCode,
}

// ── VGA Buffer (raw pointer access — volatile via ptr) ───────────────────

/// Tulis ScreenChar langsung ke VGA memory dengan volatile write
/// (menghindari compiler optimization yang menghapus write ke MMIO)
#[inline]
unsafe fn vga_write(row: usize, col: usize, ch: ScreenChar) {
    // Dinonaktifkan: QEMU dalam mode Framebuffer (Bootloader v0.11), 
    // menulis ke 0xb8000 akan menyebabkan Page Fault / Triple Fault.
}

/// Baca ScreenChar dari VGA memory dengan volatile read
#[inline]
unsafe fn vga_read(row: usize, col: usize) -> ScreenChar {
    ScreenChar { ascii: b' ', color_code: ColorCode::new(Color::Black, Color::Black) }
}

// ── Writer ───────────────────────────────────────────────────────────────

pub struct Writer {
    col_pos: usize,
    color:   ColorCode,
}

impl Writer {
    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => self.new_line(),
            b'\x08' => {
                // Backspace
                if self.col_pos > 0 {
                    self.col_pos -= 1;
                    unsafe {
                        vga_write(BUFFER_HEIGHT - 1, self.col_pos, ScreenChar {
                            ascii:      b' ',
                            color_code: self.color,
                        });
                    }
                }
            }
            byte => {
                if self.col_pos >= BUFFER_WIDTH {
                    self.new_line();
                }
                unsafe {
                    vga_write(BUFFER_HEIGHT - 1, self.col_pos, ScreenChar {
                        ascii:      byte,
                        color_code: self.color,
                    });
                }
                self.col_pos += 1;
            }
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                0x20..=0x7e | b'\n' | b'\x08' => self.write_byte(byte),
                _ => self.write_byte(0xfe), // ■ untuk char non-ASCII
            }
        }
    }

    fn new_line(&mut self) {
        // Geser semua baris ke atas
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                let ch = unsafe { vga_read(row, col) };
                unsafe { vga_write(row - 1, col, ch) };
            }
        }
        self.clear_row(BUFFER_HEIGHT - 1);
        self.col_pos = 0;
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii:      b' ',
            color_code: self.color,
        };
        for col in 0..BUFFER_WIDTH {
            unsafe { vga_write(row, col, blank) };
        }
    }

    pub fn clear_screen(&mut self) {
        for row in 0..BUFFER_HEIGHT {
            self.clear_row(row);
        }
        self.col_pos = 0;
    }

    pub fn set_color(&mut self, fg: Color, bg: Color) {
        self.color = ColorCode::new(fg, bg);
    }
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

// ── Global Writer ────────────────────────────────────────────────────────

lazy_static! {
    pub static ref WRITER: Spinlock<Writer> = Spinlock::new(Writer {
        col_pos: 0,
        color:   ColorCode::new(Color::White, Color::Black),
    });
}

/// Inisialisasi VGA — clear screen dan set warna default
pub fn init() {
    WRITER.lock().clear_screen();
}

/// Set warna output berikutnya
pub fn set_color(fg: Color, bg: Color) {
    WRITER.lock().set_color(fg, bg);
    
    // Sinkronisasi warna dengan Framebuffer
    let (r, g, b) = match fg {
        Color::Black => (0, 0, 0),
        Color::Blue => (0, 0, 170),
        Color::Green => (0, 170, 0),
        Color::Cyan => (0, 170, 170),
        Color::Red => (170, 0, 0),
        Color::Magenta => (170, 0, 170),
        Color::Brown => (170, 85, 0),
        Color::LightGray => (170, 170, 170),
        Color::DarkGray => (85, 85, 85),
        Color::LightBlue => (85, 85, 255),
        Color::LightGreen => (85, 255, 85),
        Color::LightCyan => (85, 255, 255),
        Color::LightRed => (255, 85, 85),
        Color::Pink => (255, 85, 255),
        Color::Yellow => (255, 255, 85),
        Color::White => (255, 255, 255),
    };

    x86_64::instructions::interrupts::without_interrupts(|| {
        if let Some(display) = crate::drivers::framebuffer::DISPLAY.lock().as_mut() {
            display.set_color(r, g, b);
        }
    });
}

// ── Print Macros ─────────────────────────────────────────────────────────

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => {
        $crate::drivers::framebuffer::_print(format_args!($($arg)*))
    };
}

#[macro_export]
macro_rules! println {
    ()              => ($crate::print!("\n"));
    ($($arg:tt)*)   => ($crate::print!("{}\n", format_args!($($arg)*)));
}

// Macros telah dipindahkan untuk di-route ke serial.
