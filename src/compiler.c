/*
 * March Language - One-Pass Compiler Implementation
 */

#define _POSIX_C_SOURCE 200809L

#include "compiler.h"
#include "primitives.h"
#include "debug.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

/* Forward declarations */
static bool compile_if(compiler_t* comp);
static bool compile_true(compiler_t* comp);
static bool compile_false(compiler_t* comp);
static bool materialize_quotations(compiler_t* comp);

/* Create compiler */
compiler_t* compiler_create(dictionary_t* dict, march_db_t* db) {
    compiler_t* comp = malloc(sizeof(compiler_t));
    if (!comp) return NULL;

    comp->dict = dict;
    comp->db = db;
    comp->type_stack_depth = 0;
    comp->cells = cell_buffer_create();
    comp->blob = blob_buffer_create();  /* CID-based encoding buffer */
    comp->verbose = false;
    comp->quot_stack_depth = 0;
    comp->buffer_stack_depth = 0;
    comp->blob_stack_depth = 0;
    comp->quot_counter = 0;
    comp->pending_quot_count = 0;

    if (!comp->cells || !comp->blob) {
        if (comp->cells) cell_buffer_free(comp->cells);
        if (comp->blob) blob_buffer_free(comp->blob);
        free(comp);
        return NULL;
    }

    return comp;
}

/* Free compiler */
void compiler_free(compiler_t* comp) {
    if (comp) {
        cell_buffer_free(comp->cells);
        blob_buffer_free(comp->blob);
        /* Free pending quotation CIDs */
        for (int i = 0; i < comp->pending_quot_count; i++) {
            free(comp->pending_quot_cids[i]);
        }
        free(comp);
    }
}

/* Register primitives */
void compiler_register_primitives(compiler_t* comp) {
    type_sig_t sig;

    DEBUG_COMPILER("Registering primitives...");

    /* Register assembly primitives */
    register_primitives(comp->dict);

    /* Register immediate (compile-time) words */

    /* if: ( flag quot_true quot_false -- ... ) */
    /* Type checking handled dynamically based on quotations */
    parse_type_sig("->", &sig);  /* Minimal signature - real checking in handler */
    dict_add(comp->dict, "if", NULL, NULL, 0, &sig, false, true,
             (immediate_handler_t)compile_if);

    /* true: ( -- -1 ) */
    parse_type_sig("-> i64", &sig);
    dict_add(comp->dict, "true", NULL, NULL, 0, &sig, false, true,
             (immediate_handler_t)compile_true);

    /* false: ( -- 0 ) */
    parse_type_sig("-> i64", &sig);
    dict_add(comp->dict, "false", NULL, NULL, 0, &sig, false, true,
             (immediate_handler_t)compile_false);

    debug_dump_dict_stats(comp->dict);
}

/* Push type onto type stack */
static void push_type(compiler_t* comp, type_id_t type) {
    if (comp->type_stack_depth >= MAX_TYPE_STACK) {
        fprintf(stderr, "Type stack overflow\n");
        return;
    }
    comp->type_stack[comp->type_stack_depth++] = type;
}

/* Pop type from type stack */
static type_id_t pop_type(compiler_t* comp) {
    if (comp->type_stack_depth == 0) {
        fprintf(stderr, "Type stack underflow\n");
        return TYPE_ANY;
    }
    return comp->type_stack[--comp->type_stack_depth];
}

/* Apply word's type signature to type stack */
static bool apply_signature(compiler_t* comp, type_sig_t* sig) {
    /* Check we have enough inputs */
    if (comp->type_stack_depth < sig->input_count) {
        fprintf(stderr, "Type error: need %d inputs, have %d\n",
                sig->input_count, comp->type_stack_depth);
        return false;
    }

    /* Pop inputs and verify types */
    for (int i = sig->input_count - 1; i >= 0; i--) {
        type_id_t expected = sig->inputs[i];
        type_id_t actual = pop_type(comp);

        /* TYPE_ANY matches anything */
        if (expected != TYPE_ANY && actual != TYPE_ANY && expected != actual) {
            fprintf(stderr, "Type mismatch: expected %d, got %d\n", expected, actual);
            return false;
        }
    }

    /* Push outputs */
    for (int i = 0; i < sig->output_count; i++) {
        push_type(comp, sig->outputs[i]);
    }

    return true;
}

