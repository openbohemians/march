; ! ( value addr -- )
; Stores a 64-bit value to memory address
; Stack effect: Pop value and address, write value to address

section .text
extern vm_dispatch
global op_store

op_store:
    ; rsi = data stack pointer
    ; [rsi] = TOS (addr)
    ; [rsi+8] = second (value)
    ; NOTE: rbx is reserved for VM IP, use r10 for temp storage

    mov rax, [rsi]          ; Load address
    mov r10, [rsi + 8]      ; Load value into temp register (not rbx!)
    mov [rax], r10          ; Store value at address
    add rsi, 16             ; Drop both items
    jmp vm_dispatch         ; Return to VM dispatch (FORTH-style)
