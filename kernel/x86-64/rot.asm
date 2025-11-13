; rot ( a b c -- b c a )
; Rotates the top three stack items
; Stack effect: Third item moves to top

section .text
extern vm_dispatch
global op_rot

op_rot:
    ; rsi = data stack pointer
    ; [rsi] = TOS (c)
    ; [rsi+8] = second (b)
    ; [rsi+16] = third (a)
    ; NOTE: rbx is reserved for VM IP, use r10 for temp storage

    mov rax, [rsi]          ; Load c
    mov r10, [rsi + 8]      ; Load b into temp register (not rbx!)
    mov rcx, [rsi + 16]     ; Load a

    mov [rsi], rcx          ; Store a at TOS
    mov [rsi + 8], rax      ; Store c at second
    mov [rsi + 16], r10     ; Store b at third
    jmp vm_dispatch         ; Return to VM dispatch (FORTH-style)
