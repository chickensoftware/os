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

### Kernel Base 
- [x] Global Descriptor Table
- [x] Interrupt Handling
- [ ] Complete ISR
- [ ] ACPI Tables
    - [x] RSDP
    - [x] RSDT/XSDT
    - [x] MADT
    - [ ] FADT
- [ ] APIC IO
- [ ] Timer
- [ ] Keyboard support

### Memory Management
- [x] Custom Memory Map
- [x] Physical Memory Manager
- [x] Paging
- [x] Global Page Table Manager
- [x] Virtual Memory Manager
- [x] Global Virtual Memory Manager 
- [x] Basic Kernel Heap Allocator 
    - [x] Bump Allocator
    - [x] Linked List Allocator
- [ ] Full-fetched Kernel Heap Allocator

### Video Output
- [x] Raw Framebuffer
- [ ] Full-fetched Framebuffer
- [x] Text & Fonts
- [x] Global Writer

### Scheduling
- [ ] Scheduler
- [ ] Processes
- [ ] Threads
- [x] Spin Lock

### Userspace
- [ ] Switching Modes
- [ ] Interrupt Handling in Userspace
- [ ] System Calls
- [ ] Userspace Heap Allocator

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
