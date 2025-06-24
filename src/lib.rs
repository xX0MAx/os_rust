#![no_std]
#![feature(abi_x86_interrupt)]
#![feature(naked_functions)]
#![feature(asm_sym)]

pub mod interrupts;
pub mod vga_buffer;
pub mod gdt;
pub mod memory;
pub mod allocator;
pub mod syscalls;
pub mod shell;
pub mod ramfs;
extern crate alloc;

pub fn init() {
    gdt::init();

    interrupts::init_idt();

    unsafe { interrupts::PICS.lock().initialize() };

    unsafe { interrupts::PICS.lock().write_masks(0xFD, 0xFF) };

    syscalls::init_syscall();

    x86_64::instructions::interrupts::enable();
}

pub fn hlt_loop() -> ! {
    loop {
        x86_64::instructions::hlt();
    }
}