// note: for now just using qemu_println, will later be changed to custom implementation.

use core::{
    error::Error,
    fmt::{Debug, Display, Formatter},
};

use chicken_util::{graphics::Color, BootInfo};

use crate::{
    println,
    video::{
        framebuffer::RawFrameBuffer,
        text::{Writer, WRITER},
    },
};

pub(super) mod framebuffer;
pub mod text;

const FOREGROUND_COLOR: Color = Color::white();
const BACKGROUND_COLOR: Color = Color::black();

const CHICKEN_OS: &str = r#"
   _____ _     _      _               ____   _____
  / ____| |   (_)    | |             / __ \ / ____|
 | |    | |__  _  ___| | _____ _ __ | |  | | (___
 | |    | '_ \| |/ __| |/ / _ \ '_ \| |  | |\___ \
 | |____| | | | | (__|   <  __/ | | | |__| |____) |
  \_____|_| |_|_|\___|_|\_\___|_| |_|\____/|_____/
                                                   "#;

pub(super) fn set_up(boot_info: &BootInfo) {
    // initialize framebuffer
    let framebuffer = RawFrameBuffer::from(boot_info.framebuffer_metadata);
    framebuffer.fill(Color::black());

    // initialize global writer
    WRITER.lock().get_or_init(|| {
        Writer::new(
            boot_info.font,
            framebuffer,
            FOREGROUND_COLOR,
            BACKGROUND_COLOR,
        )
    });

    println!("{}", CHICKEN_OS);
}

#[derive(Copy, Clone)]
enum VideoError {
    CoordinatesOutOfBounds(usize, usize),
    UnsupportedCharacter,
}

impl Debug for VideoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        match self {
            VideoError::CoordinatesOutOfBounds(x, y) => write!(
                f,
                "Video Error: Coordinates out of bounds: x: {}, y: {}.",
                x, y
            ),
            VideoError::UnsupportedCharacter => write!(f, "Video Error: Unsupported character."),
        }
    }
}

impl Display for VideoError {
    fn fmt(&self, f: &mut Formatter<'_>) -> core::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl Error for VideoError {}
