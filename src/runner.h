/*
 * March Language - VM Runner
 * Execute compiled words on the VM
 */

#ifndef MARCH_RUNNER_H
#define MARCH_RUNNER_H

#include "types.h"
#include "loader.h"
#include "vm_api.h"
#include <stddef.h>
#include <stdint.h>

extern uint64_t data_stack_base[VM_DATA_STACK_WORDS];

/* Runner context */
typedef struct {
    loader_t* loader;
} runner_t;

/* Create/free runner */
runner_t* runner_create(loader_t* loader);
void runner_free(runner_t* runner);

/* Execute a word by name */
bool runner_execute(runner_t* runner, const char* name);

/* Get stack contents after execution */
int runner_get_stack(runner_t* runner, int64_t* stack, int max_depth);

/* Print stack (for debugging) */
void runner_print_stack(runner_t* runner);

#endif /* MARCH_RUNNER_H */
