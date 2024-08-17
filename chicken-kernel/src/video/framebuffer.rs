use core::{fmt::Debug, ptr::write_volatile};

use chicken_util::graphics::{
    font::Font,
    framebuffer::{FrameBufferMetadata, BPP},
    Color,
};

use crate::video::VideoError;

/// Directly accesses video memory in order to display graphics
#[derive(Clone, Debug)]
pub(crate) struct RawFrameBuffer {
    pub(in crate::video) meta_data: FrameBufferMetadata,
}

impl RawFrameBuffer {
    /// Draws a pixel onto the screen at coordinates x,y and with the specified color. Returns, whether the action succeeds or the coordinates are invalid.
    pub(in crate::video) fn draw_pixel(
        &self,
        x: usize,
        y: usize,
        color: Color,
    ) -> Result<(), VideoError> {
        if !self.in_bounds(x, y) {
            return Err(VideoError::CoordinatesOutOfBounds(x, y));
        }

        let pitch = self.meta_data.stride * BPP;

        unsafe {
            let pixel = (self.meta_data.base as *mut u8).add(pitch * y + BPP * x);

            if self.meta_data.is_rgb {
                write_volatile(pixel, color.red); // Red
                write_volatile(pixel.add(1), color.green); // Green
                write_volatile(pixel.add(2), color.blue); // Blue
            } else {
                write_volatile(pixel, color.blue); // Blue
                write_volatile(pixel.add(1), color.green); // Green
                write_volatile(pixel.add(2), color.red); // Red
            }
        }

        Ok(())
    }
    /// Fills entire display with certain color
    pub(in crate::video) fn fill(&self, color: Color) {
        for x in 0..self.meta_data.width {
            for y in 0..self.meta_data.height {
                self.draw_pixel(x, y, color).unwrap();
            }
        }
    }
}

impl RawFrameBuffer {
    pub(in crate::video) fn draw_char(
        &self,
        character: char,
        x_offset: usize,
        y_offset: usize,
        foreground_color: Color,
        background_color: Color,
        font: Font,
    ) -> Result<(), VideoError> {
        if character as usize >= font.glyphs().len() {
            return Err(VideoError::UnsupportedCharacter);
        }

        let character_offset = character as usize * font.glyph_bytes();
        let character_ptr = unsafe { font.glyph_buffer_address.add(character_offset) };

        let glyph_height = font.glyph_height();
        let glyph_width = font.glyph_width();

        for y in 0..glyph_height {
            for x in 0..glyph_width {
                let byte_index = (y * glyph_width + x) / 8;
                let bit_index = 7 - ((y * glyph_width + x) % 8);

                let byte = unsafe { *character_ptr.add(byte_index) };
                let color = if (byte & (1 << bit_index)) != 0 {
                    foreground_color
                } else {
                    background_color
                };

                self.draw_pixel(x + x_offset, y + y_offset, color)?;
            }
        }

        Ok(())
    }
}

impl RawFrameBuffer {
    /// Whether a point is within the framebuffer vram
    fn in_bounds(&self, x: usize, y: usize) -> bool {
        x < self.meta_data.width && y < self.meta_data.height
    }
}

impl From<FrameBufferMetadata> for RawFrameBuffer {
    fn from(value: FrameBufferMetadata) -> Self {
        Self { meta_data: value }
    }
}
