# ChickenOS

ChickenOS is a lightweight hobby operating system for x86-64 developed in Rust.

## Features

- **Higher Half Kernel**: Built from scratch with Rust, providing a safe and modern approach to systems programming.
- **Bootloader**: ChickenOS includes its own bootloader, designed to initialize the system and hand control over to the ChickenOS kernel.

## Building and Running ChickenOS

ChickenOS can be built and run using the provided `Makefile`.

### Prerequisites

Ensure you have the following tools installed:

- Rust (nightly toolchain)
- `qemu` (for running the OS in a virtualized environment)
- `OVMF` (for UEFI support in QEMU)
- `parted`, `mkfs.fat` (for preparing a USB drive)

### Building ChickenOS

#### Building & running in QEMU
```bash
make run release=true
```

#### Building & running on real hardware
```bash
make usb USB_DEVICE=/dev/<device> release=true
```

## Progress Overview

### Kernel Entry 
- [x] Higher Half Kernel Entry 
- [x] Basic Bootloader

### Kernel Architecture 
- [ ] Global Descriptor Table
- [ ] Interrupt Handling
- [ ] ACPI Tables
- [ ] APIC IO
- [ ] Timer
- [ ] Keyboard support

### Memory Management
- [ ] Physical Memory Manager
- [ ] Paging
- [ ] Virtual Memory Manager
- [ ] Heap Allocator

### Video Output
- [ ] Framebuffer
- [ ] Text & Fonts

### Scheduling
- [ ] Scheduler
- [ ] Processes
- [ ] Threads
- [ ] Locks

### Userspace
- [ ] Switching Modes
- [ ] Interrupt Handling in Userspace
- [ ] System Calls

### Inter-Process Communication
- [ ] Shared Memory
- [ ] Message Passing

### Virtual Filesystem
- [ ] Virtual Filesystem
- [ ] TAR Filesystem
- [ ] Loading ELFs

```plaintext
   \\
   (o>
\\_//)
 \_/_)
   _|_
```