/* Compile a number literal */
static bool compile_number(compiler_t* comp, int64_t num) {
    /* Legacy: Emit LIT cell */
    cell_buffer_append(comp->cells, encode_lit(num));

    /* CID-based: Store literal and encode reference */
    unsigned char* cid = db_store_literal(comp->db, num, "i64");
    if (!cid) {
        fprintf(stderr, "Error: Failed to store literal %ld\n", num);
        return false;
    }
    encode_cid_ref(comp->blob, BLOB_DATA, cid);
    free(cid);

    /* Push i64 type */
    push_type(comp, TYPE_I64);

    if (comp->verbose) {
        printf("  LIT %ld → i64\n", num);
    }

    return true;
}

/* Compile a word reference */
static bool compile_word(compiler_t* comp, const char* name) {
    /* Lookup word (may be immediate, primitive, or user word) */
    dict_entry_t* entry = dict_lookup(comp->dict, name);

    if (!entry) {
        fprintf(stderr, "Unknown word: %s\n", name);
        return false;
    }

    /* Check if this is an immediate (compile-time) word */
    if (entry->is_immediate) {
        if (!entry->handler) {
            fprintf(stderr, "Internal error: immediate word '%s' has no handler\n", name);
            return false;
        }
        /* Call the immediate handler */
        return entry->handler(comp);
    }

    /* Not immediate - materialize any COMPLETED quotations first */
    /* Don't materialize if we're currently INSIDE a quotation (building it) */
    if (comp->quot_stack_depth > 0 && comp->buffer_stack_depth == 0) {
        if (!materialize_quotations(comp)) {
            return false;
        }
    }

    /* Not immediate - do type-aware lookup for overload resolution */
    entry = dict_lookup_typed(comp->dict, name,
                             comp->type_stack,
                             comp->type_stack_depth);

    if (!entry) {
        DEBUG_COMPILER("Failed to find match for word: %s", name);
        debug_dump_type_stack("Type stack at failure", comp->type_stack, comp->type_stack_depth);
        fprintf(stderr, "Type error: no matching overload for word: %s\n", name);
        return false;
    }

    /* Apply type signature */
    if (!apply_signature(comp, &entry->signature)) {
        fprintf(stderr, "Type error in word: %s\n", name);
        return false;
    }

    /* Emit XT cell */
    if (entry->is_primitive) {
        /* Legacy: Emit XT with address */
        cell_buffer_append(comp->cells, encode_xt(entry->addr));

        /* CID-based: Emit primitive ID (2 bytes) */
        encode_primitive(comp->blob, entry->prim_id);
    } else {
        /* Legacy: Emit placeholder */
        cell_buffer_append(comp->cells, encode_xt(NULL));

        /* CID-based: Emit CID reference */
        if (entry->cid) {
            encode_cid_ref(comp->blob, BLOB_WORD, entry->cid);
        } else {
            fprintf(stderr, "Error: user word '%s' has no CID\n", name);
            return false;
        }
    }

    if (comp->verbose) {
        printf("  XT %s", name);
        print_type_sig(&entry->signature);
        printf("\n");
    }

    return true;
}

/* Start quotation - save current state and begin new compilation */
static bool compile_lparen(compiler_t* comp) {
    if (comp->quot_stack_depth >= MAX_QUOT_DEPTH) {
        fprintf(stderr, "Quotation nesting too deep (max %d)\n", MAX_QUOT_DEPTH);
        return false;
    }

    /* Create new quotation */
    quotation_t* quot = malloc(sizeof(quotation_t));
    if (!quot) return false;

    quot->cells = cell_buffer_create();
    quot->blob = blob_buffer_create();
    if (!quot->cells || !quot->blob) {
        if (quot->cells) cell_buffer_free(quot->cells);
        if (quot->blob) blob_buffer_free(quot->blob);
        free(quot);
        return false;
    }

    /* Capture current type stack as quotation inputs */
    quot->input_count = comp->type_stack_depth;
    for (int i = 0; i < comp->type_stack_depth; i++) {
        quot->inputs[i] = comp->type_stack[i];
    }

    /* Push quotation onto stack */
    comp->quot_stack[comp->quot_stack_depth++] = quot;

    /* Save current buffers and switch to quotation buffers */
    comp->buffer_stack[comp->buffer_stack_depth++] = comp->cells;
    comp->cells = quot->cells;
    comp->blob_stack[comp->blob_stack_depth++] = comp->blob;
    comp->blob = quot->blob;

    /* Reset type stack for quotation body */
    comp->type_stack_depth = 0;

    if (comp->verbose) {
        printf("  ( start quotation\n");
    }

    return true;
}

