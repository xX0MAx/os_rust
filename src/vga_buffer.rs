use core::fmt;
use spin::Mutex;
use lazy_static::lazy_static;
use alloc::vec::Vec;

#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    Black = 0,
    Blue = 1,
    Green = 2,
    Cyan = 3,
    Red = 4,
    Magenta = 5,
    Brown = 6,
    LightGray = 7,
    DarkGray = 8,
    LightBlue = 9,
    LightGreen = 10,
    LightCyan = 11,
    LightRed = 12,
    Pink = 13,
    Yellow = 14,
    White = 15,
}

const BUFFER_HEIGHT: usize = 25;
const BUFFER_WIDTH: usize = 80;
const INPUT_BUFFER_SIZE: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
struct ColorCode(u8);

impl ColorCode {
    fn new(foreground: Color, background: Color) -> ColorCode {
        ColorCode((background as u8) << 4 | (foreground as u8))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
struct ScreenChar {
    ascii_character: u8,
    color_code: ColorCode,
}

#[repr(transparent)]
struct Buffer {
    chars: [[ScreenChar; BUFFER_WIDTH]; BUFFER_HEIGHT],
}

pub struct InputBuffer {
    buf: [u8; INPUT_BUFFER_SIZE],
    len: usize,
}

pub struct UpBuffer {
    lines: Vec<[ScreenChar; BUFFER_WIDTH]>,
    max_lines: usize,
}

pub struct DownBuffer {
    lines: Vec<[ScreenChar; BUFFER_WIDTH]>,
    max_lines: usize,
}

impl UpBuffer {
    pub fn new(max_history: usize) -> Self {
        Self {
            lines: Vec::with_capacity(max_history),
            max_lines: max_history,
        }
    }
    
    pub fn push_line(&mut self, line: [ScreenChar; BUFFER_WIDTH]) {
        if self.lines.len() >= self.max_lines {
            self.lines.remove(0);
        }
        self.lines.push(line);
    }
    
    pub fn pop_line(&mut self) -> Option<[ScreenChar; BUFFER_WIDTH]> {
        self.lines.pop()
    }
    
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }
}

impl DownBuffer {
    pub fn new(max_history: usize) -> Self {
        Self {
            lines: Vec::with_capacity(max_history),
            max_lines: max_history,
        }
    }
    
    pub fn push_line(&mut self, line: [ScreenChar; BUFFER_WIDTH]) {
        if self.lines.len() >= self.max_lines {
            self.lines.remove(0);
        }
        self.lines.push(line);
    }
    
    pub fn pop_line(&mut self) -> Option<[ScreenChar; BUFFER_WIDTH]> {
        self.lines.pop()
    }
    
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }
}

impl InputBuffer {
    pub const fn new() -> Self {
        Self {
            buf: [0; INPUT_BUFFER_SIZE],
            len: 0,
        }
    }

    pub fn push_byte(&mut self, byte: u8) {
        if self.len < INPUT_BUFFER_SIZE {
            self.buf[self.len] = byte;
            self.len += 1;
        }
    }

    pub fn clear(&mut self) {
        self.len = 0;
    }

    pub fn as_str(&self) -> &str {
        core::str::from_utf8(&self.buf[..self.len]).unwrap_or("")
    }
}

pub struct Writer {
    column_position: usize,
    row_position: usize,
    color_code: ColorCode,
    buffer: &'static mut Buffer,
    buffer_string: InputBuffer,
    up_buffer: UpBuffer,
    down_buffer: DownBuffer,
    write_row: isize,
}

impl Writer {
    pub fn set_color(&mut self, foreground: Color, background: Color) {
        self.color_code = ColorCode::new(foreground, background);
    }
}

impl Writer {
    pub fn new() -> Self {
        Writer {
            column_position: 0,
            row_position: 0,
            color_code: ColorCode::new(Color::White, Color::Black),
            buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
            buffer_string: InputBuffer::new(),
            up_buffer: UpBuffer::new(100),
            down_buffer: DownBuffer::new(100),
            write_row: 0,
        }
    }

    pub fn plus_write_row(&mut self){
        self.write_row += 1;
    }

    pub fn minus_write_row(&mut self){
        self.write_row -= 1;
    }

    pub fn check_write_row(&mut self){
        if (self.write_row <= 1 || self.write_row as usize == usize::MAX){
            while self.write_row <= 1 {
                self.scroll_up();
                self.write_row += 1;
            }
        }
        if self.write_row as usize >= BUFFER_HEIGHT{
            while self.write_row as usize >= BUFFER_HEIGHT {
            self.scroll_down();
            self.write_row -= 1;
            }
        }
    }

