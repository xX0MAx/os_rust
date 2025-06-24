use core::arch::{asm, naked_asm};
use x86_64::registers::model_specific::Msr;

use crate::vga_buffer::{WRITER};

const IA32_LSTAR: u32 = 0xC000_0082;
const IA32_STAR: u32 = 0xC000_0081;
const IA32_FMASK: u32 = 0xC000_0084;

pub const SYSCALL_WRITE: u64 = 1;

pub unsafe fn syscall(syscall_number: u64, arg1: u64, arg2: u64, arg3: u64) -> u64 {
    let ret: u64;
    asm!(
        "syscall",
        in("rax") syscall_number,
        in("rdi") arg1,
        in("rsi") arg2,
        in("rdx") arg3,
        lateout("rax") ret,
    );
    ret
}

#[no_mangle]
pub extern "C" fn syscall_dispatcher(
    syscall_number: u64,
    arg1: u64,
    arg2: u64,
    arg3: u64,
    _arg4: u64,
    _arg5: u64,
    _arg6: u64,
) -> u64 {
    match syscall_number {
        SYSCALL_WRITE => sys_write(arg1, arg2 as *const u8, arg3 as usize),
        _ => 0,
    }
}

#[unsafe(naked)]
#[no_mangle]
pub extern "C" fn syscall_entry() -> ! {
        naked_asm!(
            "mov rcx, r10",
            "call {dispatcher}",
            "sysretq",
            dispatcher = sym syscall_dispatcher,
        );
    
}

pub fn sys_write(_fd: u64, buf: *const u8, len: usize) -> u64 {
    use core::slice;

    let slice = unsafe { slice::from_raw_parts(buf, len) };

    for &byte in slice {
        match byte {
            0x20..=0x7e | b'\n' => {
                if byte == b'\n' {
                    WRITER.lock().new_line();
                } else {
                    WRITER.lock().write_byte(byte);
                }
            }
            _ => WRITER.lock().write_byte(0xfe),
        }
    }
    len as u64
}

pub fn init_syscall() {
    unsafe {
        Msr::new(IA32_LSTAR).write(syscall_entry as u64);
        Msr::new(IA32_STAR).write(((0x08u64) << 32) | ((0x10u64) << 48));
        Msr::new(IA32_FMASK).write(0);
    }
}