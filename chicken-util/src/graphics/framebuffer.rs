use core::{
    fmt,
    fmt::{Debug, Formatter},
};

pub const BPP: usize = 4; // bytes per pixel = pixel_stride

#[derive(Copy, Clone)]
pub struct FrameBufferMetadata {
    pub base: u64,
    pub size: usize,
    pub width: usize,
    pub height: usize,
    pub stride: usize, // pixels per scanline
    pub is_rgb: bool,  // RGB | BGR => for now only supports these pixel formats
}

impl Debug for FrameBufferMetadata {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_fmt(format_args!(
            "FrameBufferMetadata {{\n\tbase: {:#x},\n\tsize: {:#x},\n\twidth: {},\n\theight: {},\n\tstride: {},\n}}",
            self.base, self.size, self.width, self.height, self.stride
        ))
    }
}
