; 2r> ( -- n1 n2 ) ( R: n1 n2 -- )
; Moves top two items from return stack to data stack
; Stack effect: Pop two from return stack, push both to data stack
; Note: n2 is TOS on both stacks

section .text
extern vm_dispatch
global op_twofromr

op_twofromr:
    ; rsi = data stack pointer
    ; rdi = return stack pointer
    ; [rdi] = return stack TOS (n2)
    ; [rdi+8] = return stack second (n1)
    ; NOTE: rbx is reserved for VM IP, use r10 for temp storage

    mov rax, [rdi + 8]      ; Load n1
    mov r10, [rdi]          ; Load n2 into temp register (not rbx!)
    add rdi, 16             ; Drop both from return stack
    sub rsi, 16             ; Allocate space on data stack
    mov [rsi + 8], rax      ; Push n1 (will be second on data stack)
    mov [rsi], r10          ; Push n2 (will be TOS on data stack)
    jmp vm_dispatch         ; Return to VM dispatch (FORTH-style)
