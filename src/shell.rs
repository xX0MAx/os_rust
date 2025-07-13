use crate::{println, print, println_colored, print_colored};
use crate::vga_buffer::{WRITER, buffer_copy, buffer_clear, Color, HistoryBuffer};
use crate::ramfs::{Node, CURRENT_DIR, ROOT_DIR, NodeRef};
use alloc::{string::{String, ToString}, vec::Vec, rc::Rc, format};
use core::arch::asm;

use crate::interrupts::{ENTER_PRESSED, UP_PRESSED, DOWN_PRESSED};

static mut DIR_STACK: Vec<NodeRef> = Vec::new();

pub fn shell_loop() -> ! {
    unsafe {
        if ROOT_DIR.is_none() {
            ROOT_DIR = Some(Node::new_dir());
        }
        if let Some(root) = &ROOT_DIR {
            CURRENT_DIR = Some(root.clone());
            DIR_STACK.clear();
            DIR_STACK.push(root.clone());
        }
    }
    let current_dir = unsafe {
        CURRENT_DIR.as_ref().expect("File system not initialized").clone()
    };
    let mut counter = 0;
    let mut max_history = 0;
    let mut history_command = HistoryBuffer::new(10);
    print_colored!(Color::Magenta, Color::Black,"\n/ > ");
    buffer_clear();
    loop {
        unsafe {
            if ENTER_PRESSED {
                let mut buf = [0u8; 256]; 
                let len = buffer_copy(&mut buf);
                let s = core::str::from_utf8(&buf[..len]).unwrap_or("<invalid utf8>");
                history_command.push_line(s.trim_end_matches('\n').to_string());
                
                if max_history < 10{
                    max_history += 1;
                    counter = max_history;
                }

                execute_command(s);
                ENTER_PRESSED = false;
                let path = get_path(&DIR_STACK);
                print!("\n");
                print_colored!(Color::Magenta, Color::Black, "{}", path);
                print_colored!(Color::Magenta, Color::Black, " > ");
                buffer_clear();
            }

            else if UP_PRESSED{
                if counter > 0 {
                    counter -= 1;
                }
                let command = history_command.get_line(counter);

                let mut writer = WRITER.lock();
                writer.check_write_row();
                let mut row = writer.get_write_row();
                writer.clear_row(row);
                writer.write_col_null();
                drop(writer);

                let path = get_path(&DIR_STACK);
                print_colored!(Color::Magenta, Color::Black, "{}", path);
                print_colored!(Color::Magenta, Color::Black, " > ");
                buffer_clear();
                if let Some(cmd) = command {
                    print!("{}", cmd);
                }
                UP_PRESSED = false;
            }

            else if DOWN_PRESSED{
                if counter < max_history {
                    counter += 1;
                }
                let command = history_command.get_line(counter);
                let mut writer = WRITER.lock();
                writer.check_write_row();
                let mut row = writer.get_write_row();
                writer.clear_row(row);
                writer.write_col_null();
                drop(writer);
                
                let path = get_path(&DIR_STACK);
                print_colored!(Color::Magenta, Color::Black, "{}", path);
                print_colored!(Color::Magenta, Color::Black, " > ");
                buffer_clear();
                if let Some(cmd) = command {
                    print!("{}", cmd);
                }
                DOWN_PRESSED = false;
            }

            else{
                x86_64::instructions::hlt();
            }
        }
    }
}

