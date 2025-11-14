; pick - Copy nth stack item to top
; Stack effect: ( x0 x1 ... xn n -- x0 x1 ... xn xn )
; n=0 is equivalent to dup (copy TOS)
; n=1 is equivalent to over (copy second item)
; n=2 copies third item, etc.
;
; Implementation:
;   - Pop n from stack
;   - Calculate offset: (n+1) * 8 bytes
;   - Load value from that position
;   - Push it onto stack

global op_pick
extern vm_dispatch

op_pick:
    ; rsi points to TOS (top of stack)
    ; Stack layout (grows down):
    ;   [rsi]     = n (depth to pick from)
    ;   [rsi+8]   = x0 (item at depth 0, would be picked by n=0)
    ;   [rsi+16]  = x1 (item at depth 1, would be picked by n=1)
    ;   ...

    mov rax, [rsi]          ; Load n (depth)
    add rsi, 8              ; Drop n from stack

    ; Calculate offset: (n+1) * 8
    ; We want to access [rsi + n*8] since we already dropped n
    imul rax, 8             ; rax = n * 8

    ; Load value from stack at that depth
    mov r10, [rsi + rax]    ; r10 = value at depth n

    ; Push value onto stack
    sub rsi, 8              ; Make room
    mov [rsi], r10          ; Store picked value

    jmp vm_dispatch
