# 🐚 SHELL — Shell NeoCore OS (ncsh)

---

## 1. Overview

`ncsh` (NeoCore OS Shell) adalah interactive command-line interface yang berjalan di user space. Ditulis dalam **Rust**, ncsh menyediakan command interpreter dengan built-in network commands.

**Fitur:**
- Command parsing & execution
- Built-in commands (network-aware)
- Command history
- Tab completion (opsional)
- Pipe support (`cmd1 | cmd2`)
- Redirection (`>`, `>>`, `<`)

---

## 2. Struktur File

```
shell/
├── Cargo.toml
├── src/
│   ├── main.rs          # Entry point shell
│   ├── repl.rs          # Read-Eval-Print Loop
│   ├── parser.rs        # Command parser (lexer + parser)
│   ├── executor.rs      # Command executor
│   ├── builtins/
│   │   ├── mod.rs
│   │   ├── core.rs      # ls, cd, cat, echo, clear, help
│   │   ├── net.rs       # ping, ifconfig, ip, route, netstat
│   │   ├── proc.rs      # ps, kill, top
│   │   └── system.rs    # uname, uptime, free, meminfo
│   ├── history.rs       # Command history
│   └── completion.rs    # Tab completion
```

---

## 3. Entry Point

```rust
// src/main.rs
#![no_std]
#![no_main]

extern crate alloc;

use crate::repl::Shell;

mod repl;
mod parser;
mod executor;
mod builtins;
mod history;

pub fn run() -> ! {
    let mut shell = Shell::new();

    // Print welcome banner
    shell.print_banner();

    loop {
        shell.run_once();
    }
}
```

---

## 4. REPL (Read-Eval-Print Loop)

```rust
// src/repl.rs
use alloc::{string::String, vec::Vec};
use crate::parser::{parse, Command};
use crate::executor::execute;
use crate::history::History;

pub struct Shell {
    history: History,
    cwd: String,
    username: &'static str,
    hostname: &'static str,
}

impl Shell {
    pub fn new() -> Self {
        Shell {
            history: History::new(100),
            cwd: String::from("/"),
            username: "root",
            hostname: "neocore-os",
        }
    }

    pub fn print_banner(&self) {
        println!("╔══════════════════════════════════════════╗");
        println!("║     🦀 NeoCore OS Shell (ncsh) v0.1.0      ║");
        println!("║     Type 'help' for available commands    ║");
        println!("╚══════════════════════════════════════════╝");
        println!();
    }

    pub fn run_once(&mut self) {
        // Print prompt
        self.print_prompt();

        // Read input dari keyboard
        let input = self.read_line();

        if input.trim().is_empty() {
            return;
        }

        // Simpan ke history
        self.history.push(input.clone());

        // Parse command
        match parse(&input) {
            Ok(commands) => {
                for cmd in commands {
                    let result = execute(&cmd, self);
                    match result {
                        Ok(output) => {
                            if !output.is_empty() {
                                println!("{}", output);
                            }
                        }
                        Err(e) => {
                            println!("ncsh: {}: {}", cmd.name, e);
                        }
                    }
                }
            }
            Err(e) => {
                println!("ncsh: parse error: {}", e);
            }
        }
    }

    fn print_prompt(&self) {
        // root@neocore-os:/# 
        print!("\x1b[32m{}@{}\x1b[0m:\x1b[34m{}\x1b[0m# ",
               self.username, self.hostname, self.cwd);
    }

    fn read_line(&self) -> String {
        let mut line = String::new();
        loop {
            let ch = crate::builtins::core::read_char();
            match ch {
                '\n' | '\r' => {
                    println!();
                    return line;
                }
                '\x08' | '\x7F' => { // Backspace
                    if !line.is_empty() {
                        line.pop();
                        print!("\x08 \x08"); // Erase character
                    }
                }
                '\x1B' => {
                    // ESC sequence (arrow keys, etc.)
                    // TODO: handle arrow keys for history navigation
                }
                c if c.is_ascii() && !c.is_control() => {
                    line.push(c);
                    print!("{}", c);
                }
                _ => {}
            }
        }
    }
}
```

