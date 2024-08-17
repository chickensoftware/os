use core::{
    fmt,
    fmt::{Debug, Formatter},
    slice,
};

pub const PSF1_MAGIC: u16 = 0x0436;
pub const PSF2_MAGIC: u32 = 0x864ab572;

#[derive(Copy, Clone, Debug)]
pub struct Font {
    /// Either PSF1 or PSF2 header
    pub header: PSFHeader,
    /// Glyph buffer pointer
    pub glyph_buffer_address: *const u8,
    /// Size of glyph buffer
    pub glyph_buffer_size: usize,
}

impl Font {
    pub fn glyphs(&self) -> &[u8] {
        unsafe { slice::from_raw_parts(self.glyph_buffer_address, self.glyph_buffer_size) }
    }

    pub fn glyph_bytes(&self) -> usize {
        match self.header {
            PSFHeader::Version1(header) => header.character_size as usize,
            PSFHeader::Version2(header) => header.glyph_size as usize,
        }
    }

    pub fn glyph_height(&self) -> usize {
        match self.header {
            PSFHeader::Version1(header) => header.character_size as usize,
            PSFHeader::Version2(header) => header.height as usize,
        }
    }

    pub fn glyph_width(&self) -> usize {
        match self.header {
            PSFHeader::Version1(_) => 8,
            PSFHeader::Version2(header) => header.width as usize,
        }
    }
}

unsafe impl Send for Font {}
unsafe impl Sync for Font {}

#[derive(Copy, Clone, Debug)]
pub enum PSFHeader {
    Version1(PSF1Header),
    Version2(PSF2Header),
}
#[repr(C)]
#[derive(Copy, Clone)]
pub struct PSF1Header {
    /// Magic number: 0x0436
    pub magic: u16,
    /// Font Mode: Whether font is a 256 or 512 glyph set
    pub font_mode: u8,
    /// Character Size: Glyph height
    pub character_size: u8,
}

impl Debug for PSF1Header {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "PSF1Header {{\n\tmagic: 0x{:x},\n\tfont_mode: {},\n\tcharacter_size: {},\n}}",
            self.magic, self.font_mode, self.character_size
        ))
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PSF2Header {
    /// Magic number: 0x864ab572
    pub magic: u32,
    /// Version: currently always 0
    pub version: u32,
    /// Header Size: Size of header in bytes (usually 32)
    pub header_size: u32,
    // Flags: Indicate unicode table (0 if there isn't one)
    pub flags: u32,
    /// Length: Number of glyphs
    pub length: u32,
    /// Glyph Size: Number of bytes per glyph
    pub glyph_size: u32,
    /// Height: Height of each glyph
    pub height: u32,
    /// Width: Width of each glyph
    pub width: u32,
}

impl Debug for PSF2Header {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "PSF2Header {{\n\tmagic: 0x{:x},\n\tversion: {},\n\theader_size: {},\n\tflags: {},\n\tlength: {},\n\tglyph_size: {},\n\theight: {},\n\twidth: {},\n}}",
            self.magic, self.version, self.header_size, self.flags, self.length, self.glyph_size, self.height, self.width
        ))
    }
}
