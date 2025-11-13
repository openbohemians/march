; 2>r ( n1 n2 -- ) ( R: -- n1 n2 )
; Moves top two items from data stack to return stack
; Stack effect: Pop two from data stack, push both to return stack
; Note: n2 is TOS on both stacks

section .text
extern vm_dispatch
global op_twotor

op_twotor:
    ; rsi = data stack pointer
    ; rdi = return stack pointer
    ; [rsi] = TOS (n2)
    ; [rsi+8] = second (n1)
    ; NOTE: rbx is reserved for VM IP, use r10 for temp storage

    mov rax, [rsi + 8]      ; Load n1
    mov r10, [rsi]          ; Load n2 into temp register (not rbx!)
    add rsi, 16             ; Drop both from data stack
    sub rdi, 16             ; Allocate space on return stack
    mov [rdi + 8], rax      ; Push n1 (will be second on return stack)
    mov [rdi], r10          ; Push n2 (will be TOS on return stack)
    jmp vm_dispatch         ; Return to VM dispatch (FORTH-style)
