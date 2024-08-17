use core::{
    cell::OnceCell,
    fmt::{Debug, Write},
};

use chicken_util::graphics::{font::Font, Color};

use crate::{
    base::interrupts::without_interrupts,
    scheduling::spin::SpinLock,
    video::{framebuffer::RawFrameBuffer, VideoError},
};

pub static WRITER: SpinLock<OnceCell<Writer>> = SpinLock::new(OnceCell::new());

#[derive(Debug)]
pub(crate) struct Writer {
    row: usize,
    col: usize,
    foreground_color: Color,
    background_color: Color,
    framebuffer: RawFrameBuffer,
    font: Font,
}
impl Writer {
    pub(super) fn new(
        font: Font,
        framebuffer: RawFrameBuffer,
        foreground_color: Color,
        background_color: Color,
    ) -> Self {
        Writer {
            row: 0,
            col: 0,
            foreground_color,
            background_color,
            font,
            framebuffer,
        }
    }
}

impl Writer {
    pub(crate) fn write_char(&mut self, character: char) {
        let mut x = self.col;
        let mut y = self.row;

        match character {
            '\n' => {
                if (y + 1) * self.font.glyph_height() >= self.framebuffer.meta_data.height {
                    // looping terminal
                    self.framebuffer.fill(self.background_color);
                    y = 0;
                } else {
                    y += 1
                }
                x = 0;
            }
            character => {
                if x * self.font.glyph_width() >= self.framebuffer.meta_data.width {
                    if (y + 1) * self.font.glyph_height() >= self.framebuffer.meta_data.height {
                        // looping terminal
                        self.framebuffer.fill(self.background_color);
                        y = 0;
                    } else {
                        y += 1
                    }
                    x = 0;
                }

                if let Err(err) = self.framebuffer.draw_char(
                    character,
                    x * self.font.glyph_width(),
                    y * self.font.glyph_height(),
                    self.foreground_color,
                    self.background_color,
                    self.font,
                ) {
                    match err {
                        // should never happen
                        VideoError::CoordinatesOutOfBounds(_, _) => return,
                        // print ? instead
                        VideoError::UnsupportedCharacter => {
                            self.framebuffer
                                .draw_char(
                                    '?',
                                    x * self.font.glyph_width(),
                                    y * self.font.glyph_height(),
                                    self.foreground_color,
                                    self.background_color,
                                    self.font,
                                )
                                .unwrap();
                        }
                    }
                }
                x += 1;
            }
        }
        self.col = x;
        self.row = y;
    }

    fn _write_str(&mut self, s: &str) {
        for character in s.chars() {
            self.write_char(character);
        }
    }
}

impl Write for Writer {
    fn write_str(&mut self, s: &str) -> core::fmt::Result {
        self._write_str(s);
        Ok(())
    }
}

#[macro_export]
macro_rules! print {
    ($($arg:tt)*) => ($crate::video::text::_print(format_args!($($arg)*)));
}

#[macro_export]
macro_rules! println {
    () => ($crate::print!("\n"));
    ($($arg:tt)*) => ($crate::print!("{}\n", format_args!($($arg)*)));
}

#[doc(hidden)]
pub fn _print(args: core::fmt::Arguments) {
    without_interrupts(|| {
        if let Some(writer) = WRITER.lock().get_mut() {
            writer.write_fmt(args).unwrap();
        }
    })
}
