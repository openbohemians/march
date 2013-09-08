// Rotate top three.
defcode "rot",3,,ROT
pop %eax
pop %ebx
pop %ecx
push %eax
push %ecx
push %ebx
NEXT
