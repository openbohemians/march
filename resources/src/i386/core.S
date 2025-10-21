// March Core Words

//
// Drop the top of stack.
//
defcode "drop",4,,DROP,1
  pop %eax
NEXT

//
// Duplicate top of stack.
//
defcode "dup",3,,DUP,1
  mov (%esp),%eax
  push %eax
NEXT

//
// Get the second element of stack
// and copy it to the top of the stack.
//
defcode "over",4,,OVER,2
  mov 4(%esp),%eax
  push %eax 
NEXT

//
// Rotate N stack elements. If N is a negative integer
// then the direction of rotation is in reverse.
//
defcode "rotate",3,,ROT,3
  // TODO
  pop %eax
  pop %ebx
  pop %ecx
  push %eax
  push %ecx
  push %ebx
NEXT

//
// Reverse rotate top three.
//
defcode "unrot",5,,UNROT,3
  pop %eax
  pop %ebx
  pop %ecx
  push %ebx
  push %eax
  push %ecx
NEXT

//
// Swap top two elements on stack.
//
defcode "swap",4,,SWAP,2
  pop %eax
  pop %ebx
  push %eax
  push %ebx
NEXT

//
// Run block multiple times.
//
//    (drop) 4 times
//
defcode "times",5,,TIMES,2
  // TODO
NEXT

//
// Addition
//
defcode "+",1,,ADD,2,type_int,type_int
  pop %eax
  pop %ebx
  add %ecx,%eax,%ebx
  push %ecx
NEXT

