bits 64

extern interrupt_dispatch

section .text
    ; IDT setup
    global load_idt

    load_idt:
        lidt [rdi]
        ret

    interrupt_stub:
        push rax
        push rbx
        push rcx
        push rdx
        push rsi
        push rdi
        push rbp
        push r8
        push r9
        push r10
        push r11
        push r12
        push r13
        push r14
        push r15

        mov rdi, rsp
        call interrupt_dispatch

        mov rsp, rax
        pop r15
        pop r14
        pop r13
        pop r12
        pop r11
        pop r10
        pop r9
        pop r8
        pop rbp
        pop rdi
        pop rsi
        pop rdx
        pop rcx
        pop rbx
        pop rax

        ; remove the vector number + error code
        add rsp, 16

        iretq

; todo: refactor interrupt handling

    ; predefined vector numbers: (0-21)
    global vector_0_handler

    ; divide error (has no error code)
    align 16
    vector_0_handler:
        ; vector 0 has no error code
        push 0
        ; vector number
        push 0
        jmp interrupt_stub

    ; align to the next 16-byte boundary
    ; debug exception (has no error code)
    align 16
    vector_1_handler:
        push 0
        ; vector number
        push 1
        jmp interrupt_stub

    ; non-maskable interrupt (has no error code)
    align 16
    vector_2_handler:
        push 0
        ; vector number
        push 2
        jmp interrupt_stub

    ; breakpoint (has no error code)
    align 16
    vector_3_handler:
        push 0
        ; vector number
        push 3
        jmp interrupt_stub

    ; overflow (has no error code)
    align 16
    vector_4_handler:
        push 0
        ; vector number
        push 4
        jmp interrupt_stub

    ; bound range exceeded (has no error code)
    align 16
    vector_5_handler:
        push 0
        ; vector number
        push 5
        jmp interrupt_stub

    ; invalid opcode (undefined opcode) (has no error code)
    align 16
    vector_6_handler:
        push 0
        ; vector number
        push 6
        jmp interrupt_stub

    ; device not available (no math processor) (has no error code)
    align 16
    vector_7_handler:
        push 0
        ; vector number
        push 7
        jmp interrupt_stub

    ; double fault (has error code (zero))
    align 16
        vector_8_handler:
        ; vector number
        push 8
        jmp interrupt_stub

    ; coprocessor segment overrun (reserved) (has no error code)
    align 16
    vector_9_handler:
        push 0
        ; vector number
        push 9
        jmp interrupt_stub

    ; invalid tss (has error code)
    align 16
    vector_10_handler:
        ; vector number
        push 10
        jmp interrupt_stub


    ; segment not present (has error code)
    align 16
    vector_11_handler:
        ; vector number
        push 11
        jmp interrupt_stub


    ; stack segment fault (has error code)
    align 16
    vector_12_handler:
        ; vector number
        push 12
        jmp interrupt_stub


    ; general protection fault (has error code)
    align 16
    vector_13_handler:
        ; vector number
        push 13
        jmp interrupt_stub


    ; page fault (has error code)
    align 16
    vector_14_handler:
        ; vector number
        push 14
        jmp interrupt_stub

    ; vector number 15 is reserved
    align 16
    vector_15_handler:
        push 0
        ; vector number
        push 15
        jmp interrupt_stub

    ; x87 fpu floating-point error (math fault) (has no error code)
    align 16
    vector_16_handler:
        push 0
        ; vector number
        push 16
        jmp interrupt_stub

    ; alignment check (has error code (zero))
    align 16
    vector_17_handler:
        ; vector number
        push 17
        jmp interrupt_stub


    ; machine check (has no error code)
    align 16
    vector_18_handler:
        push 0
        ; vector number
        push 18
        jmp interrupt_stub


    ; simd floating-point exception (has no error code)
    align 16
    vector_19_handler:
        push 0
        ; vector number
        push 19
        jmp interrupt_stub


    ; virtualization exception (has no error code)
    align 16
    vector_20_handler:
        push 0
        ; vector number
        push 20
        jmp interrupt_stub

    ; control protection exception (has error code)
    align 16
    vector_21_handler:
        ; vector number
        push 21
        jmp interrupt_stub

    ; vector numbers 22-31 are reserved
    align 16
    vector_22_handler:
        push 0
        ; vector number
        push 22
        jmp interrupt_stub

    align 16
    vector_23_handler:
        push 0
        ; vector number
        push 23
        jmp interrupt_stub

    align 16
    vector_24_handler:
        push 0
        ; vector number
        push 24
        jmp interrupt_stub

    align 16
    vector_25_handler:
        push 0
        ; vector number
        push 25
        jmp interrupt_stub

    align 16
    vector_26_handler:
        push 0
        ; vector number
        push 26
        jmp interrupt_stub

    align 16
    vector_27_handler:
        push 0
        ; vector number
        push 27
        jmp interrupt_stub

    align 16
    vector_28_handler:
        push 0
        ; vector number
        push 28
        jmp interrupt_stub

    align 16
    vector_29_handler:
        push 0
        ; vector number
        push 29
        jmp interrupt_stub

    align 16
    vector_30_handler:
        push 0
        ; vector number
        push 30
        jmp interrupt_stub

    align 16
    vector_31_handler:
        push 0
        ; vector number
        push 31
        jmp interrupt_stub

    ; user defined vector numbers: (32-255)

    align 16
    vector_32_handler:
        push 0
        ; vector number
        push 32
        jmp interrupt_stub

    align 16
    vector_33_handler:
        push 0
        ; vector number
        push 33
        jmp interrupt_stub
