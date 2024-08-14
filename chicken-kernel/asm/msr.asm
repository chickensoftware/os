bits 64

; contains code for cpu model specific registers

section .text
; check for model specific registers (like EFER)
global cpu_has_msr

cpu_has_msr:
; eax Version Information
mov rax, 1
; clear rdx which will hold the value
xor rdx, rdx

cpuid

; check if msr is supported
test rdx, 1 << 5
jz no_msr

; return 1
mov rax, 1
ret

no_msr:
; return 0
xor rax, rax
ret


global get_msr

; get model specific register specified by index passed to fucnction
get_msr:
; move msr index to rcx
mov rcx, rdi

; only uses lower 32-bits of rcx
rdmsr

; construct 64-bit msr value
; bitshift high 32-bits in rdx to low 32-bits
shl rdx, 32

; combine rax and rdx to form a 64-bit value msr
or rax, rdx

ret

global set_msr

set_msr:
; copy msr index into rcx
mov rcx, rdi

; copy lower 32 bits of value into rax (wrsmr will only use lower 32 bits anyway)
mov rax, rsi

; copy higher 32 bits of value into rdx
mov rdx, rsi
shr rdx, 32

; write value
wrmsr

ret
