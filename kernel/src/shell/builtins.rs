// kernel/src/shell/builtins.rs — Built-in commands ncsh
// SPDX-License-Identifier: MIT

use crate::{print, println, serial_println};
use crate::drivers::vga::{self, Color};
use crate::interrupts::pit;
use crate::memory;
use crate::task::scheduler;

// ── help ──────────────────────────────────────────────────────────────────

pub fn help() {
    vga::set_color(Color::Yellow, Color::Black);
    println!("+-------------------------------------------------------------+");
    println!("|  [System]: sys, cpu, ram, diag, test, ver, load, time, hw   |");
    println!("|            temp, pci, usb, uptime, dmesg                    |");
    println!("|  [File]  : dsk, fs, mount, umount, ls, cd, pwd, cp, mv, rm  |");
    println!("|            mkdir, cat, nano, find, grep, df, du, iostat     |");
    println!("|  [Net]   : net, ip, dns, ping, rt, netstat, arp, ifconfig   |");
    println!("|  [Proc]  : proc, svc, log, sec, pkg, usr, perm, whoami      |");
    println!("|            passwd, env, echo, kill, ps, top, free           |");
    println!("|  [Util]  : help, clear, reboot, power, halt, neofetch       |");
    println!("|                                                             |");
    println!("|  Note: Most commands are planned for Phase 4 & Phase 5.     |");
    println!("+-------------------------------------------------------------+");
    vga::set_color(Color::White, Color::Black);
}

// ── clear ─────────────────────────────────────────────────────────────────

pub fn clear() {
    vga::set_color(Color::White, Color::Black);
    crate::drivers::vga::WRITER.lock().clear_screen();
}

// ── uname ─────────────────────────────────────────────────────────────────

pub fn uname() {
    vga::set_color(Color::Cyan, Color::Black);
    println!("NeoCore OS  v0.1.0  x86_64");
    println!("Kernel: Rust (nightly)  Network: Go neonetd");
    println!("Build:  2026-10-01  Author: NeoCore Contributors");
    vga::set_color(Color::White, Color::Black);
}

// ── uptime ────────────────────────────────────────────────────────────────

pub fn uptime() {
    let secs  = pit::uptime_secs();
    let ticks = pit::ticks();
    let hours = secs / 3600;
    let mins  = (secs % 3600) / 60;
    let s     = secs % 60;

    vga::set_color(Color::LightGreen, Color::Black);
    println!("Uptime: {:02}h {:02}m {:02}s  ({} ticks / 1000 Hz)",
             hours, mins, s, ticks);
    vga::set_color(Color::White, Color::Black);
}

// ── meminfo ───────────────────────────────────────────────────────────────

pub fn meminfo() {
    let s = memory::heap::stats();
    let used_kb  = s.used  / 1024;
    let free_kb  = s.free  / 1024;
    let total_kb = s.total / 1024;

    vga::set_color(Color::LightCyan, Color::Black);
    println!("Heap Memory:");
    println!("  Total : {:>6} KB  ({} MB)", total_kb, total_kb / 1024);
    println!("  Used  : {:>6} KB", used_kb);
    println!("  Free  : {:>6} KB", free_kb);

    // Bar progress
    let pct = if total_kb > 0 { used_kb * 40 / total_kb } else { 0 };
    print!("  [");
    vga::set_color(Color::LightRed, Color::Black);
    for _ in 0..pct         { print!("█"); }
    vga::set_color(Color::DarkGray, Color::Black);
    for _ in pct..40        { print!("░"); }
    vga::set_color(Color::LightCyan, Color::Black);
    println!("] {}%", if total_kb > 0 { used_kb * 100 / total_kb } else { 0 });

    vga::set_color(Color::White, Color::Black);
}

// ── sched ─────────────────────────────────────────────────────────────────

pub fn sched_info() {
    let s = scheduler::stats();
    vga::set_color(Color::Magenta, Color::Black);
    println!("Scheduler:");
    println!("  Mode   : Cooperative (Phase 1)");
    println!("  Ticks  : {}  (1 ms/tick)", s.ticks);
    println!("  Uptime : {} seconds", s.uptime_secs);
    println!("  Phase 5: Preemptive Round-Robin (planned)");
    vga::set_color(Color::White, Color::Black);
}

// ── echo ──────────────────────────────────────────────────────────────────

pub fn echo(args: &str) {
    println!("{}", args);
}

// ── lspci ─────────────────────────────────────────────────────────────────

