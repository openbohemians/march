/*
 * March Language - VM Runner Implementation
 */

#include "runner.h"
#include <stdlib.h>
#include <stdio.h>

/* Create runner */
runner_t* runner_create(loader_t* loader) {
    runner_t* runner = malloc(sizeof(runner_t));
    if (!runner) return NULL;

    runner->loader = loader;

    /* Initialize VM */
    vm_init();

    return runner;
}

/* Free runner */
void runner_free(runner_t* runner) {
    if (runner) {
        free(runner);
    }
}

/* Execute a word by name */
bool runner_execute(runner_t* runner, const char* name) {
    /* Try CID-based linking first */
    dict_entry_t* entry = dict_lookup(runner->loader->dict, name);
    if (entry && entry->cid) {
        vm_word_entry_t* linked_code = (vm_word_entry_t*)loader_link_cid(runner->loader, entry->cid);
        if (linked_code) {
            vm_run(linked_code->body);
            return true;
        }
    }

    /* Fallback to legacy cell-based loading */
    loaded_word_t* word = loader_find_word(runner->loader, name);
    if (!word) {
        word = loader_load_word(runner->loader, name, "user");
        if (!word) {
            fprintf(stderr, "Cannot load word: %s\n", name);
            return false;
        }
    }

    /* Execute on VM */
    vm_run(word->cells);

    return true;
}

/* Get stack contents after execution */
int runner_get_stack(runner_t* runner, int64_t* stack, int max_depth) {
    (void)runner;  /* Unused - VM is global state */

    /* Calculate stack depth
     * Stack grows down from top: data_stack_base + 8*1024 - 8
     * vm_get_dsp() returns current stack pointer
     * Depth = (initial_top - current_sp) / sizeof(uint64_t)
     */
    uint64_t* dsp = vm_get_dsp();
    uint64_t* stack_top = data_stack_base + VM_DATA_STACK_WORDS;
    ptrdiff_t depth = stack_top - dsp;

    if (depth < 0) depth = 0;
    if (depth > max_depth) depth = max_depth;

    /* Copy stack contents (dsp points to bottom of used stack) */
    for (ptrdiff_t i = 0; i < depth; i++) {
        stack[depth - 1 - i] = (int64_t)dsp[i];  /* Reverse order - TOS is at index 0 */
    }

    return (int)depth;
}

/* Print stack (for debugging) */
void runner_print_stack(runner_t* runner) {
    int64_t stack[32];
    int depth = runner_get_stack(runner, stack, 32);

    printf("Stack (%d items):\n", depth);
    for (int i = 0; i < depth; i++) {
        printf("  [%d] = %ld\n", i, stack[i]);
    }
}
