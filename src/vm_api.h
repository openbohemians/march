/*
 * March Language - Computed Goto VM API
 */

#ifndef MARCH_VM_API_H
#define MARCH_VM_API_H

#include "types.h"
#include <stdint.h>
#include <stddef.h>

#define VM_DATA_STACK_WORDS   1024
#define VM_RETURN_STACK_WORDS 1024

typedef struct vm_word_entry {
    cell_t *body;
    void (*entry)(void);
    void *stub_mem;
    size_t stub_size;
} vm_word_entry_t;

extern uint64_t data_stack_base[VM_DATA_STACK_WORDS];

void vm_init(void);
void vm_run(cell_t *entry);
vm_word_entry_t* vm_make_entry(cell_t *body);
void vm_free_entry(vm_word_entry_t *entry);
uint64_t* vm_get_dsp(void);

#endif /* MARCH_VM_API_H */
