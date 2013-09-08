// Get the second element of stack
// and copy it to the top of the stack.
defcode "over",4,,OVER
mov 4(%esp),%eax
push %eax 
NEXT
