use crate::base::interrupts::CpuState;

pub(crate) mod pit;
// note: For now, only pit is supported; HPET, LAPIC may follow later.
pub(crate) trait Timer {
    const BASE_FREQUENCY: u64;

    /// Increment tick counter.
    fn tick();

    /// Current uptime since enabling interrupts in ms.
    fn current_uptime_ms(&self) -> u64;

    /// Called when timer interrupt occurs.
    fn perform_context_switch(&self, context: *const CpuState) -> *const CpuState;

    /// Set frequency of timer. Also enables the timer, if it hasn't been enabled already.
    ///
    /// # Safety
    /// Requires IO privileges and caller must ensure that frequency is valid.
    unsafe fn set_frequency(&mut self, frequency: u64);

    /// Get frequency of timer.
    fn frequency(&self) -> u64;
}
