#![allow(dead_code)] // keeping all command constants for completeness, although, they are not all used


use crate::base::io::{inb, io_wait, outb, Port};
// ports:
// handled interrupt numbers 0 - 7:
// control information
const PIC_MASTER_COMMAND: Port = 0x20;
// data
const PIC_MASTER_DATA: Port = 0x21;

// handles interrupt numbers 8 - 15:
// control information
const PIC_SLAVE_COMMAND: Port = 0xA0;
// data
const PIC_SLAVE_DATA: Port = 0xA1;

// data:
// indicates that ICW4 will be present
const ICW1_ICW4: u8 = 0x01;
// initialization command
const ICW1_INIT: u8 = 0x10;
// 8086/88 (MCS-80/85) mode
const ICW4_8086: u8 = 0x01;

/// Remaps the pic outputs. The master chip to [`PIC_MASTER_DATA`] and the slave chip to [`PIC_SLAVE_DATA`].
///
/// # Safety
/// Needs IO privileges.
pub(super) unsafe fn remap() {

    // masks interrupts sent to the computer
    let bitmask_master = inb(PIC_MASTER_DATA);
    io_wait();
    let bitmask_slave = inb(PIC_SLAVE_DATA);
    io_wait();

    // initialize PIC master and slave chip
    outb(PIC_MASTER_COMMAND, ICW1_INIT | ICW1_ICW4);
    io_wait();
    outb(PIC_SLAVE_COMMAND, ICW1_INIT | ICW1_ICW4);
    io_wait();

    // set interrupt offsets to avoid collision with interrupt indices
    outb(PIC_MASTER_DATA, 0x20);
    io_wait();
    outb(PIC_SLAVE_DATA, 0x28);
    io_wait();

    // tell PIC master and slave how they correspond to each other
    outb(PIC_MASTER_DATA, 4);
    io_wait();
    outb(PIC_SLAVE_DATA, 2);
    io_wait();

    // set operation mode to 8086
    outb(PIC_MASTER_DATA, ICW4_8086);
    io_wait();
    outb(PIC_SLAVE_DATA, ICW4_8086);
    io_wait();

    // restore bitmasks
    outb(PIC_MASTER_DATA, bitmask_master);
    io_wait();
    outb(PIC_SLAVE_DATA, bitmask_slave);
    io_wait();
}
/// Disables all pic outputs.
///
/// # Safety
/// Needs IO privileges.
pub(super) unsafe fn disable() {
    // mask all interrupt ports
    outb(PIC_MASTER_DATA, 0xFF);
    outb(PIC_SLAVE_DATA, 0xFF);
}
