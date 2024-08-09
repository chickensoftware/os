OUTPUT_FORMAT(elf64-x86-64)
ENTRY(_start)

PHDRS
{
    /* flags bits: 0 = execute, 1 = write, 2 = read */
     text   PT_LOAD FLAGS((1 << 0) | (1 << 2));
     rodata PT_LOAD FLAGS((1 << 2));
     data   PT_LOAD FLAGS((1 << 1) | (1 << 2));
}

SECTIONS
{
    KERNEL_VIRTUAL_OFFSET = 0xFFFFFFFF80000000;
    KERNEL_PHYSICAL_BASE = 0x100000;
    KERNEL_VIRTUAL_BASE = KERNEL_VIRTUAL_OFFSET + KERNEL_PHYSICAL_BASE;

    . = KERNEL_VIRTUAL_BASE;

    .text ALIGN(0x1000): AT (. - KERNEL_VIRTUAL_OFFSET)
    {
        *(.text*)
    } :text

    .data ALIGN(0x1000): AT (. - KERNEL_VIRTUAL_OFFSET)
    {
        *(.data*)
    } :data
    .rodata ALIGN(0x1000): AT (. - KERNEL_VIRTUAL_OFFSET)
    {
        *(.rodata*)
    } :rodata
    .bss ALIGN(0x1000): AT (. - KERNEL_VIRTUAL_OFFSET)
    {
        *(COMMON)
        *(.bss*)
    } :data

    KERNEL_VIRTUAL_END = .;
}