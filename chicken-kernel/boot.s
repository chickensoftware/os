bits 64

extern kernel_main ; rust main
extern KERNEL_STACK_SIZE

; set up stack

section .bss align=16
_start_stack:
  resb 16 * 1024 ; 16KiB

section .text
global _start
_start:
  mov rsp, _start_stack + 16 * 1024
  call kernel_main

.fin:
  hlt
  jmp .fin