pub fn lspci() {
    let devices = crate::drivers::pci::PCI_DEVICES.lock();
    if devices.is_empty() {
        println!("Tidak ada device PCI yang terdeteksi.");
        return;
    }

    vga::set_color(Color::LightCyan, Color::Black);
    println!("Daftar Perangkat PCI ({} total):", devices.len());
    println!("+-----+------+------+---------+---------+----------+");
    println!("| Bus | Slot | Func | Vendor  | Device  | Class ID |");
    println!("+-----+------+------+---------+---------+----------+");
    
    vga::set_color(Color::White, Color::Black);
    for dev in devices.iter() {
        println!(
            "| {:03} |  {:02}  |   {}  |  0x{:04X} |  0x{:04X} |   0x{:02X}   |",
            dev.bus, dev.slot, dev.func, dev.vendor_id, dev.device_id, dev.class_id
        );
    }
    
    vga::set_color(Color::LightCyan, Color::Black);
    println!("+-----+------+------+---------+---------+----------+");
    vga::set_color(Color::White, Color::Black);
}

// ── ls ────────────────────────────────────────────────────────────────────

pub fn ls() {
    let fs_lock = crate::fs::ROOT_FS.lock();
    if let Some(fs) = fs_lock.as_ref() {
        let root_cluster = fs.bpb.root_cluster;
        match fs.read_dir(root_cluster) {
            Ok(entries) => {
                vga::set_color(Color::LightCyan, Color::Black);
                println!("Isi Root Direktori:");
                vga::set_color(Color::White, Color::Black);
                
                for entry in entries {
                    if entry.is_dir {
                        vga::set_color(Color::LightBlue, Color::Black);
                        println!("  [DIR]  {}", entry.name);
                    } else {
                        vga::set_color(Color::White, Color::Black);
                        println!("  [FILE] {} ({} bytes)", entry.name, entry.size);
                    }
                }
            }
            Err(_) => {
                // Because bootimage doesn't create a real FAT32, we will mock it for now!
                vga::set_color(Color::LightCyan, Color::Black);
                println!("Isi Root Direktori (Virtual/Mock):");
                vga::set_color(Color::White, Color::Black);
                
                vga::set_color(Color::LightBlue, Color::Black);
                println!("  [DIR]  system");
                println!("  [DIR]  users");
                vga::set_color(Color::White, Color::Black);
                println!("  [FILE] kernel.bin (40960 bytes)");
                println!("  [FILE] config.ini (128 bytes)");
                println!("  [FILE] readme.txt (512 bytes)");
            }
        }
    } else {
        vga::set_color(Color::LightRed, Color::Black);
        println!("Tidak ada sistem file yang terdeteksi.");
    }
    vga::set_color(Color::White, Color::Black);
}

// ── halt ──────────────────────────────────────────────────────────────────

pub fn halt() {
    vga::set_color(Color::LightRed, Color::Black);
    println!("Halting NeoCore OS...");
    println!("Power off QEMU dengan Ctrl+A, X");
    vga::set_color(Color::White, Color::Black);
    serial_println!("[HALT] Kernel halted by user");

    x86_64::instructions::interrupts::disable();
    loop {
        x86_64::instructions::hlt();
    }
}

// ── neofetch ──────────────────────────────────────────────────────────────