/* End quotation - finalize and push onto quotation stack */
static bool compile_rparen(compiler_t* comp) {
    if (comp->quot_stack_depth == 0) {
        fprintf(stderr, "Unmatched ')' - no quotation in progress\n");
        return false;
    }

    if (comp->buffer_stack_depth == 0) {
        fprintf(stderr, "Internal error: buffer stack empty in rparen\n");
        return false;
    }

    /* Get current quotation */
    quotation_t* quot = comp->quot_stack[comp->quot_stack_depth - 1];

    /* Emit EXIT */
    cell_buffer_append(quot->cells, encode_exit());

    /* Capture output types */
    quot->output_count = comp->type_stack_depth;
    for (int i = 0; i < comp->type_stack_depth; i++) {
        quot->outputs[i] = comp->type_stack[i];
    }

    /* Restore parent buffers */
    comp->cells = comp->buffer_stack[--comp->buffer_stack_depth];
    comp->blob = comp->blob_stack[--comp->blob_stack_depth];

    /* Restore type stack (quotation inputs + outputs become new stack) */
    comp->type_stack_depth = quot->input_count;
    for (int i = 0; i < quot->input_count; i++) {
        comp->type_stack[i] = quot->inputs[i];
    }

    if (comp->verbose) {
        printf("  ) end quotation: ");
        for (int i = 0; i < quot->input_count; i++) {
            printf("%d ", quot->inputs[i]);
        }
        printf("-> ");
        for (int i = 0; i < quot->output_count; i++) {
            printf("%d ", quot->outputs[i]);
        }
        printf(" (%zu cells)\n", quot->cells->count);
    }

    /* Note: quotation stays on quot_stack for immediate word to consume */

    return true;
}

/* Pop quotation from stack (for use by immediate words) */
static quotation_t* pop_quotation(compiler_t* comp) {
    if (comp->quot_stack_depth == 0) {
        return NULL;
    }
    return comp->quot_stack[--comp->quot_stack_depth];
}

/* Materialize pending quotations as runtime values */
static bool materialize_quotations(compiler_t* comp) {
    while (comp->quot_stack_depth > 0) {
        quotation_t* quot = pop_quotation(comp);
        if (!quot) return false;

        /* Build type signature strings: inputs -> outputs */
        char input_sig[128] = "";
        char output_sig[128] = "";
        char* p;

        /* Input types */
        p = input_sig;
        for (int i = 0; i < quot->input_count; i++) {
            switch (quot->inputs[i]) {
                case TYPE_I64: p += sprintf(p, "i64 "); break;
                case TYPE_U64: p += sprintf(p, "u64 "); break;
                case TYPE_F64: p += sprintf(p, "f64 "); break;
                case TYPE_PTR: p += sprintf(p, "ptr "); break;
                case TYPE_BOOL: p += sprintf(p, "bool "); break;
                default: p += sprintf(p, "? "); break;
            }
        }
        /* Trim trailing space */
        if (p > input_sig && p[-1] == ' ') p[-1] = '\0';

        /* Output types */
        p = output_sig;
        for (int i = 0; i < quot->output_count; i++) {
            switch (quot->outputs[i]) {
                case TYPE_I64: p += sprintf(p, "i64 "); break;
                case TYPE_U64: p += sprintf(p, "u64 "); break;
                case TYPE_F64: p += sprintf(p, "f64 "); break;
                case TYPE_PTR: p += sprintf(p, "ptr "); break;
                case TYPE_BOOL: p += sprintf(p, "bool "); break;
                default: p += sprintf(p, "? "); break;
            }
        }
        /* Trim trailing space */
        if (p > output_sig && p[-1] == ' ') p[-1] = '\0';

        /* Store type signature and get sig_cid */
        unsigned char* sig_cid = db_store_type_sig(comp->db,
                                                    input_sig[0] ? input_sig : NULL,
                                                    output_sig);
        if (!sig_cid) {
            fprintf(stderr, "Failed to store quotation type signature\n");
            cell_buffer_free(quot->cells);
            blob_buffer_free(quot->blob);
            free(quot);
            return false;
        }

        if (comp->verbose) {
            printf("  Materializing quotation: %s -> %s\n",
                   input_sig[0] ? input_sig : "(none)", output_sig);
        }

        /* Store quotation as anonymous blob with BLOB_QUOTATION kind */
        /* Use blob data (CID sequence), not cells */
        unsigned char* cid = db_store_blob(comp->db, BLOB_QUOTATION, sig_cid,
                                           quot->blob->data,
                                           quot->blob->size);
        free(sig_cid);

        if (!cid) {
            fprintf(stderr, "Failed to store quotation blob\n");
            cell_buffer_free(quot->cells);
            blob_buffer_free(quot->blob);
            free(quot);
            return false;
        }

        /* Track CID for linking */
        if (comp->pending_quot_count >= MAX_QUOT_REFS) {
            fprintf(stderr, "Too many quotation references in word (max %d)\n", MAX_QUOT_REFS);
            free(cid);
            cell_buffer_free(quot->cells);
            blob_buffer_free(quot->blob);
            free(quot);
            return false;
        }
        comp->pending_quot_cids[comp->pending_quot_count++] = cid;

        if (comp->verbose) {
            printf("  Quotation CID: %s (index %d)\n", cid, comp->pending_quot_count - 1);
        }

        /* Legacy: Emit [LIT 0] placeholder */
        cell_buffer_append(comp->cells, encode_lit(0));

        /* CID-based: Emit quotation reference */
        encode_cid_ref(comp->blob, BLOB_QUOTATION, cid);

        /* Push quotation type onto stack (as pointer for now) */
        push_type(comp, TYPE_PTR);

        /* Free quotation buffers */
        cell_buffer_free(quot->cells);
        blob_buffer_free(quot->blob);
        free(quot);
    }

    return true;
}

