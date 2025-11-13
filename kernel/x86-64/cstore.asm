; c! ( byte addr -- )
; Stores a byte to memory address
; Stack effect: Pop byte and address, write byte to address

section .text
extern vm_dispatch
global op_cstore

op_cstore:
    ; rsi = data stack pointer
    ; [rsi] = TOS (addr)
    ; [rsi+8] = second (byte value)
    ; NOTE: rbx is reserved for VM IP, use r10 for temp storage

    mov rax, [rsi]          ; Load address
    mov r10, [rsi + 8]      ; Load byte value into temp register (not rbx!)
    mov [rax], r10b         ; Store low byte at address
    add rsi, 16             ; Drop both items
    jmp vm_dispatch         ; Return to VM dispatch (FORTH-style)