pub fn execute_command(command: &str) {
    let mut parts = command.trim().split_whitespace();
    let cmd = parts.next().unwrap_or("");
    
    let current_dir = unsafe {
        match &CURRENT_DIR {
            Some(dir) => dir.clone(),
            None => {
                println!("File system not initialized");
                return;
            }
        }
    };

    match cmd {
        "" => {},
        "help" => {
            println!("Available commands:");
            print_colored!(Color::Green, Color::Black,"  help");
            println!(" - show this help");
            print_colored!(Color::Green, Color::Black,"  clear");
            println!(" - clear screen");
            print_colored!(Color::Green, Color::Black,"  off");
            println!(" - off system");
            print_colored!(Color::Green, Color::Black,"  restart");
            println!(" - restart system");
            print_colored!(Color::Green, Color::Black,"  mkdir");
            println!(" - create directory");
            print_colored!(Color::Green, Color::Black,"  touch");
            println!(" - create file");
            print_colored!(Color::Green, Color::Black,"  ls");
            println!(" - show directory and files");
            print_colored!(Color::Green, Color::Black,"  cd");
            println!(" - change directory");
            print_colored!(Color::Green, Color::Black,"  rm");
            println!(" - remove file");
            print_colored!(Color::Green, Color::Black,"  rmdir");
            println!(" - remove directory");
            print_colored!(Color::Green, Color::Black,"  write");
            println!(" - write data in file");
            print_colored!(Color::Green, Color::Black,"  open");
            println!(" - take data from file");
            print_colored!(Color::Green, Color::Black,"  time");
            println!(" - show time and date");
        },
        "clear" => WRITER.lock().clear_screen(),
        "off" => unsafe{ outw(0x604, 0x2000); },
        "restart" =>{
            unsafe{
                outb(0x64, 0xFE);
                loop {
                    asm!("hlt");
                }
            };
        },
        "time" => unsafe {
            outb(0x70, 0x02);
            let min = inb(0x71);
            outb(0x70, 0x04);
            let h = inb(0x71);

            outb(0x70, 0x07);
            let d = inb(0x71);
            outb(0x70, 0x08);
            let m = inb(0x71);
            outb(0x70, 0x09);
            let y = inb(0x71);

            println!("    {:02x}:{:02x}", h, min);
            println!("    {:02x}.{:02x}.{:02x}", d, m, y);
        },
        "mkdir" => {
            if let Some(name) = parts.next() {
                let dir = Node::new_dir();
                if let Err(e) = Node::add_entry(&current_dir, name.to_string(), dir) {
                    println_colored!(Color::Red, Color::Black, "Error creating directory: {}", e);
                }
            } else {
                println_colored!(Color::Green, Color::Black, "Usage: mkdir <name>");
            }
        }
        "touch" => {
            if let Some(name) = parts.next() {
                let file = Node::new_file();
                if let Err(e) = Node::add_entry(&current_dir, name.to_string(), file) {
                    println_colored!(Color::Red, Color::Black, "Error creating file: {}", e);
                }
            } else {
                println_colored!(Color::Green, Color::Black, "Usage: touch <name>");
            }
        }
        "ls" => {
            let current_dir_borrow = current_dir.borrow();
            match &*current_dir_borrow {
                Node::Directory { entries } => {
                    for (name, node) in entries {
                        let n = node.borrow();
                        match &*n {
                            Node::Directory { .. } => println!("{}  [dir]", name),
                            Node::File { .. } => println!("{}  [file]", name),
                        }
                    }
                }
                _ => println_colored!(Color::Red, Color::Black, "Current directory is not a directory"),
            }
        }
        "cd" => {
            if let Some(arg) = parts.next() {
                if arg == ".." {
                    unsafe {
                        if DIR_STACK.len() > 1 {
                            DIR_STACK.pop();
                            CURRENT_DIR = DIR_STACK.last().cloned();
                        } else {
                            println_colored!(Color::Red, Color::Black, "Already at root directory");
                        }
                    }
                } else {
                    match Node::get_entry(&current_dir, arg) {
                        Some(node) => {
                            let n = node.borrow();
                            match &*n {
                                Node::Directory { .. } => {
                                    unsafe {
                                        DIR_STACK.push(node.clone());
                                        CURRENT_DIR = Some(node.clone());
                                    }
                                }
                                _ => println_colored!(Color::Red, Color::Black, "{} is not a directory", arg),
                            }
                        }
                        None => println_colored!(Color::Red, Color::Black, "No such directory: {}", arg),
                    }
                }
            } else {
                println_colored!(Color::Green, Color::Black, "Usage: cd <dir>");
            }
        }
        "rm" => {
            if let Some(name) = parts.next() {
                match Node::get_entry(&current_dir, name) {
                    Some(node) => {
                        let n = node.borrow();
                        match &*n {
                            Node::File { .. } => {
                                if let Err(e) = Node::remove_entry(&current_dir, name) {
                                    println_colored!(Color::Red, Color::Black, "Error removing file: {}", e);
                                }
                            }
                            Node::Directory { .. } => {
                                print_colored!(Color::Red, Color::Black, "{} is a directory,", name);
                                print!("use ");
                                print_colored!(Color::Green, Color::Black, "rmdir ");
                                println!("to remove directories");
                            }
                        }
                    }
                    None => println_colored!(Color::Red, Color::Black, "No such file: {}", name),
                }
            } else {
                println_colored!(Color::Green, Color::Black, "Usage: rm <file>");
            }
        }
        "rmdir" => {
            if let Some(name) = parts.next() {
                match Node::get_entry(&current_dir, name) {
                    Some(node) => {
                        let n = node.borrow();
                        match &*n {
                            Node::Directory { entries } => {
                                if entries.is_empty() {
                                    if let Err(e) = Node::remove_entry(&current_dir, name) {
                                        println_colored!(Color::Red, Color::Black, "Error removing directory: {}", e);
                                    }
                                } else {
                                    println_colored!(Color::Red, Color::Black, "Directory is not empty");
                                }
                            }
                            Node::File { .. } => {
                                print_colored!(Color::Red, Color::Black, "{} is a file,", name);
                                print!("use ");
                                print_colored!(Color::Green, Color::Black, "rm ");
                                println!("to remove files");
                            }
                        }
                    }
                    None => println_colored!(Color::Red, Color::Black, "No such directory: {}", name),
                }
            } else {
                println_colored!(Color::Green, Color::Black, "Usage: rmdir <dir>");
            }
        }
        "write" => {
            if let Some(name) = parts.next() {
                let data: String = parts.collect::<Vec<_>>().join(" ");
                if data.is_empty() {
                    println_colored!(Color::Green, Color::Black, "Usage: write <file> <text>");
                    return;
                }
                match Node::get_entry(&current_dir, name) {
                    Some(node) => {
                        if let Err(e) = Node::write_file(&node, data.as_bytes()) {
                            println_colored!(Color::Red, Color::Black, "Error writing to file: {}", e);
                        }
                    }
                    None => println_colored!(Color::Red, Color::Black, "No such file: {}", name),
                }
            } else {
                println_colored!(Color::Green, Color::Black, "Usage: write <file> <text>");
            }
        }
        "open" => {
            if let Some(name) = parts.next() {
                match Node::get_entry(&current_dir, name) {
                    Some(node) => {
                        let n = node.borrow();
                        match &*n {
                            Node::File { .. } => {
                                match Node::read_file(&node) {
                                    Ok(content) => {
                                        if let Ok(text) = core::str::from_utf8(&content) {
                                            println!("{}", text);
                                        } else {
                                            println!("<binary data>");
                                        }
                                    }
                                    Err(e) => println_colored!(Color::Red, Color::Black, "Error reading file: {}", e),
                                }
                            }
                            Node::Directory { .. } => println_colored!(Color::Red, Color::Black, "{} is a directory", name),
                        }
                    }
                    None => println_colored!(Color::Red, Color::Black, "No such file: {}", name),
                }
            } else {
                println_colored!(Color::Green, Color::Black, "Usage: open <file>");
            }
        }
        "hi" | "hello" | "hi!" | "hello!" => {
            println_colored!(Color::Yellow, Color::Black, "Hi broooooooo!");
            println_colored!(Color::Yellow, Color::Black, "You nice, good luck!!!");
        },
        _ => {
            println_colored!(Color::Red, Color::Black,"Unknown command: {}", command);
            print!("Use "); 
            print_colored!(Color::Green, Color::Black, "help");
            println!(" to see available commands");
        }
    }
}

