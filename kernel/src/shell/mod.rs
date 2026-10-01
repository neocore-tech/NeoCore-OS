// kernel/src/shell/mod.rs — NeoCore Shell (ncsh) — Kernel-side REPL
// Berjalan sebagai cooperative async task di atas executor
// SPDX-License-Identifier: MIT

pub mod builtins;

extern crate alloc;
use alloc::string::{String, ToString};

use crate::{print, println};
use crate::drivers::{keyboard, vga};

// ── Shell Prompt ───────────────────────────────────────────────────────────

const PROMPT: &str = "ncsh> ";

// ── Async Shell Task ───────────────────────────────────────────────────────

/// Task utama shell — baca karakter dari keyboard, eksekusi perintah
pub async fn run() {
    use vga::Color;

    // Tampilkan banner
    vga::set_color(Color::LightCyan, Color::Black);
    println!("  NeoCore Shell (ncsh) v0.1 - type 'help' for commands");
    vga::set_color(Color::White, Color::Black);

    let mut input = String::new();
    print_prompt();

    loop {
        // Yield sampai ada karakter (cooperative)
        let ch = KeyboardFuture.await;

        match ch {
            '\n' => {
                println!(); // newline setelah enter
                let cmd = input.trim().to_string();
                if !cmd.is_empty() {
                    execute(&cmd);
                }
                input.clear();
                print_prompt();
            }
            '\x08' => {
                // Backspace
                if !input.is_empty() {
                    input.pop();
                    // Hapus karakter terakhir di VGA
                    print!("\x08 \x08");
                }
            }
            '\x1B' => {
                // Escape — abaikan (bisa diperluas untuk arrow keys)
            }
            c if c.is_ascii() && !c.is_control() => {
                input.push(c);
                // Echo karakter ke layar
                vga::set_color(Color::LightGreen, Color::Black);
                print!("{}", c);
                vga::set_color(Color::White, Color::Black);
            }
            _ => {}
        }
    }
}

// ── Command Executor ───────────────────────────────────────────────────────

fn execute(cmd: &str) {
    let parts: alloc::vec::Vec<&str> = cmd.splitn(2, ' ').collect();
    let command = parts[0];
    let args    = parts.get(1).copied().unwrap_or("");

    match command {
        "help"    => builtins::help(),
        "clear"   => builtins::clear(),
        "uptime"  => builtins::uptime(),
        "meminfo" => builtins::meminfo(),
        "uname"   => builtins::uname(),
        "echo"    => builtins::echo(args),
        "lspci"   => builtins::lspci(),
        "ls"      => builtins::ls(),
        "halt"    => builtins::halt(),
        "sched"   => builtins::sched_info(),
        "neofetch"=> builtins::neofetch(),
        "ip"      => builtins::ip(args),
        "ping"    => builtins::ping(args),
        "cpu"     => builtins::cpu(args),
        "sys"     => builtins::sys(),
        "time"    => builtins::time(),
        "reboot"  => builtins::reboot(),
        "ram" | "diag" | "test" | "load" | "hw" | "temp" | "usb" | "dmesg" |
        "dsk" | "fs" | "mount" | "umount" | "cd" | "pwd" | "cp" | "mv" | "rm" | "mkdir" | "cat" | "nano" | "find" | "grep" | "df" | "du" | "iostat" |
        "net" | "dns" | "rt" | "netstat" | "arp" | "ifconfig" | "hostname" |
        "proc" | "svc" | "log" | "sec" | "pkg" | "usr" | "perm" | "whoami" | "passwd" | "env" | "kill" | "ps" | "top" | "free" |
        "power" => {
            vga::set_color(vga::Color::LightBlue, vga::Color::Black);
            println!("  [INFO] Command '{}' is planned for future phases (Phase 4/5).", command);
            vga::set_color(vga::Color::White, vga::Color::Black);
        },
        _         => {
            use crate::drivers::vga::Color;
            vga::set_color(Color::LightRed, Color::Black);
            println!("ncsh: command not found: '{}'", command);
            vga::set_color(Color::White, Color::Black);
        }
    }
}

fn print_prompt() {
    use vga::Color;
    vga::set_color(Color::LightBlue, Color::Black);
    print!("{}", PROMPT);
    vga::set_color(Color::White, Color::Black);
}

// ── Keyboard Future ────────────────────────────────────────────────────────

/// Future yang yield sampai ada karakter keyboard tersedia
struct KeyboardFuture;

impl core::future::Future for KeyboardFuture {
    type Output = char;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        match keyboard::read_char() {
            Some(c) => core::task::Poll::Ready(c),
            None    => {
                // Tidak ada input — daftarkan waker dan kembalikan Pending
                // Waker akan di-wake oleh keyboard ISR (via wake_queue)
                // Sementara ini, kita re-schedule dari timer setiap tick
                cx.waker().wake_by_ref();
                core::task::Poll::Pending
            }
        }
    }
}
