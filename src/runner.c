/*
 * March Language - VM Runner Implementation
 */

#include "runner.h"
#include "cells.h"  /* For encode_xt, encode_exit */
#include "database.h"  /* For db_store_blob, db_store_type_sig */
#include <stdlib.h>
#include <stdio.h>

/* Create runner */
runner_t* runner_create(loader_t* loader, compiler_t* comp) {
    runner_t* runner = malloc(sizeof(runner_t));
    if (!runner) return NULL;

    runner->loader = loader;
    runner->comp = comp;

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
    /* Lookup word in dictionary */
    dict_entry_t* entry = dict_lookup(runner->loader->dict, name);

    /* Phase 5: Check if word has tokens but no compiled code (Design B) */
    if (entry && entry->word_def && !entry->cid) {
        if (runner->comp->verbose) {
            printf("\nOn-demand compilation: %s\n", name);
        }

        /* For top-level execution words, use empty input types (no args expected) */
        /* The word will be compiled with whatever types it produces */
        type_id_t empty_inputs[1];  /* Unused, but need valid array */
        int input_count = entry->signature.input_count;

        /* Compile the word with its defined signature */
        blob_buffer_t* compiled_blob = word_compile_with_context(
            runner->comp, entry->word_def, empty_inputs, input_count
        );

        if (!compiled_blob) {
            fprintf(stderr, "Error: Failed to compile word '%s' on-demand\n", name);
            return false;
        }

        /* Build type signature string from word's signature */
        char type_sig_str[256] = "-> ";  /* Top-level words have no inputs */
        char* p = type_sig_str + 3;
        for (int i = 0; i < entry->signature.output_count; i++) {
            type_id_t t = entry->signature.outputs[i];
            switch (t) {
                case TYPE_I64: p += sprintf(p, "i64 "); break;
                case TYPE_U64: p += sprintf(p, "u64 "); break;
                case TYPE_F64: p += sprintf(p, "f64 "); break;
                case TYPE_PTR: p += sprintf(p, "ptr "); break;
                case TYPE_BOOL: p += sprintf(p, "bool "); break;
                case TYPE_STR: p += sprintf(p, "str "); break;
                case TYPE_STR_MUT: p += sprintf(p, "str! "); break;
                case TYPE_ARRAY: p += sprintf(p, "array "); break;
                case TYPE_ARRAY_MUT: p += sprintf(p, "array! "); break;
                default: p += sprintf(p, "? "); break;
            }
        }

        /* Store the compiled blob in database */
        unsigned char* sig_cid = db_store_type_sig(runner->loader->db, NULL, type_sig_str);
        if (!sig_cid) {
            fprintf(stderr, "Error: Failed to store type signature for on-demand compilation\n");
            blob_buffer_free(compiled_blob);
            return false;
        }

        unsigned char* cid = db_store_blob(runner->loader->db, BLOB_WORD, sig_cid,
                                           compiled_blob->data, compiled_blob->size);
        free(sig_cid);
        blob_buffer_free(compiled_blob);

        if (!cid) {
            fprintf(stderr, "Error: Failed to store compiled word\n");
            return false;
        }

        /* Update dictionary entry with the new CID */
        if (entry->cid) {
            free(entry->cid);
        }
        entry->cid = cid;

        if (runner->comp->verbose) {
            printf("  Stored compiled version in database\n");
        }
    }

    /* Try CID-based linking */
    if (entry && entry->cid) {
        /* CID-based path: link and execute */
        void* linked_code = loader_link_cid(runner->loader, entry->cid);
        if (linked_code) {
            /* The linked code is a DOCOL wrapper (machine code).
             * Create a tiny cell stream that calls it, then pass to vm_run.
             * This ensures the VM state (rsi/rdi/rbx) is properly initialized.
             */
            cell_t bootstrap[2];
            bootstrap[0] = encode_xt(linked_code);  /* Call the wrapper */
            bootstrap[1] = encode_exit();            /* EXIT */

            fprintf(stderr, "TRACE: About to call vm_run with linked code at %p\n", linked_code);
            fprintf(stderr, "TRACE: Bootstrap cells: [0]=%016lx [1]=%016lx\n",
                    (unsigned long)bootstrap[0], (unsigned long)bootstrap[1]);
            fflush(stderr);
            vm_run(bootstrap);
            fprintf(stderr, "TRACE: vm_run returned successfully\n");
            fflush(stderr);
            return true;
        } else {
            fprintf(stderr, "Error: Failed to link word '%s'\n", name);
            return false;
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
    uint64_t* stack_top = (uint64_t*)((uint8_t*)data_stack_base + 8*1024 - 8);
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
