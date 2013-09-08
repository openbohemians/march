// duplicate top of stack
defcode "dup",3,,DUP
mov (%esp),%eax
push %eax
NEXT
