// Gnu Assembler Macros

//
// NEXT macro.
//

    .macro NEXT
        lodsl
        jmp *(%eax)
    .endm

//
// Macros to deal with the return stack.
//

    .macro PUSHRSP reg
        lea -4(%ebp),%ebp // push reg on to return stack
        movl \reg,(%ebp)
    .endm

    .macro POPRSP reg
        mov (%ebp),\reg // pop top of return stack to reg
        lea 4(%ebp),%ebp
    .endm

//
// DOCOL, the interpreter!
//

    .text
    .align 4
DOCOL:
    PUSHRSP %esi // push %esi on to the return stack
    addl $4,%eax // %eax points to codeword, so make
    movl %eax,%esi // %esi point to first data word
    NEXT

//
// Assembler entry point.
//

    .text
    .globl _start
_start:
    cld
    mov %esp,var_S0             // Save the initial data stack pointer in FORTH variable S0.
    mov $return_stack_top,%ebp  // Initialise the return stack.
    call set_up_data_segment

    mov $cold_start,%esi  // Initialise interpreter.
    NEXT                  // Run interpreter!

    .section .rodata
cold_start:               // High-level code without a codeword.
    .int QUIT

//
// Flags
//

.set F_IMMED,0x80
.set F_HIDDEN,0x20
.set F_LENMASK,0x1f // length mask

// Store the chain of links.

.set link,0

//
// Define word.
//

.macro defword name, namelen, flags=0, label
    .section .rodata
    .align 4
    .globl name_\label
    name_\label :
    .int link // link
    .set link,name_\label
    .byte \flags+\namelen // flags + length byte
    .ascii "\name" // the name
    .align 4 // padding to next 4 byte boundary
    .globl \label
    \label :
    .int DOCOL // codeword - the interpreter
    // list of word pointers follow
.endm

//
// Define code.
//

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
    code_\label :          // assembler code follows
.endm


