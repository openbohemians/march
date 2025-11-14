/*
 * March Language - One-Pass Compiler
 */

#ifndef MARCH_COMPILER_H
#define MARCH_COMPILER_H

#include "types.h"
#include "cells.h"
#include "tokens.h"
#include "dictionary.h"
#include "database.h"

/* Maximum quotation nesting depth */
#define MAX_QUOT_DEPTH 16

/* Maximum array literal nesting depth */
#define MAX_ARRAY_DEPTH 16

/* Maximum pending quotation references in a single word */
#define MAX_QUOT_REFS 64

/* Maximum allocation slots per word */
#define MAX_SLOTS 256

/* Maximum cached word definitions */
#define MAX_WORD_DEFS 256

/* Maximum specialization cache entries */
#define MAX_SPECIALIZATIONS 512

/* Quotation kind */
typedef enum {
    QUOT_LITERAL,  /* Lexical - uncompiled tokens, compile at use site */
    QUOT_TYPED     /* Typed - compiled with concrete types */
} quot_kind_t;

/* Quotation (compile-time only) */
typedef struct {
    quot_kind_t kind;              /* Literal vs Typed */

    /* For QUOT_TYPED: compiled form */
    cell_buffer_t* cells;          /* Legacy: runtime cells */
    blob_buffer_t* blob;           /* CID-based: blob encoding */

    /* For QUOT_LITERAL: token storage */
    token_t* tokens;               /* Array of captured tokens */
    int token_count;
    int token_capacity;

    /* Type information */
    type_id_t inputs[MAX_TYPE_STACK];
    int input_count;
    type_id_t outputs[MAX_TYPE_STACK];
    int output_count;
} quotation_t;

/* Word definition (stored form) - words are named quotations */
typedef struct word_definition {
    char* name;
    token_t* tokens;               /* Array of definition tokens */
    int token_count;
    int token_capacity;
    type_sig_t* type_sig;          /* Optional explicit type signature */
} word_definition_t;

/* Specialization cache entry - stores compiled versions by concrete types */
typedef struct {
    char* word_name;               /* Name of the word */
    type_id_t input_types[8];      /* Concrete input types (cache key) */
    int input_count;
    unsigned char* cid;            /* CID of compiled specialization (32 bytes) */
} specialization_t;

/* Compiler state */
typedef struct compiler {
    dictionary_t* dict;
    march_db_t* db;
    type_stack_entry_t type_stack[MAX_TYPE_STACK];
    int type_stack_depth;
    cell_buffer_t* cells;          /* Legacy: runtime cells (deprecated) */
    blob_buffer_t* blob;           /* CID-based blob encoding (LINKING.md) */
    ref_graph_t* ref_graph;        /* Reference graph for memory management */
    bool verbose;

    /* Compile-time slot allocation (like register allocation for heap ptrs) */
    bool slot_used[MAX_SLOTS];        /* Which slots are currently in use */
    int slot_count;                   /* Number of slots allocated (peak usage) */

    /* Type signature for next word definition (from $ declaration) */
    type_sig_t* pending_type_sig;

    /* Quotation compilation support */
    quotation_t* quot_stack[MAX_QUOT_DEPTH];
    int quot_stack_depth;

    /* Buffer stack for nested quotations */
    cell_buffer_t* buffer_stack[MAX_QUOT_DEPTH + 1];  /* +1 for root */
    int buffer_stack_depth;

    /* Blob buffer stack for nested quotations (CID-based) */
    blob_buffer_t* blob_stack[MAX_QUOT_DEPTH + 1];    /* +1 for root */
    int blob_stack_depth;

    /* Runtime quotation counter for generating unique names (deprecated) */
    int quot_counter;

    /* Pending quotation CID references (for linking) */
    unsigned char* pending_quot_cids[MAX_QUOT_REFS];
    int pending_quot_count;

    /* Array literal compilation support */
    int array_marker_stack[MAX_ARRAY_DEPTH];  /* Stack depth at each [ */
    int array_marker_depth;                    /* Number of nested [ ] */
    int array_consumed_count[MAX_ARRAY_DEPTH]; /* Pre-marker values consumed by _ at each level */

    /* Word definition cache (compile-time only) */
    /* Words are named quotations - stored as tokens, compiled at call site */
    word_definition_t* word_defs[MAX_WORD_DEFS];
    int word_def_count;

    /* Specialization cache - stores compiled versions by concrete types */
    /* Cache key: (word_name, input_types[]) → CID */
    specialization_t specializations[MAX_SPECIALIZATIONS];
    int specialization_count;
} compiler_t;

/* Create/free compiler */
compiler_t* compiler_create(dictionary_t* dict, march_db_t* db);
void compiler_free(compiler_t* comp);

/* Compile a file */
bool compiler_compile_file(compiler_t* comp, const char* filename);

/* Register primitives */
void compiler_register_primitives(compiler_t* comp);

/* Phase 5: On-demand compilation for token-based words */
blob_buffer_t* word_compile_with_context(compiler_t* comp, word_definition_t* word_def,
                                          type_id_t* input_types, int input_count);

#endif /* MARCH_COMPILER_H */