/* Immediate word: if - compile conditional branch with inlined quotations */
static bool compile_if(compiler_t* comp) {
    /* Stack should be: flag ( true ) ( false ) if */
    /* Pop two quotations (in reverse order: false, then true) */

    if (comp->quot_stack_depth < 2) {
        fprintf(stderr, "if requires two quotations: ( true ) ( false )\n");
        return false;
    }

    quotation_t* false_quot = pop_quotation(comp);
    quotation_t* true_quot = pop_quotation(comp);

    if (!false_quot || !true_quot) {
        fprintf(stderr, "Internal error: quotations not available for if\n");
        return false;
    }

    /* Check that flag is on type stack */
    if (comp->type_stack_depth < 1) {
        fprintf(stderr, "if requires a boolean flag on stack\n");
        return false;
    }

    /* Pop flag type */
    pop_type(comp);

    if (comp->verbose) {
        printf("  IF compiling with true=%zu cells, false=%zu cells\n",
               true_quot->cells->count, false_quot->cells->count);
    }

    /* Look up branch primitives */
    dict_entry_t* zbranch_entry = dict_lookup(comp->dict, "0branch");
    dict_entry_t* branch_entry = dict_lookup(comp->dict, "branch");

    if (!zbranch_entry || !branch_entry) {
        fprintf(stderr, "Internal error: branch primitives not registered\n");
        return false;
    }

    /* Emit 0BRANCH primitive XT and placeholder offset */
    /* Pattern: [XT: 0branch] [LIT: offset] */
    cell_buffer_append(comp->cells, encode_xt(zbranch_entry->addr));
    size_t zbranch_offset_pos = comp->cells->count;
    cell_buffer_append(comp->cells, encode_lit(0));  /* Placeholder offset */

    /* Inline true quotation cells (minus EXIT) */
    for (size_t i = 0; i < true_quot->cells->count - 1; i++) {  /* -1 to skip EXIT */
        cell_buffer_append(comp->cells, true_quot->cells->cells[i]);
    }

    /* Emit BRANCH primitive XT and placeholder offset */
    /* Pattern: [XT: branch] [LIT: offset] */
    cell_buffer_append(comp->cells, encode_xt(branch_entry->addr));
    size_t branch_offset_pos = comp->cells->count;
    cell_buffer_append(comp->cells, encode_lit(0));  /* Placeholder offset */

    /* Calculate 0BRANCH offset: points to start of false branch */
    /* Offset is in cells from the cell AFTER the offset LIT */
    int64_t zbranch_offset = (int64_t)(comp->cells->count - zbranch_offset_pos - 1);
    comp->cells->cells[zbranch_offset_pos] = encode_lit(zbranch_offset);

    /* Inline false quotation cells (minus EXIT) */
    for (size_t i = 0; i < false_quot->cells->count - 1; i++) {  /* -1 to skip EXIT */
        cell_buffer_append(comp->cells, false_quot->cells->cells[i]);
    }

    /* Calculate BRANCH offset: points past end of false branch */
    /* Offset is in cells from the cell AFTER the offset LIT */
    int64_t branch_offset = (int64_t)(comp->cells->count - branch_offset_pos - 1);
    comp->cells->cells[branch_offset_pos] = encode_lit(branch_offset);

    /* Apply quotation output types to current stack */
    /* Both branches should have same output types - use true_quot */
    for (int i = 0; i < true_quot->output_count; i++) {
        push_type(comp, true_quot->outputs[i]);
    }

    /* Free quotations */
    cell_buffer_free(true_quot->cells);
    free(true_quot);
    cell_buffer_free(false_quot->cells);
    free(false_quot);

    if (comp->verbose) {
        printf("  IF compiled: 0branch offset=%lld, branch offset=%lld\n",
               (long long)zbranch_offset, (long long)branch_offset);
    }

    return true;
}

