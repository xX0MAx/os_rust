use lazy_static::lazy_static;
use pic8259::ChainedPics;
use x86_64::{
    structures::idt::{InterruptDescriptorTable, InterruptStackFrame, PageFaultErrorCode},
    instructions::port::Port,
};
use spin::Mutex;

use crate::{gdt, hlt_loop, println};
use crate::vga_buffer::WRITER;

pub const PIC_1_OFFSET: u8 = 32;
pub const PIC_2_OFFSET: u8 = PIC_1_OFFSET + 8;

pub static mut ENTER_PRESSED: bool = false;
pub static mut UP_PRESSED: bool = false;
pub static mut DOWN_PRESSED: bool = false;
pub static mut LEFT_PRESSED: bool = false;
pub static mut RIGHT_PRESSED: bool = false;

lazy_static! {
    static ref IDT: InterruptDescriptorTable = {
        let mut idt = InterruptDescriptorTable::new();
        idt.breakpoint.set_handler_fn(breakpoint_handler);
        unsafe {
            idt.double_fault.set_handler_fn(double_fault_handler)
                .set_stack_index(gdt::DOUBLE_FAULT_IST_INDEX);
            idt[InterruptIndex::Timer.as_usize()]
                .set_handler_fn(timer_interrupt_handler);
            idt[InterruptIndex::Keyboard.as_usize()]
                .set_handler_fn(keyboard_interrupt_handler);
            idt.page_fault
                .set_handler_fn(page_fault_handler);
            idt[InterruptIndex::Mouse.as_usize()]
                .set_handler_fn(mouse_interrupt_handler);
        }
        idt
    };
}

pub fn init_idt() {
    IDT.load();
}

extern "x86-interrupt" fn breakpoint_handler(
    stack_frame: InterruptStackFrame)
{
    println!("EXCEPTION: BREAKPOINT\n{:#?}", stack_frame);
}

extern "x86-interrupt" fn double_fault_handler(
    stack_frame: InterruptStackFrame, _error_code: u64) -> !
{
    panic!("EXCEPTION: DOUBLE FAULT\n{:#?}", stack_frame);
}

pub static PICS: spin::Mutex<ChainedPics> =
    spin::Mutex::new(unsafe { ChainedPics::new(PIC_1_OFFSET, PIC_2_OFFSET) });

#[derive(Debug, Clone, Copy)]
#[repr(u8)]
pub enum InterruptIndex {
    Timer = PIC_1_OFFSET,
    Keyboard,
    Mouse = PIC_2_OFFSET + 4,
}

impl InterruptIndex {
    fn as_u8(self) -> u8 {
        self as u8
    }

    fn as_usize(self) -> usize {
        usize::from(self.as_u8())
    }
}

extern "x86-interrupt" fn timer_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    //print!(".");

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Timer.as_u8());
    }
}

extern "x86-interrupt" fn keyboard_interrupt_handler(
    _stack_frame: InterruptStackFrame)
{
    use pc_keyboard::{layouts, DecodedKey, HandleControl, Keyboard, ScancodeSet1};
    use x86_64::instructions::port::Port;

    lazy_static! {
        static ref KEYBOARD: Mutex<Keyboard<layouts::Us104Key, ScancodeSet1>> =
            Mutex::new(Keyboard::new(ScancodeSet1::new(),
                layouts::Us104Key, HandleControl::Ignore)
            );
    }

    let mut keyboard = KEYBOARD.lock();
    let mut port = Port::new(0x60);
    let scancode: u8 = unsafe { port.read() };

    if let Ok(Some(key_event)) = keyboard.add_byte(scancode) {
        if let Some(key) = keyboard.process_keyevent(key_event) {
            match key {
                DecodedKey::Unicode(character) => {
                    let mut writer = WRITER.lock();
                    match character {
                        '\x08' => writer.backspace(),
                        '\n' | '\r' => {
                            writer.write_byte(b'\n');
                            unsafe{ENTER_PRESSED = true;}
                        }
                        c => {
                            writer.write_byte(c as u8);
                        }
                    }
                }
                DecodedKey::RawKey(key) => {
                    match key {
                        pc_keyboard::KeyCode::ArrowUp => unsafe {UP_PRESSED = true;},
                        pc_keyboard::KeyCode::ArrowDown => unsafe {DOWN_PRESSED = true;},
                        //pc_keyboard::KeyCode::ArrowLeft => unsafe {LEFT_PRESSED = true;},
                        //pc_keyboard::KeyCode::ArrowRight => unsafe {RIGHT_PRESSED = true;},
                        pc_keyboard::KeyCode::Backspace => {WRITER.lock().backspace();},
                        _ => {}
                    }
                }
            }
        }
    }

    unsafe {
        PICS.lock()
            .notify_end_of_interrupt(InterruptIndex::Keyboard.as_u8());
    }
}

