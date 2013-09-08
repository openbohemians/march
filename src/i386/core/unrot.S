// Reverse rotate top three.
defcode "unrot",4,,UNROT
pop %eax
pop %ebx
pop %ecx
push %ebx
push %eax
push %ecx
NEXT