/* Immediate word: true - emit literal -1 */
static bool compile_true(compiler_t* comp) {
    cell_buffer_append(comp->cells, encode_lit(-1));
    push_type(comp, TYPE_I64);

    if (comp->verbose) {
        printf("  LIT -1 (true)\n");
    }

    return true;
}

/* Immediate word: false - emit literal 0 */
static bool compile_false(compiler_t* comp) {
    cell_buffer_append(comp->cells, encode_lit(0));
    push_type(comp, TYPE_I64);

    if (comp->verbose) {
        printf("  LIT 0 (false)\n");
    }

    return true;
}

/* Compile a word definition */
static bool compile_definition(compiler_t* comp, token_stream_t* stream) {
    /* Read word name */
    token_t name_tok;
    if (!token_stream_next(stream, &name_tok)) {
        fprintf(stderr, "Expected word name after ':'\n");
        return false;
    }

    if (name_tok.type != TOK_WORD) {
        fprintf(stderr, "Expected word name, got token type %d\n", name_tok.type);
        token_free(&name_tok);
        return false;
    }

    char* word_name = strdup(name_tok.text);
    token_free(&name_tok);

    if (comp->verbose) {
        printf("\nCompiling word: %s\n", word_name);
    }

    /* Reset buffers and type stack for new definition */
    cell_buffer_clear(comp->cells);
    blob_buffer_clear(comp->blob);
    comp->type_stack_depth = 0;

    /* Build source text as we compile */
    char* source_text = malloc(4096);  /* Initial buffer */
    if (!source_text) {
        free(word_name);
        return false;
    }
    source_text[0] = '\0';
    size_t source_len = 0;
    size_t source_cap = 4096;

    /* Compile tokens until ';' */
    token_t tok;
    while (token_stream_next(stream, &tok)) {
        if (tok.type == TOK_SEMICOLON) {
            token_free(&tok);
            break;
        }

        /* Append token text to source (with space separator) */
        size_t tok_len = strlen(tok.text);
        size_t needed = source_len + tok_len + 2;  /* +2 for space and null */
        if (needed > source_cap) {
            source_cap = needed * 2;
            char* new_buf = realloc(source_text, source_cap);
            if (!new_buf) {
                free(source_text);
                free(word_name);
                token_free(&tok);
                return false;
            }
            source_text = new_buf;
        }
        if (source_len > 0) {
            source_text[source_len++] = ' ';
        }
        strcpy(source_text + source_len, tok.text);
        source_len += tok_len;

        bool success = false;

        if (tok.type == TOK_NUMBER) {
            success = compile_number(comp, tok.number);
        } else if (tok.type == TOK_WORD) {
            success = compile_word(comp, tok.text);
        } else if (tok.type == TOK_LPAREN) {
            success = compile_lparen(comp);
        } else if (tok.type == TOK_RPAREN) {
            success = compile_rparen(comp);
        } else {
            fprintf(stderr, "Unexpected token in definition: %s\n", tok.text);
        }

        token_free(&tok);

        if (!success) {
            free(source_text);
            free(word_name);
            return false;
        }
    }

    /* Materialize any pending quotations before ending definition */
    if (comp->quot_stack_depth > 0) {
        if (!materialize_quotations(comp)) {
            free(source_text);
            free(word_name);
            return false;
        }
    }

    /* Emit EXIT */
    cell_buffer_append(comp->cells, encode_exit());

    /* Build type signature from final stack state */
    /* For now, just use "-> " + output types */
    /* TODO: input type inference for words without literals */
    char type_sig[256];
    if (comp->type_stack_depth > 0) {
        strcpy(type_sig, "-> ");
        for (int i = 0; i < comp->type_stack_depth; i++) {
            switch (comp->type_stack[i]) {
                case TYPE_I64: strcat(type_sig, "i64 "); break;
                case TYPE_U64: strcat(type_sig, "u64 "); break;
                case TYPE_F64: strcat(type_sig, "f64 "); break;
                case TYPE_PTR: strcat(type_sig, "ptr "); break;
                case TYPE_BOOL: strcat(type_sig, "bool "); break;
                case TYPE_STR: strcat(type_sig, "str "); break;
                case TYPE_ANY: strcat(type_sig, "any "); break;
                case TYPE_UNKNOWN: strcat(type_sig, "? "); break;
            }
        }
    } else {
        strcpy(type_sig, "->");
    }

    if (comp->verbose) {
        printf("  Type signature: %s\n", type_sig);
        printf("  Source: %s\n", source_text);
        printf("  %zu cells, %zu bytes\n",
               comp->cells->count, comp->cells->count * 8);
        printf("  %zu blob bytes\n", comp->blob->size);
    }

    /* Legacy: Store cells in database */
    bool stored = db_store_word(comp->db, word_name, "user",
                                (uint8_t*)comp->cells->cells,
                                comp->cells->count,
                                type_sig,
                                source_text);

    if (!stored) {
        fprintf(stderr, "Failed to store word (legacy): %s\n", word_name);
        free(source_text);
        free(word_name);
        return false;
    }

    /* CID-based: Compute CID and store blob */
    unsigned char* sig_cid = db_store_type_sig(comp->db, NULL, type_sig);
    if (!sig_cid) {
        fprintf(stderr, "Failed to store type signature for: %s\n", word_name);
        free(source_text);
        free(word_name);
        return false;
    }

    unsigned char* word_cid = db_store_blob(comp->db, BLOB_WORD, sig_cid,
                                             comp->blob->data,
                                             comp->blob->size);
    free(sig_cid);

    if (!word_cid) {
        fprintf(stderr, "Failed to store word blob: %s\n", word_name);
        free(source_text);
        free(word_name);
        return false;
    }

    if (comp->verbose) {
        char* cid_hex = cid_to_hex(word_cid);
        printf("  ✓ Stored word: %s (CID: %.16s...)\n", word_name, cid_hex);
        free(cid_hex);
    }

    /* Add to dictionary so it can be used in later definitions */
    type_sig_t sig;
    if (parse_type_sig(type_sig, &sig)) {
        /* Pass the CID so other words can reference it */
        dict_add(comp->dict, word_name, NULL, word_cid, 0, &sig, false, false, NULL);
    } else {
        free(word_cid);
    }

    free(source_text);
    free(word_name);
    return true;
}

/* Compile a file */
bool compiler_compile_file(compiler_t* comp, const char* filename) {
    token_stream_t* stream = token_stream_create(filename);
    if (!stream) {
        fprintf(stderr, "Cannot open file: %s\n", filename);
        return false;
    }

    if (comp->verbose) {
        printf("Compiling: %s\n", filename);
    }

    token_t tok;
    while (token_stream_next(stream, &tok)) {
        bool success = true;

        if (tok.type == TOK_COLON) {
            /* Start word definition */
            success = compile_definition(comp, stream);
        } else if (tok.type == TOK_NUMBER || tok.type == TOK_WORD) {
            /* Top-level expressions not yet supported */
            fprintf(stderr, "Top-level expressions not supported yet: %s\n", tok.text);
            success = false;
        }

        token_free(&tok);

        if (!success) {
            token_stream_free(stream);
            return false;
        }
    }

    token_stream_free(stream);
    return true;
}