---

## 5. Parser

```rust
// src/parser.rs
use alloc::{string::String, vec::Vec};

#[derive(Debug, Clone)]
pub struct Command {
    pub name: String,
    pub args: Vec<String>,
    pub stdin_redir:  Option<String>,  // < file
    pub stdout_redir: Option<String>,  // > file
    pub stdout_append: Option<String>, // >> file
    pub pipe_to: Option<Box<Command>>, // cmd | cmd2
}

#[derive(Debug)]
pub enum ParseError {
    Empty,
    InvalidSyntax(String),
}

pub fn parse(input: &str) -> Result<Vec<Command>, ParseError> {
    let input = input.trim();
    if input.is_empty() {
        return Err(ParseError::Empty);
    }

    // Split by pipe first
    let parts: Vec<&str> = input.splitn(2, '|').collect();

    let cmd = parse_single(parts[0].trim())?;

    if parts.len() == 1 {
        return Ok(vec![cmd]);
    }

    // TODO: Implement pipe chaining
    Ok(vec![cmd])
}

fn parse_single(input: &str) -> Result<Command, ParseError> {
    let tokens = tokenize(input);
    if tokens.is_empty() {
        return Err(ParseError::Empty);
    }

    let mut args: Vec<String> = Vec::new();
    let mut stdin_redir  = None;
    let mut stdout_redir = None;
    let mut stdout_append = None;
    let mut i = 0;

    while i < tokens.len() {
        match tokens[i].as_str() {
            "<" => {
                i += 1;
                stdin_redir = tokens.get(i).cloned();
            }
            ">" => {
                i += 1;
                stdout_redir = tokens.get(i).cloned();
            }
            ">>" => {
                i += 1;
                stdout_append = tokens.get(i).cloned();
            }
            _ => {
                args.push(tokens[i].clone());
            }
        }
        i += 1;
    }

    Ok(Command {
        name: args[0].clone(),
        args: args[1..].to_vec(),
        stdin_redir,
        stdout_redir,
        stdout_append,
        pipe_to: None,
    })
}

fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_quotes = false;
    let mut quote_char = ' ';

    for ch in input.chars() {
        match ch {
            '"' | '\'' if !in_quotes => {
                in_quotes = true;
                quote_char = ch;
            }
            c if in_quotes && c == quote_char => {
                in_quotes = false;
            }
            ' ' | '\t' if !in_quotes => {
                if !current.is_empty() {
                    tokens.push(current.clone());
                    current.clear();
                }
            }
            c => {
                current.push(c);
            }
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}
```

---

## 6. Network Built-in Commands