pub fn neofetch() {
    let secs  = pit::uptime_secs();
    let mins  = (secs % 3600) / 60;
    let s     = memory::heap::stats();
    let used_kb  = s.used  / 1024;
    let total_kb = s.total / 1024;

    // Get CPU Brand String if possible
    let mut cpu_name = "x86_64 (Generic)";
    let mut brand = [0u8; 48];
    unsafe {
        use core::arch::x86_64::__cpuid;
        let res_ext = __cpuid(0x80000000);
        if res_ext.eax >= 0x80000004 {
            let b1 = __cpuid(0x80000002);
            let b2 = __cpuid(0x80000003);
            let b3 = __cpuid(0x80000004);
            brand[0..4].copy_from_slice(&b1.eax.to_le_bytes());
            brand[4..8].copy_from_slice(&b1.ebx.to_le_bytes());
            brand[8..12].copy_from_slice(&b1.ecx.to_le_bytes());
            brand[12..16].copy_from_slice(&b1.edx.to_le_bytes());
            brand[16..20].copy_from_slice(&b2.eax.to_le_bytes());
            brand[20..24].copy_from_slice(&b2.ebx.to_le_bytes());
            brand[24..28].copy_from_slice(&b2.ecx.to_le_bytes());
            brand[28..32].copy_from_slice(&b2.edx.to_le_bytes());
            brand[32..36].copy_from_slice(&b3.eax.to_le_bytes());
            brand[36..40].copy_from_slice(&b3.ebx.to_le_bytes());
            brand[40..44].copy_from_slice(&b3.ecx.to_le_bytes());
            brand[44..48].copy_from_slice(&b3.edx.to_le_bytes());
            let mut len = 0;
            while len < 48 && brand[len] != 0 { len += 1; }
            if let Ok(s) = core::str::from_utf8(&brand[0..len]) {
                cpu_name = s;
            }
        }
    }

    println!("");
    vga::set_color(Color::Cyan, Color::Black);
    println!("       _.-._        neo@NeoCore");
    println!("      | | | |       -----------");
    println!("     _| | | |_      OS: NeoCore OS 0.1.0 x86_64");
    println!("    | | | | | |     Uptime: {} mins", mins);
    println!("    | | | | | |     Shell: ncsh 0.1");
    println!("    | _.-._ | |     Resolution: 1280x720");
    println!("    | | | | | |     CPU: {}", cpu_name);
    println!("    |       | |     Memory: {} MiB / {} MiB", used_kb / 1024, total_kb / 1024);
    println!("    |_______|/      ");
    println!("");
    
    // Cetak warna-warni menggunakan karakter Spasi dengan Background Color (solusi untuk font error)
    print!("    ");
    vga::set_color(Color::Black, Color::Black); print!("  ");
    vga::set_color(Color::Black, Color::Red); print!("  ");
    vga::set_color(Color::Black, Color::Green); print!("  ");
    vga::set_color(Color::Black, Color::Yellow); print!("  ");
    vga::set_color(Color::Black, Color::Blue); print!("  ");
    vga::set_color(Color::Black, Color::Magenta); print!("  ");
    vga::set_color(Color::Black, Color::Cyan); print!("  ");
    vga::set_color(Color::Black, Color::LightGray); println!("  ");
    
    vga::set_color(Color::White, Color::Black);
    print!("    ");
    vga::set_color(Color::Black, Color::DarkGray); print!("  ");
    vga::set_color(Color::Black, Color::LightRed); print!("  ");
    vga::set_color(Color::Black, Color::LightGreen); print!("  ");
    vga::set_color(Color::Black, Color::Yellow); print!("  ");
    vga::set_color(Color::Black, Color::LightBlue); print!("  ");
    vga::set_color(Color::Black, Color::Pink); print!("  ");
    vga::set_color(Color::Black, Color::LightCyan); print!("  ");
    vga::set_color(Color::Black, Color::White); println!("  ");

    println!("");
    vga::set_color(Color::White, Color::Black);
}

// ── ip ────────────────────────────────────────────────────────────────────

pub fn ip(args: &str) {
    if args == "a" || args == "addr" {
        vga::set_color(Color::LightCyan, Color::Black);
        println!("1: lo: <LOOPBACK,UP,LOWER_UP> mtu 65536");
        vga::set_color(Color::White, Color::Black);
        println!("    inet 127.0.0.1/8 scope host lo");
        
        vga::set_color(Color::LightCyan, Color::Black);
        println!("2: net0: <BROADCAST,MULTICAST,UP,LOWER_UP> mtu 1500");
        vga::set_color(Color::White, Color::Black);
        println!("    link/ether 52:54:00:12:34:56 brd ff:ff:ff:ff:ff:ff");
        println!("    inet 10.0.2.15/24 brd 10.0.2.255 scope global net0");
    } else {
        println!("Usage: ip a");
    }
}

// ── cpu ───────────────────────────────────────────────────────────────────

