use crate::base::io::keyboard::KeyboardType;

#[derive(Debug)]
pub struct Qwertz;

impl KeyboardType for Qwertz {
    const LEFT_SHIFT: u8 = 0x2A;
    const RIGHT_SHIFT: u8 = 0x36;
    const ENTER: u8 = 0x1C;

    const ASCII_TABLE: [char; 58] =
        ['\0', '\0', '1', '2', '3', '4', '5', '6', '7', '8',
         '9', '0', 'ß', '´', '\0', '\0', 'q', 'w', 'e', 'r',
         't', 'z', 'u', 'i', 'o', 'p', 'ü', '+', '\0', '\0',
         'a', 's', 'd', 'f', 'g', 'h', 'j', 'k', 'l', 'ö',
         'ä', '^', '\0', '#', 'y', 'x', 'c', 'v', 'b', 'n', 
         'm', ',', '.', '-', '\0', '*', '\0', ' '];

    fn translate(scancode: u8, uppercase: bool) -> char {
        let character = *(Self::ASCII_TABLE.get(scancode as usize).unwrap_or(&'\0'));
        if uppercase { character.to_ascii_uppercase() } else { character }
    }
}

