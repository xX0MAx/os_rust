#![no_std]
#![no_main]

extern crate alloc;

use core::panic::PanicInfo;
use test_os::{
    println, 
    print, 
    println_colored, 
    print_colored, 
    vga_buffer::Color, 
    shell::shell_loop, 
    ramfs::Node,
};
use bootloader::{BootInfo, entry_point};

entry_point!(kernel_main);

#[unsafe(no_mangle)]
fn kernel_main(boot_info: &'static BootInfo) -> ! {
    use test_os::memory::{self, BootInfoFrameAllocator};
    use x86_64::{VirtAddr};
    use test_os::allocator;
    
    test_os::init();
    let phys_mem_offset = VirtAddr::new(boot_info.physical_memory_offset);
    let mut mapper = unsafe { memory::init(phys_mem_offset) };
    let mut frame_allocator = unsafe { BootInfoFrameAllocator::init(&boot_info.memory_map) };

    allocator::init_heap(&mut mapper, &mut frame_allocator).expect("heap initialization failed");

    Node::init_fs();

    println_colored!(Color::LightCyan, Color::Black, "\n        Hello!");
    print!("    It's test OS on Rust by ");
    print_colored!(Color::Magenta, Color::Black,"xX0MAx");
    println!(".");
    print!("    Write ");
    print_colored!(Color::Green, Color::Black,"help");
    println!(" to see available commands.");

    shell_loop();
    test_os::hlt_loop();
}

#[cfg(not(test))]
#[panic_handler]
fn panic(info: &PanicInfo) -> !{
	println!("{}", info);
    test_os::hlt_loop();
}