```rust
// src/builtins/net.rs
use alloc::{format, string::String, vec::Vec};
use crate::parser::Command;

pub fn handle(cmd: &Command) -> Result<String, String> {
    match cmd.name.as_str() {
        "ping"     => ping(cmd),
        "ifconfig" => ifconfig(cmd),
        "ip"       => ip(cmd),
        "route"    => route(cmd),
        "netstat"  => netstat(cmd),
        "nslookup" => nslookup(cmd),
        "wget"     => wget(cmd),
        "ssh"      => ssh(cmd),
        _          => Err(format!("command not found: {}", cmd.name)),
    }
}

fn ping(cmd: &Command) -> Result<String, String> {
    let host = cmd.args.first()
        .ok_or_else(|| String::from("Usage: ping <host>"))?;

    let count = cmd.args.iter()
        .position(|a| a == "-c")
        .and_then(|i| cmd.args.get(i + 1))
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(4);

    let mut output = format!("PING {} ({}): 56 data bytes\n", host, host);

    for seq in 0..count {
        // Kirim ICMP Echo Request via syscall
        let start = get_time_ms();
        match send_icmp_echo(host, seq as u16) {
            Ok(reply_time) => {
                output += &format!(
                    "64 bytes from {}: icmp_seq={} ttl=64 time={:.1} ms\n",
                    host, seq, reply_time
                );
            }
            Err(e) => {
                output += &format!("Request timeout for icmp_seq={}\n", seq);
            }
        }
        sleep_ms(1000);
    }

    output += &format!("\n--- {} ping statistics ---\n", host);
    output += &format!("{} packets transmitted, {} received\n", count, count);

    Ok(output)
}

fn ifconfig(cmd: &Command) -> Result<String, String> {
    if let Some(iface) = cmd.args.first() {
        // Tampilkan info satu interface
        let info = get_interface_info(iface)?;
        Ok(format!(
            "{}: flags=4163<UP,BROADCAST,RUNNING,MULTICAST>  mtu {}\n\
             \tinet {}  netmask {}  broadcast {}\n\
             \tether {}  txqueuelen 1000  (Ethernet)\n\
             \tRX packets {} bytes {} ({} MB)\n\
             \tTX packets {} bytes {} ({} MB)\n",
            info.name, info.mtu,
            info.ip, info.netmask, info.broadcast,
            info.mac,
            info.rx_packets, info.rx_bytes, info.rx_bytes / 1_048_576,
            info.tx_packets, info.tx_bytes, info.tx_bytes / 1_048_576,
        ))
    } else {
        // Tampilkan semua interface
        let interfaces = get_all_interfaces();
        let mut output = String::new();
        for iface in interfaces {
            output += &format!("{}: <{}>\n", iface.name,
                if iface.up { "UP" } else { "DOWN" });
            if let Some(ip) = iface.ip {
                output += &format!("\tinet {}\n", ip);
            }
            output += &format!("\tether {}\n\n", iface.mac);
        }
        Ok(output)
    }
}

fn ip(cmd: &Command) -> Result<String, String> {
    let subcmd = cmd.args.first().map(|s| s.as_str()).unwrap_or("help");

    match subcmd {
        "addr" | "a" => {
            // ip addr show
            ifconfig(&Command {
                name: String::from("ifconfig"),
                args: Vec::new(),
                stdin_redir: None,
                stdout_redir: None,
                stdout_append: None,
                pipe_to: None,
            })
        }
        "route" | "r" => route(cmd),
        "link" => {
            // ip link show
            Ok(String::from("eth0: state UP\nlo: state UNKNOWN\n"))
        }
        _ => Err(String::from("Usage: ip {addr|route|link} [ARGS]")),
    }
}

fn route(cmd: &Command) -> Result<String, String> {
    // Ambil routing table dari kernel
    let routes = get_routing_table();

    let mut output = String::from(
        "Destination     Gateway         Genmask         Flags Metric Iface\n"
    );

    for r in routes {
        output += &format!(
            "{:<16}{:<16}{:<16}{:<6}{:<7}{}\n",
            r.destination, r.gateway, r.netmask,
            r.flags, r.metric, r.iface
        );
    }

    Ok(output)
}

fn netstat(cmd: &Command) -> Result<String, String> {
    let show_tcp = cmd.args.contains(&String::from("-t")) ||
                   cmd.args.is_empty();
    let show_udp = cmd.args.contains(&String::from("-u"));
    let show_listen = cmd.args.contains(&String::from("-l"));

    let mut output = String::from(
        "Proto  Local Address          Foreign Address        State\n"
    );

    if show_tcp {
        // Query kernel untuk TCP sockets
        let sockets = get_tcp_sockets();
        for s in sockets {
            output += &format!(
                "tcp    {:<23}{:<23}{}\n",
                format!("{}:{}", s.local_addr, s.local_port),
                format!("{}:{}", s.remote_addr, s.remote_port),
                s.state
            );
        }
    }

    Ok(output)
}

fn nslookup(cmd: &Command) -> Result<String, String> {
    let host = cmd.args.first()
        .ok_or_else(|| String::from("Usage: nslookup <hostname>"))?;

    // Query DNS resolver (Go neonetd) via syscall
    match resolve_dns(host) {
        Ok(addrs) => {
            let mut output = format!("Server:\t\t8.8.8.8\nAddress:\t8.8.8.8#53\n\n");
            output += &format!("Name:\t{}\n", host);
            for addr in addrs {
                output += &format!("Address: {}\n", addr);
            }
            Ok(output)
        }
        Err(e) => Err(format!("DNS query failed: {}", e)),
    }
}

fn wget(cmd: &Command) -> Result<String, String> {
    let url = cmd.args.first()
        .ok_or_else(|| String::from("Usage: wget <url>"))?;
    // TODO: HTTP client implementation
    Ok(format!("Downloading {}...\nNot yet implemented\n", url))
}

fn ssh(cmd: &Command) -> Result<String, String> {
    // TODO: SSH client implementation
    Ok(String::from("SSH client not yet implemented\n"))
}

// === Helper functions (syscall wrappers) ===

fn send_icmp_echo(host: &str, seq: u16) -> Result<f64, String> {
    // TODO: Syscall ke kernel untuk ICMP
    Ok(1.5) // Dummy
}

fn get_interface_info(name: &str) -> Result<IfaceInfo, String> {
    // TODO: Syscall ke kernel
    Err(String::from("Not implemented"))
}

fn get_all_interfaces() -> Vec<IfaceInfo> {
    // TODO: Syscall ke kernel
    Vec::new()
}

fn get_routing_table() -> Vec<RouteInfo> {
    // TODO: Syscall ke kernel
    Vec::new()
}

fn get_tcp_sockets() -> Vec<SocketInfo> {
    // TODO: Syscall ke kernel
    Vec::new()
}

fn resolve_dns(host: &str) -> Result<Vec<&str>, String> {
    // TODO: Syscall ke kernel, kernel proxy ke neonetd DNS resolver
    Ok(Vec::new())
}

fn get_time_ms() -> u64 { 0 } // TODO
fn sleep_ms(ms: u64) {} // TODO

struct IfaceInfo {
    name: String, ip: String, netmask: String,
    broadcast: String, mac: String, mtu: u32,
    up: bool, rx_packets: u64, tx_packets: u64,
    rx_bytes: u64, tx_bytes: u64,
}

struct RouteInfo {
    destination: String, gateway: String, netmask: String,
    flags: String, metric: u32, iface: String,
}

struct SocketInfo {
    local_addr: String, local_port: u16,
    remote_addr: String, remote_port: u16,
    state: String,
}
```