extern "x86-interrupt" fn page_fault_handler(
    stack_frame: InterruptStackFrame,
    error_code: PageFaultErrorCode,
) {
    use x86_64::registers::control::Cr2;

    println!("EXCEPTION: PAGE FAULT");
    println!("Accessed Address: {:?}", Cr2::read());
    println!("Error Code: {:?}", error_code);
    println!("{:#?}", stack_frame);
    hlt_loop();
}

lazy_static! {
    static ref MOUSE_PACKET: Mutex<[u8; 4]> = Mutex::new([0; 4]);
    static ref MOUSE_PACKET_INDEX: Mutex<usize> = Mutex::new(0);
}

extern "x86-interrupt" fn mouse_interrupt_handler(_stack_frame: InterruptStackFrame) {
    let mut port = Port::new(0x60);
    let data: u8 = unsafe { port.read() };

    let mut packet = MOUSE_PACKET.lock();
    let mut index = MOUSE_PACKET_INDEX.lock();

    if *index == 0 {
        if data & 0x08 == 0 {
            *index = 0;
            unsafe {
                PICS.lock().notify_end_of_interrupt(InterruptIndex::Mouse.as_u8());
            }
            return;
        }
    }

    packet[*index] = data;
    *index += 1;

    if *index == 4 {
        /*
        let left = packet[0] & 0x1 != 0; press LMB
        let right = packet[0] & 0x2 != 0; press RMB
        let middle = packet[0] & 0x4 != 0; press Wheel
        */
        let wheel = packet[3] as i8;
        let mut writer = WRITER.lock();

        match wheel{
            -1 => {
                writer.scroll_up();
                writer.plus_write_row();
            }
            1 => {
                writer.scroll_down();
                writer.minus_write_row();
            }
            _ => {}
        }

        *index = 0;
    }

    unsafe {
        PICS.lock().notify_end_of_interrupt(InterruptIndex::Mouse.as_u8());
    }
}

pub fn init_mouse() {
    let mut command_port = Port::new(0x64);
    let mut data_port = Port::new(0x60);

    wait_input_buffer_empty();
    unsafe { command_port.write(0xA8u8); }

    wait_input_buffer_empty();
    unsafe { command_port.write(0x20u8); }
    wait_output_buffer_full();
    let status = unsafe { data_port.read() } | 0x02;
    wait_input_buffer_empty();
    unsafe { command_port.write(0x60u8); }
    wait_input_buffer_empty();
    unsafe { data_port.write(status); }

    wait_input_buffer_empty();
    unsafe { command_port.write(0xD4u8); }
    wait_input_buffer_empty();
    unsafe { data_port.write(0xF4u8); }
    wait_output_buffer_full();
    
    send_mouse_command(0xF4);

    send_mouse_command_with_data(0xF3, 200);
    send_mouse_command_with_data(0xF3, 100);
    send_mouse_command_with_data(0xF3, 80);

    send_mouse_command(0xF2);
    wait_output_buffer_full();
}

fn send_mouse_command(cmd: u8) {
    let mut command_port = Port::new(0x64);
    let mut data_port = Port::new(0x60);
    wait_input_buffer_empty();
    unsafe { command_port.write(0xD4u8); }
    wait_input_buffer_empty();
    unsafe { data_port.write(cmd); }
    wait_output_buffer_full();
    let ack = unsafe { data_port.read() };
}

fn send_mouse_command_with_data(cmd: u8, data: u8) {
    send_mouse_command(cmd);
    send_mouse_command(data);
}


fn wait_input_buffer_empty() {
    let mut status_port = Port::new(0x64);
    loop {
        let status: u8 = unsafe { status_port.read() };
        if status & 0x02 == 0 {
            break;
        }
    }
}

fn wait_output_buffer_full() {
    let mut status_port = Port::new(0x64);
    loop {
        let status: u8 = unsafe { status_port.read() };
        if status & 0x01 != 0 {
            break;
        }
    }
}