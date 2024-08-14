bits 64

extern kernel_main ; rust main

; kernel entry setup

section .text
    global _start
    _start:
      call kernel_main

    .fin:
      hlt
      jmp .fin

    ; additional assembly functions

    ; GDT setup
    global load_gdt

    load_gdt:
        lgdt [rdi]
        mov ax, 0x10
        mov ds, ax
        mov es, ax
        mov fs, ax
        mov gs, ax
        mov ss, ax
        pop rdi
        mov rax, 0x08
        push rax
        push rdi
        retfq