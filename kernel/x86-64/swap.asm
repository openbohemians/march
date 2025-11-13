; swap ( a b -- b a )
; Exchanges the top two stack items
; Stack effect: Swap TOS and second item

section .text
extern vm_dispatch
global op_swap

op_swap:
    ; rsi = data stack pointer
    ; [rsi] = TOS (b)
    ; [rsi+8] = second (a)
    ; NOTE: rbx is reserved for VM IP, use r10 for temp storage

    mov rax, [rsi]          ; Load b
    mov r10, [rsi + 8]      ; Load a into temp register (not rbx!)
    mov [rsi], r10          ; Store a at TOS
    mov [rsi + 8], rax      ; Store b at second
    jmp vm_dispatch         ; Return to VM dispatch (FORTH-style)
