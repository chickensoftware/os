use core::marker::PhantomData;

use crate::{base::io::keyboard::qwertz::Qwertz, print, println, scheduling::spin::SpinLock};

mod qwertz;

pub(in crate::base) static KEYBOARD: SpinLock<Keyboard<Qwertz>> = SpinLock::new(Keyboard::new());

macro_rules! handle_scancode {
    ($self:ident, $scancode:ident, $type:ty, $default_action:expr, $($key:expr => $action:stmt), *) => {
        // specific action for specific key
        $(
            if $scancode == $key {
                $action
                return;
            }
        )*
        // default action
        {
            let ascii = <$type>::translate($scancode, $self.is_left_shift || $self.is_right_shift);
            $default_action(ascii);
        }
    }
}

#[derive(Debug)]
pub(in crate::base) struct Keyboard<T>
where
    T: KeyboardType,
{
    is_left_shift: bool,
    is_right_shift: bool,
    _marker: PhantomData<T>,
}

impl<T> Keyboard<T>
where
    T: KeyboardType,
{
    const fn new() -> Self {
        Self {
            is_left_shift: false,
            is_right_shift: false,
            _marker: PhantomData,
        }
    }

    pub(in crate::base) fn handle(&mut self, scancode: u8) {
        handle_scancode!(self, scancode, T,
            |ascii| {
                if ascii != '\0' {
                    print!("{}", ascii)
                }
            },
            T::LEFT_SHIFT => { self.is_left_shift = true; },
            T::LEFT_SHIFT + 0x80 => { self.is_left_shift = false; },
            T::RIGHT_SHIFT => { self.is_right_shift = true; },
            T::RIGHT_SHIFT + 0x80 => { self.is_right_shift = false; },
            T::ENTER => println!()
        );
    }
}

pub(in crate::base) trait KeyboardType {
    const LEFT_SHIFT: u8;
    const RIGHT_SHIFT: u8;

    const ENTER: u8;

    const ASCII_TABLE: [char; 58];

    fn translate(scancode: u8, uppercase: bool) -> char;
}
