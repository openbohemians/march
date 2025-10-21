// March (c) 2015 Tab Computing

// NEXT macro
.macro NEXT
    lodsl
    jmp *(%eax)
.endm

// macros to deal with the return stack
.macro PUSHRSP reg
    lea -4(%ebp),%ebp // push reg on to return stack
    movl \reg,(%ebp)
.endm

.macro POPRSP reg
    mov (%ebp),\reg // pop top of return stack to reg
    lea 4(%ebp),%ebp
.endm

// DOCOL, the interpreter!
    .text
    .align 4
DOCOL:
    PUSHRSP %esi // push %esi on to the return stack
    addl $4,%eax // %eax points to codeword, so make
    movl %eax,%esi // %esi point to first data word
    NEXT

// entry point
    .text
    .globl _start
_start:
    cld
    mov %esp,var_S0             // Save the initial data stack pointer in FORTH variable S0.
    mov $return_stack_top,%ebp  // Initialise the return stack.
    call set_up_data_segment

    mov $cold_start,%esi        // Initialise interpreter.
    NEXT                        // Run interpreter!

    .section .rodata
cold_start:                     // High-level code without a codeword.
    .int QUIT

// definition flags
.set F_IMMED,0x80
.set F_HIDDEN,0x20
.set F_LENMASK,0x1f // length mask

// linked-list definition store (TODO: convert to binary search tree)
.set link,0

// type definition
.macro deftype name, namelen, flags=0, partype, label
    .section .rodata
    .align 4
  .globl type_\label
  type_\label :
    .int partype                      // link to parent type
    .byte \flags + namelen
    .ascii "\name"
.endm

// march word definition
.macro defword name, namelen, flags=0, label, arity, types:vararg
    .section .rodata
    .align 4
  .globl name_\label
  name_\label :
    .int link                          // link (TODO: binary link)
    .set link,name_\label
    .word \flags + \namelen + \arity   // flags + length + arity
    .ascii "\name"                     // word name
    .align 4                           // padding to next 4 byte boundary
    .ifnb \types                       // type signature
      \types                           //   if not blank
    .else
      .rept \arity
        .int $0                        // if blank then all args are Any type
      .endr
    .end
    .align 4                           // padding to next 4 byte boundry
  .globl \label
  \label :
    .int DOCOL                         // codeword - the interpreter
    // list of word pointers follow
.endm

// machine code definition
.macro defcode name, namelen, flags=0, label
    .section .rodata
    .align 4
  .globl name_\label
  name_\label :
    .int link              // link
    .set link,name_\label
    .byte \flags+\namelen  // flags + length byte
    .ascii "\name"         // the name
    .align 4               // padding to next 4 byte boundary
  .globl \label
  \label :
    .int code_\label       // codeword
    .text
    //.align 4
  .globl code_\label
  code_\label :            // assembler code follows
.endm


