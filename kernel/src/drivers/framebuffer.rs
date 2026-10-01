use bootloader_api::info::{FrameBuffer, PixelFormat};
use font8x8::{BASIC_FONTS, UnicodeFonts};
use core::fmt;
use spinning_top::Spinlock;
use lazy_static::lazy_static;

lazy_static! {
    pub static ref DISPLAY: Spinlock<Option<Display>> = Spinlock::new(None);
}

pub struct Display {
    framebuffer: &'static mut [u8],
    info: bootloader_api::info::FrameBufferInfo,
    x_pos: usize,
    y_pos: usize,
    color: [u8; 3], // RGB color
}

impl Display {
    pub fn new(fb: &mut FrameBuffer) -> Self {
        let info = fb.info();
        let buffer = fb.buffer_mut();
        let static_buffer = unsafe {
            core::slice::from_raw_parts_mut(buffer.as_mut_ptr(), buffer.len())
        };
        Self {
            framebuffer: static_buffer,
            info,
            x_pos: 0,
            y_pos: 0,
            color: [255, 255, 255],
        }
    }

    pub fn set_color(&mut self, r: u8, g: u8, b: u8) {
        self.color = [r, g, b];
    }

    fn write_pixel(&mut self, x: usize, y: usize, active: bool) {
        let pixel_offset = y * self.info.stride + x;
        let color = match self.info.pixel_format {
            PixelFormat::Rgb => if active { [self.color[0], self.color[1], self.color[2], 0] } else { [0, 0, 0, 0] },
            PixelFormat::Bgr => if active { [self.color[2], self.color[1], self.color[0], 0] } else { [0, 0, 0, 0] },
            PixelFormat::U8 => if active { [if self.color[0] > 128 { 0xF } else { 0 }, 0, 0, 0] } else { [0, 0, 0, 0] },
            _ => if active { [self.color[0], self.color[1], self.color[2], 0] } else { [0, 0, 0, 0] },
        };

        let bytes_per_pixel = self.info.bytes_per_pixel;
        let byte_offset = pixel_offset * bytes_per_pixel;

        if byte_offset + bytes_per_pixel <= self.framebuffer.len() {
            self.framebuffer[byte_offset..(byte_offset + bytes_per_pixel)]
                .copy_from_slice(&color[..bytes_per_pixel]);
        }
    }

    fn write_char(&mut self, c: char) {
        if c == '\n' {
            self.newline();
            return;
        }

        if self.x_pos >= self.info.width {
            self.newline();
        }

        let bitmap = match BASIC_FONTS.get(c) {
            Some(bitmap) => bitmap,
            None => BASIC_FONTS.get('?').unwrap(),
        };

        for (y, row) in bitmap.iter().enumerate() {
            for x in 0..8 {
                let active = *row & (1 << x) != 0;
                // Skala 2x agar font lebih mudah dibaca (16x16)
                self.write_pixel(self.x_pos + x * 2, self.y_pos + y * 2, active);
                self.write_pixel(self.x_pos + x * 2 + 1, self.y_pos + y * 2, active);
                self.write_pixel(self.x_pos + x * 2, self.y_pos + y * 2 + 1, active);
                self.write_pixel(self.x_pos + x * 2 + 1, self.y_pos + y * 2 + 1, active);
            }
        }
        self.x_pos += 16;
    }

    fn newline(&mut self) {
        self.y_pos += 16;
        self.x_pos = 0;

        if self.y_pos >= self.info.height {
            self.clear_screen();
        }
    }

    pub fn clear_screen(&mut self) {
        self.framebuffer.fill(0);
        self.x_pos = 0;
        self.y_pos = 0;
    }

    pub fn backspace(&mut self) {
        if self.x_pos >= 16 {
            self.x_pos -= 16;
        } else if self.y_pos >= 16 {
            self.y_pos -= 16;
            self.x_pos = (self.info.width / 16) * 16 - 16;
        }
        // Hapus karakter
        for y in 0..16 {
            for x in 0..16 {
                self.write_pixel(self.x_pos + x, self.y_pos + y, false);
            }
        }
    }
}

impl fmt::Write for Display {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        for c in s.chars() {
            if c == '\x08' {
                self.backspace();
            } else {
                self.write_char(c);
            }
        }
        Ok(())
    }
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;

    interrupts::without_interrupts(|| {
        if let Some(display) = DISPLAY.lock().as_mut() {
            display.write_fmt(args).unwrap();
        }
        crate::serial::_print(args); // Selalu mirror ke serial untuk debugging
    });
}