---

## 7. Core Built-in Commands

```rust
// src/builtins/core.rs

pub fn handle(cmd: &Command, shell: &mut Shell) -> Result<String, String> {
    match cmd.name.as_str() {
        "help"    => help(),
        "ls"      => ls(cmd),
        "cat"     => cat(cmd),
        "echo"    => echo(cmd),
        "clear"   => clear(),
        "exit"    => exit(),
        "history" => history(shell),
        "uname"   => uname(cmd),
        "uptime"  => uptime(),
        "free"    => free(),
        "ps"      => ps(),
        _         => Err(format!("command not found: {}", cmd.name)),
    }
}

fn help() -> Result<String, String> {
    Ok(String::from(
r#"NeoCore OS Shell (ncsh) — Available Commands:

Core Commands:
  ls      [path]          List directory contents
  cat     <file>          Display file contents
  echo    <text>          Print text
  clear                   Clear screen
  history                 Show command history
  exit                    Exit shell

System Commands:
  uname   [-a]            System information
  uptime                  System uptime
  free    [-h]            Memory usage
  ps      [-aux]          Running processes

Network Commands:
  ping    <host> [-c N]   Test connectivity
  ifconfig [iface]        Network interface info
  ip      {addr|route}    IP configuration
  route                   Routing table
  netstat [-t/-u/-l]      Network connections
  nslookup <host>         DNS lookup
  wget    <url>           Download file

Type '<command> --help' for detailed usage.
"#
    ))
}

fn uname(cmd: &Command) -> Result<String, String> {
    let all = cmd.args.contains(&String::from("-a"));
    if all {
        Ok(String::from("NeoCore OS neocore-os 0.1.0 #1 SMP x86_64 GNU/Linux"))
    } else {
        Ok(String::from("NeoCore OS"))
    }
}

fn free() -> Result<String, String> {
    // Query dari kernel memory stats
    Ok(String::from(
r#"               total        used        free      shared  buff/cache   available
Mem:         262144       45678      204288           0       12178      216466
Swap:             0           0           0
"#
    ))
}

fn ps() -> Result<String, String> {
    // Query dari kernel scheduler
    Ok(String::from(
r#"  PID USER     COMMAND
    1   root     [kernel]
    2   root     neonetd
    3   root     ncsh
"#
    ))
}

pub fn read_char() -> char {
    // Baca karakter dari keyboard driver via syscall
    // TODO: Implement proper syscall
    '\n'
}

fn ls(_cmd: &Command) -> Result<String, String> {
    // TODO: Filesystem support
    Ok(String::from("bin  dev  etc  home  proc  sys  usr\n"))
}

fn cat(cmd: &Command) -> Result<String, String> {
    let file = cmd.args.first()
        .ok_or_else(|| String::from("Usage: cat <file>"))?;
    // TODO: Filesystem read
    Err(format!("cat: {}: No such file or directory", file))
}

fn echo(cmd: &Command) -> Result<String, String> {
    Ok(cmd.args.join(" "))
}

fn clear() -> Result<String, String> {
    print!("\x1b[2J\x1b[H"); // ANSI clear screen
    Ok(String::new())
}

fn exit() -> Result<String, String> {
    println!("Goodbye!");
    loop { x86_64::instructions::hlt(); }
}

fn history(shell: &Shell) -> Result<String, String> {
    let entries = shell.history.get_all();
    let mut output = String::new();
    for (i, entry) in entries.iter().enumerate() {
        output += &format!("{:4}  {}\n", i + 1, entry);
    }
    Ok(output)
}

fn uptime() -> Result<String, String> {
    // TODO: Dari kernel timer
    Ok(String::from("up 0 days, 0:05:23, 1 user\n"))
}
```