    pub fn write_byte(&mut self, byte: u8) {
        match byte {
            b'\n' => {
                self.check_write_row();
                self.new_line();
                self.buffer_string.push_byte(b'\n');
            }
            0x20..=0x7e => {
                self.check_write_row();
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }
                let row = self.write_row as usize;
                let col = self.column_position;
                let color_code = self.color_code;
                self.buffer.chars[row][col] = ScreenChar {
                    ascii_character: byte,
                    color_code,
                };
                self.column_position += 1;
                self.buffer_string.push_byte(byte);
            }
            _ => {
                self.check_write_row();
                if self.column_position >= BUFFER_WIDTH {
                    self.new_line();
                }
                let row = self.write_row as usize;
                let col = self.column_position;
                let color_code = self.color_code;
                self.buffer.chars[row][col] = ScreenChar {
                    ascii_character: 0xfe,
                    color_code,
                };
                self.column_position += 1;
                self.buffer_string.push_byte(0xfe);
            }
        }
    }

    pub fn scroll_up(&mut self) {
        self.up_buffer.push_line(self.buffer.chars[BUFFER_HEIGHT-1]);
        for row in (0..BUFFER_HEIGHT-1).rev() {
            for col in 0..BUFFER_WIDTH {
                self.buffer.chars[row + 1][col] = self.buffer.chars[row][col];
            }
        }
    
        self.clear_row(0);
    
        if self.row_position < BUFFER_HEIGHT - 1 {
            self.row_position += 1;
        }
        if let Some(line) = self.down_buffer.pop_line() {
            self.buffer.chars[0] = line;
        } else {
            self.clear_row(0);
        }
    }

    pub fn scroll_down(&mut self){
        self.down_buffer.push_line(self.buffer.chars[0]);
        for row in 1..BUFFER_HEIGHT {
            for col in 0..BUFFER_WIDTH {
                self.buffer.chars[row - 1][col] = self.buffer.chars[row][col];
             }
        }    

        self.clear_row(BUFFER_HEIGHT - 1);

        self.row_position = BUFFER_HEIGHT - 1;
        if let Some(line) = self.up_buffer.pop_line() {
            self.buffer.chars[BUFFER_HEIGHT - 1] = line;
        } else {
            self.clear_row(BUFFER_HEIGHT - 1);
        }
    }

    pub fn new_line(&mut self) {
        self.down_buffer.push_line(self.buffer.chars[0]);
        self.column_position = 0;
        self.write_row += 1;

        if self.write_row as usize >= BUFFER_HEIGHT {
            for row in 1..BUFFER_HEIGHT {
                for col in 0..BUFFER_WIDTH {
                    self.buffer.chars[row - 1][col] = self.buffer.chars[row][col];
                }
            }
            self.clear_row(BUFFER_HEIGHT - 1);
            self.write_row = (BUFFER_HEIGHT - 1) as isize;
        }
    }

    fn clear_row(&mut self, row: usize) {
        let blank = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };
        for col in 0..BUFFER_WIDTH {
            self.buffer.chars[row][col] = blank;
        }
    }

    pub fn write_string(&mut self, s: &str) {
        for byte in s.bytes() {
            match byte {
                0x20..=0x7e | b'\n' => self.write_byte(byte),
                _ => self.write_byte(0xfe),
            }
        }
    }

    pub fn clear_screen(&mut self) {
        self.check_write_row();
        for row in 0..BUFFER_HEIGHT {
            self.clear_row(row);
        }
        self.column_position = 0;
        self.row_position = 0;
        self.buffer_string.clear();
        self.up_buffer.clear();
        self.down_buffer.clear();
        self.write_row = 1;
    }

    pub fn backspace(&mut self) {
        self.check_write_row();
        if self.column_position == 0 && self.write_row == 0 {
            return;
        }

        if self.column_position == 0 {
            self.write_row -= 1;
            self.column_position = BUFFER_WIDTH - 1;
        } else {
            self.column_position -= 1;
        }

        self.buffer.chars[self.write_row as usize][self.column_position] = ScreenChar {
            ascii_character: b' ',
            color_code: self.color_code,
        };

        if self.buffer_string.len > 0 {
            self.buffer_string.len -= 1;
        }
    }

    pub fn get_buffer(&self) -> &str {
        self.buffer_string.as_str()
    }
}

pub fn buffer_copy(dest: &mut [u8]) -> usize {
    let writer = WRITER.lock();
    let buf = writer.get_buffer().as_bytes();
    let len = core::cmp::min(dest.len(), buf.len());
    dest[..len].copy_from_slice(&buf[..len]);
    len
}

pub fn buffer_clear() {
    let mut writer = WRITER.lock();
    writer.buffer_string.clear();
}

impl fmt::Write for Writer {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.write_string(s);
        Ok(())
    }
}

lazy_static! {
    pub static ref WRITER: Mutex<Writer> = Mutex::new(Writer {
        column_position: 0,
        row_position: 0,
        color_code: ColorCode::new(Color::White, Color::Black),
        buffer: unsafe { &mut *(0xb8000 as *mut Buffer) },
        buffer_string: InputBuffer::new(),
        up_buffer: UpBuffer::new(100),
        down_buffer: DownBuffer::new(100),
        write_row: 0,
    });
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::vga_buffer::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: fmt::Arguments) {
    use core::fmt::Write;
    use x86_64::instructions::interrupts;
    interrupts::without_interrupts(|| {
        WRITER.lock().write_fmt(args).unwrap();
    });
}

#[doc(hidden)]
pub fn print_colored(args: fmt::Arguments, fg: Color, bg: Color) {
    use x86_64::instructions::interrupts;
    use core::fmt::Write;
    interrupts::without_interrupts(|| {
        let mut writer = WRITER.lock();
        writer.set_color(fg, bg);
        writer.write_fmt(args).unwrap();
        writer.set_color(Color::White, Color::Black);
    });
}

#[macro_export]
macro_rules! print_colored {
    ($fg:expr, $bg:expr, $($arg:tt)*) => ($crate::vga_buffer::print_colored(format_args!($($arg)*),$fg, $bg));
}
#[macro_export]
macro_rules! println_colored {
    () => ($crate::print_colored!(fg, bg, "\n"));
    ($fg:expr, $bg:expr, $($arg:tt)*) => ($crate::print_colored!($fg, $bg, "{}\n", format_args!($($arg)*)));
}