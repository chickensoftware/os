use alloc::format;
use alloc::string::String;

use uefi::prelude::BootServices;
use uefi::proto::console::gop::{GraphicsOutput, PixelFormat};

use chicken_util::graphics::framebuffer::FrameBufferMetadata;

/// Initialize framebuffer (GOP)
pub(super) fn initialize_framebuffer(boot_services: &BootServices) -> Result<FrameBufferMetadata, String> {
    let gop_handle = boot_services.get_handle_for_protocol::<GraphicsOutput>().map_err(|error| format!("Could not get handle for GOP: {error}."))?;

    let mut gop = boot_services.open_protocol_exclusive::<GraphicsOutput>(gop_handle).map_err(|error| format!("Could not open GOP: {error}."))?;
    let mut raw_frame_buffer = gop.frame_buffer();
    let base = raw_frame_buffer.as_mut_ptr() as u64;
    let size = raw_frame_buffer.size();
    let info = gop.current_mode_info();

    let is_rgb = match info.pixel_format() {
        PixelFormat::Rgb => Ok(true),
        PixelFormat::Bgr => Ok(false),
        PixelFormat::Bitmask | PixelFormat::BltOnly => Err("ChickenOS (for now) only supports RGB and BGR pixel formats!")
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