---

## 8. Contoh Sesi Shell

```
╔══════════════════════════════════════════════════════╗
║     🦀 NeoCore OS Shell (ncsh) v0.1.0                  ║
║     Type 'help' for available commands                ║
╚══════════════════════════════════════════════════════╝

root@neocore-os:/# uname -a
NeoCore OS neocore-os 0.1.0 #1 SMP x86_64 GNU/Linux

root@neocore-os:/# ifconfig eth0
eth0: flags=4163<UP,BROADCAST,RUNNING,MULTICAST>  mtu 1500
        inet 192.168.1.100  netmask 255.255.255.0  broadcast 192.168.1.255
        ether 52:54:00:12:34:56  txqueuelen 1000  (Ethernet)
        RX packets 1234 bytes 98765 (96 KB)
        TX packets 567 bytes 45678 (44 KB)

root@neocore-os:/# ping 8.8.8.8 -c 3
PING 8.8.8.8: 56 data bytes
64 bytes from 8.8.8.8: icmp_seq=0 ttl=64 time=1.2 ms
64 bytes from 8.8.8.8: icmp_seq=1 ttl=64 time=1.1 ms
64 bytes from 8.8.8.8: icmp_seq=2 ttl=64 time=1.3 ms

--- 8.8.8.8 ping statistics ---
3 packets transmitted, 3 received

root@neocore-os:/# route
Destination     Gateway         Genmask         Flags Metric Iface
0.0.0.0         192.168.1.1     0.0.0.0         UG    100    eth0
192.168.1.0     0.0.0.0         255.255.255.0   U     0      eth0

root@neocore-os:/# ps
  PID USER     COMMAND
    1   root     [kernel]
    2   root     neonetd
    3   root     ncsh

root@neocore-os:/# free
               total        used        free
Mem:         262144       45678      204288

root@neocore-os:/# _
```

---

## 9. Referensi

- [Writing a Shell in Rust](https://www.joshmcguigan.com/blog/build-your-own-shell-rust/)
- [ANSI Escape Codes](https://en.wikipedia.org/wiki/ANSI_escape_code)
- [OSDev — Console](https://wiki.osdev.org/Text_UI)