pub fn cpu(_args: &str) {
    use core::arch::x86_64::__cpuid;
    vga::set_color(Color::LightCyan, Color::Black);
    println!("=== CPU Information ===");
    vga::set_color(Color::White, Color::Black);

    unsafe {
        // CPUID EAX=0: Highest Function Parameter and Manufacturer ID
        let res = __cpuid(0);
        let mut vendor = [0u8; 13];
        let ebx = res.ebx.to_le_bytes();
        let edx = res.edx.to_le_bytes();
        let ecx = res.ecx.to_le_bytes();
        
        vendor[0..4].copy_from_slice(&ebx);
        vendor[4..8].copy_from_slice(&edx);
        vendor[8..12].copy_from_slice(&ecx);
        vendor[12] = 0;
        
        let vendor_str = core::str::from_utf8(&vendor[0..12]).unwrap_or("Unknown");
        println!("Vendor      : {}", vendor_str);
        
        // CPUID EAX=1: Processor Info and Feature Bits
        let res1 = __cpuid(1);
        let family = (res1.eax >> 8) & 0xF;
        let model = (res1.eax >> 4) & 0xF;
        let stepping = res1.eax & 0xF;
        println!("Family      : {}", family);
        println!("Model       : {}", model);
        println!("Stepping    : {}", stepping);
        
        // Extended CPUID EAX=0x80000000
        let res_ext = __cpuid(0x80000000);
        if res_ext.eax >= 0x80000004 {
            let mut brand = [0u8; 48];
            let b1 = __cpuid(0x80000002);
            let b2 = __cpuid(0x80000003);
            let b3 = __cpuid(0x80000004);
            brand[0..4].copy_from_slice(&b1.eax.to_le_bytes());
            brand[4..8].copy_from_slice(&b1.ebx.to_le_bytes());
            brand[8..12].copy_from_slice(&b1.ecx.to_le_bytes());
            brand[12..16].copy_from_slice(&b1.edx.to_le_bytes());
            brand[16..20].copy_from_slice(&b2.eax.to_le_bytes());
            brand[20..24].copy_from_slice(&b2.ebx.to_le_bytes());
            brand[24..28].copy_from_slice(&b2.ecx.to_le_bytes());
            brand[28..32].copy_from_slice(&b2.edx.to_le_bytes());
            brand[32..36].copy_from_slice(&b3.eax.to_le_bytes());
            brand[36..40].copy_from_slice(&b3.ebx.to_le_bytes());
            brand[40..44].copy_from_slice(&b3.ecx.to_le_bytes());
            brand[44..48].copy_from_slice(&b3.edx.to_le_bytes());
            
            // Cari null terminator
            let mut len = 0;
            while len < 48 && brand[len] != 0 {
                len += 1;
            }
            
            let brand_str = core::str::from_utf8(&brand[0..len]).unwrap_or("Unknown CPU");
            println!("Brand String: {}", brand_str);
        } else {
            println!("Brand String: Not Supported");
        }
    }
}

// ── ping ──────────────────────────────────────────────────────────────────

pub fn ping(args: &str) {
    if args.is_empty() {
        println!("Usage: ping <ip_address>");
        return;
    }
    
    let target = args;
    vga::set_color(Color::White, Color::Black);
    println!("PING {} ({}): 56 data bytes", target, target);
    
    // Simulate ICMP Echo Reply
    for i in 1..=4 {
        // Sleep for a bit to simulate network latency
        for _ in 0..50000000 { core::hint::spin_loop(); }
        println!("64 bytes from {}: icmp_seq={} ttl=64 time=1.23 ms", target, i);
    }
    
    println!("");
    println!("--- {} ping statistics ---", target);
    println!("4 packets transmitted, 4 packets received, 0.0% packet loss");
}

// ── sys ───────────────────────────────────────────────────────────────────

pub fn sys() {
    use crate::memory;
    vga::set_color(Color::LightCyan, Color::Black);
    println!("=== System Information ===");
    vga::set_color(Color::White, Color::Black);
    println!("OS Name   : NeoCore OS");
    println!("Version   : 0.1.0-alpha");
    println!("Build     : Rust nightly x86_64-unknown-none");
    println!("Arch      : x86_64 (64-bit)");
    println!("Kernel    : Monolithic (Custom)");
    println!("Scheduler : Cooperative (Phase 1)");
    let stats = memory::heap::stats();
    let total = stats.total / 1024;
    let used  = stats.used / 1024;
    println!("Heap Mem  : {} KB used / {} KB total", used, total);
}

// ── time ──────────────────────────────────────────────────────────────────

pub fn time() {
    use x86_64::instructions::port::Port;
    let mut cmos_addr = Port::<u8>::new(0x70);
    let mut cmos_data = Port::<u8>::new(0x71);

    unsafe {
        let mut read_rtc = |reg: u8| -> u8 {
            cmos_addr.write(reg);
            let val = cmos_data.read();
            // BCD to binary conversion
            (val & 0x0F) + ((val / 16) * 10)
        };

        let second = read_rtc(0x00);
        let minute = read_rtc(0x02);
        let hour   = read_rtc(0x04);
        let day    = read_rtc(0x07);
        let month  = read_rtc(0x08);
        let year   = read_rtc(0x09); // BCD 2 digits

        vga::set_color(Color::Yellow, Color::Black);
        println!("System RTC Time: 20{:02}-{:02}-{:02} {:02}:{:02}:{:02} UTC", 
                 year, month, day, hour, minute, second);
        vga::set_color(Color::White, Color::Black);
    }
}

// ── reboot ────────────────────────────────────────────────────────────────

pub fn reboot() {
    use x86_64::instructions::port::Port;
    vga::set_color(Color::LightRed, Color::Black);
    println!("Rebooting system...");
    unsafe {
        let mut port = Port::<u8>::new(0x64); // Keyboard controller port
        port.write(0xFE); // Reset command
    }
}

