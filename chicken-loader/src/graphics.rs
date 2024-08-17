use alloc::{format, string::String};

use chicken_util::{
    graphics::{
        font::{PSF1Header, PSF2Header, PSFHeader, PSF1_MAGIC, PSF2_MAGIC},
        framebuffer::FrameBufferMetadata,
    },
    memory::PhysicalAddress,
};
use uefi::{
    prelude::BootServices,
    proto::console::gop::{GraphicsOutput, PixelFormat},
    Handle,
};

use crate::{file, memory::KERNEL_DATA, FONT_FILE_NAME};

/// Initialize framebuffer (GOP)
pub(super) fn initialize_framebuffer(
    boot_services: &BootServices,
) -> Result<FrameBufferMetadata, String> {
    let gop_handle = boot_services
        .get_handle_for_protocol::<GraphicsOutput>()
        .map_err(|error| format!("Could not get handle for GOP: {error}."))?;

    let mut gop = boot_services
        .open_protocol_exclusive::<GraphicsOutput>(gop_handle)
        .map_err(|error| format!("Could not open GOP: {error}."))?;
    let mut raw_frame_buffer = gop.frame_buffer();
    let base = raw_frame_buffer.as_mut_ptr() as u64;
    let size = raw_frame_buffer.size();
    let info = gop.current_mode_info();

    let is_rgb = match info.pixel_format() {
        PixelFormat::Rgb => Ok(true),
        PixelFormat::Bgr => Ok(false),
        PixelFormat::Bitmask | PixelFormat::BltOnly => {
            Err("ChickenOS (for now) only supports RGB and BGR pixel formats!")
        }
    }?;
    let (width, height) = info.resolution();
    let stride = info.stride();

    Ok(FrameBufferMetadata {
        base,
        size,
        width,
        height,
        stride,
        is_rgb,
    })
}
/// Load PSF2 font into memory. Returns font header, the address of the font in memory and the number of glyphs in the buffer.
pub(super) fn load_font(
    image_handle: Handle,
    bt: &BootServices,
) -> Result<(PSFHeader, PhysicalAddress, usize), String> {
    let font_data = file::get_file_data(image_handle, bt, FONT_FILE_NAME)?;
    let font_data_ptr = font_data.as_ptr(); // points to first byte of font data

    if font_data.len() < size_of::<PSF1Header>() {
        return Err("Insufficient font data for PSF1 header.".into());
    }

    let magic = unsafe { *(font_data_ptr as *const u16) };

    // check for psf1 header magic
    if magic == PSF1_MAGIC {
        let header = unsafe { *(font_data_ptr as *const PSF1Header) };
        let glyph_buffer_length = if header.font_mode == 1 { 512 } else { 256 };
        let glyph_buffer_size = glyph_buffer_length * header.character_size as usize;

        if font_data.len() < size_of::<PSF1Header>() + glyph_buffer_size {
            return Err("Insufficient font data for PSF1 font.".into());
        }

        // allocate memory for entire font data
        let total_size = size_of::<PSF1Header>() + glyph_buffer_size;
        let font_address = bt
            .allocate_pool(KERNEL_DATA, total_size)
            .map_err(|error| format!("Could not allocate pool for PSF1 font: {error}."))?
            .as_ptr() as u64;

        // copy header data to allocated memory
        unsafe {
            core::ptr::copy_nonoverlapping(
                font_data_ptr,
                font_address as *mut u8,
                size_of::<PSF1Header>(),
            );
        }

        let glyph_buffer_ptr = unsafe { (font_address as *mut u8).add(size_of::<PSF1Header>()) };

        // copy font data to allocated memory
        unsafe {
            core::ptr::copy_nonoverlapping(
                font_data_ptr.add(size_of::<PSF1Header>()),
                glyph_buffer_ptr,
                glyph_buffer_size,
            );
        }

        return Ok((
            PSFHeader::Version1(header),
            glyph_buffer_ptr as u64,
            glyph_buffer_length,
        ));
    } else {
        // check for psf2 header magic
        let magic = unsafe { *(font_data_ptr as *const u32) };
        if magic == PSF2_MAGIC {
            let header = unsafe { *(font_data_ptr as *const PSF2Header) };

            let glyph_buffer_size = (header.length * header.glyph_size) as usize;

            let header_size = size_of::<PSF2Header>();
            let total_size = header_size + glyph_buffer_size;

            if font_data.len() < total_size {
                return Err("Insufficient font data for PSF1 font.".into());
            }

            let font_address = bt
                .allocate_pool(KERNEL_DATA, total_size)
                .map_err(|error| format!("Could not allocate pool for PSF2 font: {error}."))?
                .as_ptr() as u64;

            // copy header data to allocated memory
            unsafe {
                core::ptr::copy_nonoverlapping(
                    font_data_ptr,
                    font_address as *mut u8,
                    size_of::<PSF2Header>(),
                );
            }

            let glyph_buffer_ptr = unsafe { (font_address as *mut u8).add(header_size) };
            // copy font data to allocated memory
            unsafe {
                core::ptr::copy_nonoverlapping(
                    font_data_ptr.add(header_size),
                    glyph_buffer_ptr,
                    glyph_buffer_size,
                );
            }

            return Ok((
                PSFHeader::Version2(header),
                glyph_buffer_ptr as u64,
                header.length as usize,
            ));
        }
    }
    Err(
        "Unrecognized PSF header. The magic signature does neither match version 1 nor version 2."
            .into(),
    )
}
