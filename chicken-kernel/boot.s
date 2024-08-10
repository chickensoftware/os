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