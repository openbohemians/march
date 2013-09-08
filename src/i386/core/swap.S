// swap top two elements on stack
defcode "swap",4,,SWAP
pop %eax
pop %ebx
push %eax
push %ebx
NEXT

