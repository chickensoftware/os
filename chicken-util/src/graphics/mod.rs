use core::fmt::Debug;

pub mod framebuffer;

#[derive(Copy, Clone, Debug, Default)]
pub struct Color {
    pub red: u8,
    pub green: u8,
    pub blue: u8,
}
macro_rules! color {
    ($color:ident, $red:expr, $green:expr, $blue:expr) => {
        impl Color {
            pub const fn $color() -> Color {
                Color {red: $red, green: $green, blue: $blue}
            }
        }
    };
}

color!(red, 0xFF, 0x00, 0x00);
color!(green, 0x00, 0xFF, 0x00);
color!(blue, 0x00, 0x00, 0xFF);
color!(grey, 0xC0, 0xC0, 0xC0);
color!(dark_grey, 0x1D, 0x1D, 0x1D);
color!(black, 0x00, 0x00, 0x00);
color!(white, 0xFF, 0xFF, 0xFF);
color!(yellow, 0xFF,0xFF,0x00);
