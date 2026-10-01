// kernel/src/drivers/keyboard.rs — PS/2 Keyboard Driver

use lazy_static::lazy_static;
use pc_keyboard::{
    layouts, DecodedKey, HandleControl, KeyCode, Keyboard, ScancodeSet1,
};
use spinning_top::Spinlock;

// Queue scancode: ISR → consumer (max 128 entry)
const QUEUE_SIZE: usize = 128;

struct ScancodeQueue {
    buf:   [u8; QUEUE_SIZE],
    head:  usize,
    tail:  usize,
    count: usize,
}

impl ScancodeQueue {
    const fn new() -> Self {
        ScancodeQueue {
            buf:   [0u8; QUEUE_SIZE],
            head:  0,
            tail:  0,
            count: 0,
        }
    }

    fn push(&mut self, scancode: u8) -> bool {
        if self.count >= QUEUE_SIZE {
            return false; // Queue penuh, drop
        }
        self.buf[self.tail] = scancode;
        self.tail = (self.tail + 1) % QUEUE_SIZE;
        self.count += 1;
        true
    }

    fn pop(&mut self) -> Option<u8> {
        if self.count == 0 {
            return None;
        }
        let sc = self.buf[self.head];
        self.head = (self.head + 1) % QUEUE_SIZE;
        self.count -= 1;
        Some(sc)
    }
}

lazy_static! {
    static ref QUEUE: Spinlock<ScancodeQueue> =
        Spinlock::new(ScancodeQueue::new());

    static ref KEYBOARD: Spinlock<Keyboard<layouts::Us104Key, ScancodeSet1>> =
        Spinlock::new(Keyboard::new(
            ScancodeSet1::new(),
            layouts::Us104Key,
            HandleControl::Ignore,
        ));
}

/// Dipanggil dari interrupt handler (IRQ1)
pub fn add_scancode(scancode: u8) {
    QUEUE.lock().push(scancode);
}

/// Init keyboard
pub fn init() {
    // Trigger lazy init
    let _ = QUEUE.lock();
    let _ = KEYBOARD.lock();
}

/// Baca karakter selanjutnya dari buffer (non-blocking)
/// Return None jika tidak ada input
pub fn read_char() -> Option<char> {
    let scancode = QUEUE.lock().pop()?;

    let mut kb = KEYBOARD.lock();
    if let Ok(Some(key_event)) = kb.add_byte(scancode) {
        if let Some(key) = kb.process_keyevent(key_event) {
            return match key {
                DecodedKey::Unicode(c)  => Some(c),
                DecodedKey::RawKey(kc)  => raw_key_to_char(kc),
            };
        }
    }
    None
}

fn raw_key_to_char(kc: KeyCode) -> Option<char> {
    match kc {
        KeyCode::Backspace => Some('\x08'),
        KeyCode::Delete    => Some('\x7F'),
        KeyCode::Return    => Some('\n'),
        KeyCode::Tab       => Some('\t'),
        KeyCode::Escape    => Some('\x1B'),
        _                  => None,
    }
}

// ── Tests ────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test_case]
    fn test_queue_push_pop() {
        let mut q = ScancodeQueue::new();
        assert!(q.push(0x1E)); // 'A' scancode
        assert_eq!(q.pop(), Some(0x1E));
        assert_eq!(q.pop(), None);
    }

    #[test_case]
    fn test_queue_full() {
        let mut q = ScancodeQueue::new();
        for _ in 0..QUEUE_SIZE {
            q.push(0x00);
        }
        // Queue penuh, push harus return false
        assert!(!q.push(0xFF));
    }
}