unsafe fn outw(port: u16, value: u16) {
    asm!(
        "out dx, ax",
        in("dx") port,
        in("ax") value,
        options(nomem, nostack, preserves_flags),
    );
}

unsafe fn outb(port: u16, value: u8) {
    asm!(
        "out dx, al",
        in("dx") port,
        in("al") value,
        options(nomem, nostack, preserves_flags),
    );
}

unsafe fn inb(port: u16) -> u8 {
    let result: u8;
    asm!(
        "in al, dx",
        in("dx") port,
        out("al") result,
        options(nomem, nostack, preserves_flags),
    );
    result
}

fn get_path(dir_stack: &Vec<NodeRef>) -> String {
    if dir_stack.is_empty() {
        return "/".to_string();
    }

    let mut parts = Vec::new();

    for i in 1..dir_stack.len() {
        let current = &dir_stack[i];
        let parent = &dir_stack[i - 1];

        let parent_borrow = parent.borrow();
        match &*parent_borrow {
            Node::Directory { entries } => {
                let name = entries.iter()
                    .find_map(|(name, node_ref)| {
                        if Rc::ptr_eq(node_ref, current) {
                            Some(name.clone())
                        } else {
                            None
                        }
                    });
                if let Some(name) = name {
                    parts.push(name);
                } else {
                    parts.push("<unknown>".to_string());
                }
            }
            _ => {
                parts.push("<not a directory>".to_string());
            }
        }
    }
    format!("/{}", parts.join("/"))
}