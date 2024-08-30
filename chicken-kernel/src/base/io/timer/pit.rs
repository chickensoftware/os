use crate::{
    base::{
        interrupts::CpuState,
        io::{io_wait, outb, Port, timer::Timer},
    }
    ,
    scheduling::{SCHEDULER, spin::SpinLock},
};

const TICK_GENERATOR_PORT: Port = 0x40;
const PIT_PORT: Port = 0x43;

pub(in crate::base) static PIT: SpinLock<ProgrammableIntervalTimer> =
    SpinLock::new(ProgrammableIntervalTimer::new());

#[derive(Debug)]
pub(in crate::base) struct ProgrammableIntervalTimer {
    divisor: u16,
}

impl ProgrammableIntervalTimer {
    pub(in crate::base) const MAX_DIVISOR: u16 = 65535;

    const fn new() -> Self {
        Self {
            divisor: Self::MAX_DIVISOR,
        }
    }
    /// Set divisor of PIT. Also enables it, if it hasn't been enabled already.
    ///
    /// # Safety
    /// Requires IO privileges.
    unsafe fn set_divisor(&mut self, mut divisor: u16) {
        if divisor < 100 {
            divisor = 100;
        }

        self.divisor = divisor;

        // set mode 2 (rate generator)
        outb(PIT_PORT, 0b00110100);
        io_wait();
        // send lower half of divisor
        outb(TICK_GENERATOR_PORT, (self.divisor & 0x00ff) as u8);
        io_wait();
        // send higher half of divisor
        outb(TICK_GENERATOR_PORT, ((self.divisor & 0xff00) >> 8) as u8);
        io_wait();
    }
}

impl Timer for ProgrammableIntervalTimer {
    const BASE_FREQUENCY: u64 = 1193182;

    fn perform_context_switch(&self, context: *const CpuState) -> *const CpuState {
        let mut binding = SCHEDULER.lock();
        if let Some(scheduler) = binding.get_mut() {
            scheduler.schedule(context)
        } else {
            context
        }
    }

    unsafe fn set_frequency(&mut self, frequency: u64) {
        if frequency != 0 {
            self.set_divisor((ProgrammableIntervalTimer::BASE_FREQUENCY / frequency) as u16);
        }
    }

    fn frequency(&self) -> u64 {
        ProgrammableIntervalTimer::BASE_FREQUENCY / self.divisor as u64
    }
}
