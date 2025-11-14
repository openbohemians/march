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
static bool compile_lparen(compiler_t* comp);
static bool compile_rparen(compiler_t* comp);
static bool compile_lbracket(compiler_t* comp);
static bool compile_rbracket(compiler_t* comp);
static bool compile_if(compiler_t* comp);
static bool compile_times_dispatch(compiler_t* comp);
static bool compile_times(compiler_t* comp);
static bool compile_times_until(compiler_t* comp);
static bool compile_true(compiler_t* comp);
static bool compile_false(compiler_t* comp);
static bool materialize_quotations(compiler_t* comp);

/* Stack primitive immediate handlers */
static bool compile_drop(compiler_t* comp);
static bool compile_dup(compiler_t* comp);
static bool compile_swap(compiler_t* comp);
static bool compile_over(compiler_t* comp);
static bool compile_rot(compiler_t* comp);
static bool compile_underscore(compiler_t* comp);
static bool compile_march_alloc(compiler_t* comp);

/* Create compiler */
compiler_t* compiler_create(dictionary_t* dict, march_db_t* db) {
    compiler_t* comp = malloc(sizeof(compiler_t));
    if (!comp) return NULL;

    comp->dict = dict;
    comp->db = db;
    comp->type_stack_depth = 0;
    comp->cells = cell_buffer_create();
    comp->blob = blob_buffer_create();  /* CID-based encoding buffer */
    comp->ref_graph = NULL;  /* Created per-word, not global */
    comp->verbose = false;
    comp->pending_type_sig = NULL;
    comp->quot_stack_depth = 0;
    comp->buffer_stack_depth = 0;
    comp->blob_stack_depth = 0;
    comp->quot_counter = 0;
    comp->pending_quot_count = 0;
    comp->array_marker_depth = 0;
    comp->word_def_count = 0;
    comp->specialization_count = 0;

    /* Initialize slot allocation */
    comp->slot_count = 0;
    for (int i = 0; i < MAX_SLOTS; i++) {
        comp->slot_used[i] = false;
    }

    if (!comp->cells || !comp->blob) {
        if (comp->cells) cell_buffer_free(comp->cells);
        if (comp->blob) blob_buffer_free(comp->blob);
        free(comp);
        return NULL;
    }

    return comp;
}

/* Free word definition */
static void word_definition_free(word_definition_t* def) {
    if (def) {
        free(def->name);
        /* Free tokens */
        if (def->tokens) {
            for (int i = 0; i < def->token_count; i++) {
                if (def->tokens[i].text) {
                    free(def->tokens[i].text);
                }
            }
            free(def->tokens);
        }
        if (def->type_sig) {
            free(def->type_sig);
        }
        free(def);
    }
}

/* Create word definition */
static word_definition_t* word_definition_create(const char* name) {
    word_definition_t* def = malloc(sizeof(word_definition_t));
    if (!def) return NULL;

    def->name = strdup(name);
    def->token_capacity = 16;
    def->token_count = 0;
    def->tokens = malloc(sizeof(token_t) * def->token_capacity);
    def->type_sig = NULL;

    if (!def->name || !def->tokens) {
        word_definition_free(def);
        return NULL;
    }

    return def;
}

/* Append token to word definition */
static bool word_def_append_token(word_definition_t* def, const token_t* tok) {
    /* Grow array if needed */
    if (def->token_count >= def->token_capacity) {
        int new_capacity = def->token_capacity * 2;
        token_t* new_tokens = realloc(def->tokens, sizeof(token_t) * new_capacity);
        if (!new_tokens) {
            fprintf(stderr, "Failed to grow word definition token array\n");
            return false;
        }
        def->tokens = new_tokens;
        def->token_capacity = new_capacity;
    }

    /* Deep copy the token */
    def->tokens[def->token_count] = *tok;
    if (tok->text) {
        def->tokens[def->token_count].text = strdup(tok->text);
        if (!def->tokens[def->token_count].text) {
            return false;
        }
    }

    def->token_count++;
    return true;
}

/* Find word definition in cache */
static word_definition_t* find_word_definition(compiler_t* comp, const char* name) {
    for (int i = 0; i < comp->word_def_count; i++) {
        if (strcmp(comp->word_defs[i]->name, name) == 0) {
            return comp->word_defs[i];
        }
    }
    return NULL;
}

/* Specialization cache: Look up compiled version by (word_name, input_types)
 * Returns cached CID if found, NULL otherwise */
static unsigned char* specialization_lookup(compiler_t* comp, const char* word_name,
                                             type_id_t* input_types, int input_count) {
    for (int i = 0; i < comp->specialization_count; i++) {
        specialization_t* spec = &comp->specializations[i];

        /* Check if word name matches */
        if (strcmp(spec->word_name, word_name) != 0) {
            continue;
        }

        /* Check if input count matches */
        if (spec->input_count != input_count) {
            continue;
        }

        /* Check if all input types match */
        bool types_match = true;
        for (int j = 0; j < input_count; j++) {
            if (spec->input_types[j] != input_types[j]) {
                types_match = false;
                break;
            }
        }

        if (types_match) {
            return spec->cid;  /* Cache hit! */
        }
    }

    return NULL;  /* Cache miss */
}

/* Specialization cache: Store compiled version */
static bool specialization_store(compiler_t* comp, const char* word_name,
                                  type_id_t* input_types, int input_count,
                                  const unsigned char* cid) {
    if (comp->specialization_count >= MAX_SPECIALIZATIONS) {
        fprintf(stderr, "Warning: Specialization cache full (max %d)\n", MAX_SPECIALIZATIONS);
        return false;
    }

    specialization_t* spec = &comp->specializations[comp->specialization_count];

    /* Store word name */
    spec->word_name = strdup(word_name);
    if (!spec->word_name) {
        return false;
    }

    /* Store input types */
    spec->input_count = input_count;
    for (int i = 0; i < input_count; i++) {
        spec->input_types[i] = input_types[i];
    }

    /* Store CID (copy 32 bytes) */
    spec->cid = malloc(CID_SIZE);
    if (!spec->cid) {
        free(spec->word_name);
        return false;
    }
    memcpy(spec->cid, cid, CID_SIZE);

    comp->specialization_count++;

    if (comp->verbose) {
        printf("  Cached specialization #%d: %s with %d input types\n",
               comp->specialization_count, word_name, input_count);
    }

    return true;
}

/* Free compiler */
void compiler_free(compiler_t* comp) {
    if (comp) {
        cell_buffer_free(comp->cells);
        blob_buffer_free(comp->blob);
        /* Free reference graph if still allocated */
        if (comp->ref_graph) {
            ref_graph_free(comp->ref_graph);
        }
        /* Free pending type signature */
        if (comp->pending_type_sig) {
            free(comp->pending_type_sig);
        }
        /* Free pending quotation CIDs */
        for (int i = 0; i < comp->pending_quot_count; i++) {
            free(comp->pending_quot_cids[i]);
        }
        /* Free word definitions */
        for (int i = 0; i < comp->word_def_count; i++) {
            word_definition_free(comp->word_defs[i]);
        }
        /* Free specialization cache */
        for (int i = 0; i < comp->specialization_count; i++) {
            free(comp->specializations[i].word_name);
            free(comp->specializations[i].cid);
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
             (immediate_handler_t)compile_if, NULL);

    /* true: ( -- -1 ) */
    parse_type_sig("-> i64", &sig);
    dict_add(comp->dict, "true", NULL, NULL, 0, &sig, false, true,
             (immediate_handler_t)compile_true, NULL);

    /* false: ( -- 0 ) */
    parse_type_sig("-> i64", &sig);
    dict_add(comp->dict, "false", NULL, NULL, 0, &sig, false, true,
             (immediate_handler_t)compile_false, NULL);

    /* times: Polymorphic - dispatches based on quotation stack depth
     *   1 quotation:  count ( body ) times - counted loop
     *   2 quotations: ( cond ) ( body ) times - until-style loop */
    parse_type_sig("i64 ->", &sig);
    dict_add(comp->dict, "times", NULL, NULL, 0, &sig, false, true,
             (immediate_handler_t)compile_times_dispatch, NULL);

    /* Stack manipulation primitives with compile-time RC tracking */
    /* These override the runtime primitives registered by register_primitives() */

    /* drop: ( a -- ) */
    parse_type_sig("a ->", &sig);
    dict_add(comp->dict, "drop", NULL, NULL, PRIM_DROP, &sig, false, true,
             (immediate_handler_t)compile_drop, NULL);

    /* dup: ( a -- a a ) */
    parse_type_sig("a -> a a", &sig);
    dict_add(comp->dict, "dup", NULL, NULL, PRIM_DUP, &sig, false, true,
             (immediate_handler_t)compile_dup, NULL);

    /* swap: ( a b -- b a ) */
    parse_type_sig("a b -> b a", &sig);
    dict_add(comp->dict, "swap", NULL, NULL, PRIM_SWAP, &sig, false, true,
             (immediate_handler_t)compile_swap, NULL);

    /* over: ( a b -- a b a ) */
    parse_type_sig("a b -> a b a", &sig);
    dict_add(comp->dict, "over", NULL, NULL, PRIM_OVER, &sig, false, true,
             (immediate_handler_t)compile_over, NULL);

    /* rot: ( a b c -- b c a ) */
    parse_type_sig("a b c -> b c a", &sig);
    dict_add(comp->dict, "rot", NULL, NULL, PRIM_ROT, &sig, false, true,
             (immediate_handler_t)compile_rot, NULL);

    /* _: Pull value from before [ marker into array comprehension
     * Type signature is dynamic - copies type from source value
     * Must be used inside array literals */
    parse_type_sig("-> a", &sig);  /* Minimal signature - actual type from pulled value */
    dict_add(comp->dict, "_", NULL, NULL, PRIM_IDENTITY, &sig, false, true,
             (immediate_handler_t)compile_underscore, NULL);

    /* march.alloc: Allocate heap memory with type graph tracking
     * This is a dual word - has both immediate handler and runtime primitive
     * The immediate handler emits the runtime code and updates the type graph */
    parse_type_sig("i64 -> ptr", &sig);
    dict_add(comp->dict, "march.alloc", NULL, NULL, PRIM_ALLOC, &sig, false, true,
             (immediate_handler_t)compile_march_alloc, NULL);

    debug_dump_dict_stats(comp->dict);
}

/* Check if a type is heap-allocated (with slot tracking) */
static bool type_is_heap(type_id_t type) {
    /* Strings are now heap-allocated like arrays (no slot tracking) */
    /* Add TYPE_BUF when buffers are implemented */
    (void)type;  /* Suppress unused warning */
    return false;
}

/* Push type onto type stack (non-heap value) */
static void push_type(compiler_t* comp, type_id_t type) {
    if (comp->type_stack_depth >= MAX_TYPE_STACK) {
        fprintf(stderr, "Type stack overflow\n");
        return;
    }
    comp->type_stack[comp->type_stack_depth].type = type;
    comp->type_stack[comp->type_stack_depth].slot_id = -1;  /* Not heap-allocated */
    comp->type_stack[comp->type_stack_depth].node_id = NODE_ID_INVALID;  /* Not a heap object */
    comp->type_stack_depth++;
}

/* Allocate a slot for heap allocation tracking */
static int allocate_slot(compiler_t* comp) {
    /* Find first free slot */
    for (int i = 0; i < MAX_SLOTS; i++) {
        if (!comp->slot_used[i]) {
            comp->slot_used[i] = true;
            if (i >= comp->slot_count) {
                comp->slot_count = i + 1;  /* Track peak usage */
            }
            return i;
        }
    }
    fprintf(stderr, "Error: Too many allocation slots (max %d)\n", MAX_SLOTS);
    return -1;
}

/* Free a slot (mark as available for reuse) */
static void free_slot(compiler_t* comp, int slot_id) {
    if (slot_id >= 0 && slot_id < MAX_SLOTS) {
        comp->slot_used[slot_id] = false;
    }
}

/* Push heap-allocated value onto type stack */
static void push_heap_value(compiler_t* comp, type_id_t type) {
    if (comp->type_stack_depth >= MAX_TYPE_STACK) {
        fprintf(stderr, "Type stack overflow\n");
        return;
    }

    int slot_id = allocate_slot(comp);
    if (slot_id < 0) return;  /* Error already printed */

    /* Create node in reference graph */
    node_id_t node_id = NODE_ID_INVALID;
    if (comp->ref_graph) {
        node_id = ref_graph_alloc_node(comp->ref_graph, type, slot_id);
        if (node_id == NODE_ID_INVALID) {
            fprintf(stderr, "Warning: Failed to allocate ref graph node\n");
        }
    }

    comp->type_stack[comp->type_stack_depth].type = type;
    comp->type_stack[comp->type_stack_depth].slot_id = slot_id;
    comp->type_stack[comp->type_stack_depth].node_id = node_id;
    comp->type_stack_depth++;

    if (comp->verbose) {
        printf("  ALLOC slot=%d node=%u type=%d\n", slot_id, node_id, type);
    }
}

/* Pop type from type stack */
static type_id_t pop_type(compiler_t* comp) {
    if (comp->type_stack_depth == 0) {
        fprintf(stderr, "Type stack underflow\n");
        return TYPE_ANY;
    }
    comp->type_stack_depth--;
    return comp->type_stack[comp->type_stack_depth].type;
}

/* Pop type stack entry (with slot tracking) */
static type_stack_entry_t pop_type_entry(compiler_t* comp) {
    if (comp->type_stack_depth == 0) {
        fprintf(stderr, "Type stack underflow\n");
        type_stack_entry_t entry = {TYPE_ANY, -1, NODE_ID_INVALID};
        return entry;
    }
    return comp->type_stack[--comp->type_stack_depth];
}

/* Note: In slot-based model, we don't need consume_value or duplicate_value.
 * Stack primitives just copy/remove slot_ids on the type stack.
 * Slots are only freed at word end if they're not on the return stack.
 */

/* Recursive mark function for liveness analysis */
static void mark_node(ref_graph_t* graph, node_id_t node_id, bool* marked) {
    if (node_id == NODE_ID_INVALID) return;
    if (marked[node_id]) return;  /* Already marked */

    marked[node_id] = true;

    /* Recursively mark children */
    ref_node_t* node = ref_graph_get_node(graph, node_id);
    if (node) {
        for (size_t i = 0; i < node->child_count; i++) {
            mark_node(graph, node->children[i], marked);
        }
    }
}

/* Perform liveness analysis and emit FREE instructions for dead nodes */
static void emit_free_for_dead_nodes(compiler_t* comp) {
    fprintf(stderr, "TRACE: emit_free_for_dead_nodes entry, comp=%p, ref_graph=%p\n",
            (void*)comp, (void*)(comp ? comp->ref_graph : NULL));
    if (comp && comp->ref_graph) {
        fprintf(stderr, "TRACE: ref_graph has %zu nodes\n", comp->ref_graph->node_count);
    }
    fflush(stderr);

    if (!comp || !comp->ref_graph || comp->ref_graph->node_count == 0) {
        if (comp && comp->verbose && !comp->ref_graph) {
            printf("  (No ref_graph - skipping liveness analysis)\n");
        }
        fprintf(stderr, "TRACE: Skipping liveness analysis (no nodes)\n");
        fflush(stderr);
        return;  /* No heap allocations in this word */
    }

    if (comp->verbose) {
        printf("  Liveness analysis: %zu nodes in graph\n", comp->ref_graph->node_count);
    }

    /* Allocate marked array (indexed by node_id) */
    bool* marked = calloc(comp->ref_graph->node_index_capacity, sizeof(bool));
    if (!marked) {
        fprintf(stderr, "Warning: Failed to allocate marked array for liveness analysis\n");
        return;
    }

    /* Phase 1: Build root set from type stack and mark reachable nodes */
    if (comp->verbose) {
        printf("  Root set: type_stack_depth=%d\n", comp->type_stack_depth);
    }
    for (int i = 0; i < comp->type_stack_depth; i++) {
        node_id_t node_id = comp->type_stack[i].node_id;
        if (node_id != NODE_ID_INVALID) {
            if (comp->verbose) {
                printf("    Root[%d]: node=%u\n", i, node_id);
            }
            mark_node(comp->ref_graph, node_id, marked);
        }
    }

    /* Phase 2: Mark escaped nodes (FFI, async, closures) */
    for (size_t i = 0; i < comp->ref_graph->node_count; i++) {
        ref_node_t* node = &comp->ref_graph->nodes[i];
        if (node->is_escaped) {
            mark_node(comp->ref_graph, node->node_id, marked);
        }
    }

    /* Phase 3: Emit FREE for unmarked (dead) nodes */
    dict_entry_t* free_prim = dict_lookup(comp->dict, "free");
    if (!free_prim) {
        fprintf(stderr, "Warning: FREE primitive not found\n");
        free(marked);
        return;
    }

    for (size_t i = 0; i < comp->ref_graph->node_count; i++) {
        ref_node_t* node = &comp->ref_graph->nodes[i];
        if (!marked[node->node_id]) {
            /* Dead node - emit FREE for its slot */
            if (node->slot_id >= 0) {
                if (comp->verbose) {
                    printf("    FREE node=%u slot=%d (dead, not reachable)\n",
                           node->node_id, node->slot_id);
                }
                cell_buffer_append(comp->cells, encode_lit(node->slot_id));
                encode_inline_literal(comp->blob, node->slot_id);
                cell_buffer_append(comp->cells, encode_xt(free_prim->addr));
                encode_primitive(comp->blob, free_prim->prim_id);
            }
        }
    }

    free(marked);
}

/* Apply word's type signature to type stack */
static bool apply_signature(compiler_t* comp, type_sig_t* sig) {
    /* Check we have enough inputs */
    if (comp->type_stack_depth < sig->input_count) {
        fprintf(stderr, "Type error: need %d inputs, have %d\n",
                sig->input_count, comp->type_stack_depth);
        return false;
    }

    /* Type variable bindings for unification (a-z) */
    type_id_t bindings[26];
    for (int i = 0; i < 26; i++) {
        bindings[i] = TYPE_UNKNOWN;  /* Unbound */
    }

    /* Collect input types and build bindings */
    type_id_t input_types[MAX_TYPE_STACK];
    for (int i = 0; i < sig->input_count; i++) {
        int stack_pos = comp->type_stack_depth - sig->input_count + i;
        input_types[i] = comp->type_stack[stack_pos].type;
    }

    /* Pop inputs and verify types, binding type variables */
    for (int i = sig->input_count - 1; i >= 0; i--) {
        type_id_t expected = sig->inputs[i];
        type_stack_entry_t entry = pop_type_entry(comp);
        type_id_t actual = entry.type;

        /* Note: In slot model, we don't consume values here.
         * Slots stay allocated until word end. */

        /* Handle type variables */
        if (expected >= TYPE_VAR_A && expected <= TYPE_VAR_Z) {
            int var_idx = expected - TYPE_VAR_A;
            if (bindings[var_idx] == TYPE_UNKNOWN) {
                bindings[var_idx] = actual;
            } else if (bindings[var_idx] != actual) {
                fprintf(stderr, "Type variable binding conflict\n");
                return false;
            }
        } else if (expected != TYPE_ANY && actual != TYPE_ANY && expected != actual) {
            /* Allow mutable variant where immutable is expected (for read-only ops) */
            bool compatible = false;
            if (expected == TYPE_ARRAY && actual == TYPE_ARRAY_MUT) compatible = true;
            if (expected == TYPE_STR && actual == TYPE_STR_MUT) compatible = true;

            if (!compatible) {
                fprintf(stderr, "Type mismatch: expected %d, got %d\n", expected, actual);
                return false;
            }
        }
    }

    /* Push outputs, resolving type variables */
    for (int i = 0; i < sig->output_count; i++) {
        type_id_t output_type = sig->outputs[i];

        /* Resolve type variable to bound type */
        if (output_type >= TYPE_VAR_A && output_type <= TYPE_VAR_Z) {
            int var_idx = output_type - TYPE_VAR_A;
            if (bindings[var_idx] != TYPE_UNKNOWN) {
                output_type = bindings[var_idx];
            }
        }

        push_type(comp, output_type);
    }

    return true;
}

/* Compile a number literal */
static bool compile_number(compiler_t* comp, int64_t num) {
    /* Update crash context */
    char buf[32];
    snprintf(buf, sizeof(buf), "%ld", num);
    crash_context_set_token(buf);
    crash_context_set_stacks(comp->type_stack_depth,
                            comp->quot_stack_depth,
                            comp->buffer_stack_depth);

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

/* Compile a string literal */
static bool compile_string(compiler_t* comp, const char* str) {
    /* Strings are immutable, stored with 32-byte header in DB, no runtime allocation */
    /* Layout: [count: u64][elem_size: u8=1][padding: 7][elem_type: u64][reserved: u64][data...] */

    size_t str_len = strlen(str) + 1;  /* Include null terminator */
    size_t total_size = 32 + str_len;

    /* Build buffer with header + data */
    uint8_t* buffer = calloc(1, total_size);
    if (!buffer) {
        fprintf(stderr, "Error: Failed to allocate string buffer\n");
        return false;
    }

    /* Write header fields */
    uint64_t count = str_len;
    uint8_t elem_size = 1;
    uint64_t elem_type = TYPE_I64;  /* Using TYPE_I64 for now, could add TYPE_U8 */

    memcpy(buffer + 0, &count, 8);      /* count at offset 0 */
    buffer[8] = elem_size;               /* elem_size at offset 8 */
    memcpy(buffer + 16, &elem_type, 8); /* elem_type at offset 16 */
    /* reserved bytes at 24-31 remain zero */

    /* Copy string data after header */
    memcpy(buffer + 32, str, str_len);

    /* Store type signature for string (-> str) */
    unsigned char* sig_cid = db_store_type_sig(comp->db, NULL, "str");
    if (!sig_cid) {
        fprintf(stderr, "Error: Failed to store string type signature\n");
        free(buffer);
        return false;
    }

    /* Store buffer as blob in database */
    unsigned char* str_cid = db_store_blob(comp->db, BLOB_STRING, sig_cid,
                                           buffer, total_size);
    free(sig_cid);
    free(buffer);

    if (!str_cid) {
        fprintf(stderr, "Error: Failed to store string literal in database\n");
        return false;
    }

    /* Emit: CID reference only (loader will cache the blob, return pointer) */
    cell_buffer_append(comp->cells, encode_lit(0));  /* Placeholder for legacy */
    encode_cid_ref(comp->blob, BLOB_DATA, str_cid);
    free(str_cid);

    /* Stack: [str_ptr] - immutable pointer to cached blob */
    push_type(comp, TYPE_STR);

    if (comp->verbose) {
        printf("  STR \"%s\" (%zu bytes + 32 header) → str [immutable]\n", str, str_len);
    }

    return true;
}

/* Compile a word reference */
static bool compile_word(compiler_t* comp, const char* name) {
    fprintf(stderr, "TRACE: compile_word('%s') entry\n", name);
    fflush(stderr);

    /* Update crash context */
    crash_context_set_token(name);
    crash_context_set_stacks(comp->type_stack_depth,
                            comp->quot_stack_depth,
                            comp->buffer_stack_depth);

    fprintf(stderr, "TRACE: Looking up word '%s'\n", name);
    fflush(stderr);

    /* Lookup word (may be immediate, primitive, or user word) */
    dict_entry_t* entry = dict_lookup(comp->dict, name);

    fprintf(stderr, "TRACE: Lookup result: %s\n", entry ? "found" : "not found");
    fflush(stderr);

    if (!entry) {
        fprintf(stderr, "Unknown word: %s\n", name);
        return false;
    }

    /* Check if immediate */
    bool is_immediate = entry->is_immediate;

    fprintf(stderr, "TRACE: is_immediate=%d, quot_depth=%d buffer_depth=%d\n",
            is_immediate, comp->quot_stack_depth, comp->buffer_stack_depth);
    fflush(stderr);

    /* Materialize any pending quotations ONLY for non-immediate words */
    /* Immediate words (like 'times', 'if') consume quotations directly */
    if (!is_immediate && comp->quot_stack_depth > 0 && comp->buffer_stack_depth == 0) {
        fprintf(stderr, "TRACE: Materializing quotations\n");
        fflush(stderr);
        if (!materialize_quotations(comp)) {
            return false;
        }
    }

    fprintf(stderr, "TRACE: Doing type-aware lookup for '%s'\n", name);
    fflush(stderr);

    /* Extract types for type-aware lookup */
    type_id_t types[MAX_TYPE_STACK];
    for (int i = 0; i < comp->type_stack_depth; i++) {
        types[i] = comp->type_stack[i].type;
    }

    /* Do type-aware lookup for overload resolution (works for both immediate and regular) */
    entry = dict_lookup_typed(comp->dict, name,
                             types,
                             comp->type_stack_depth);

    fprintf(stderr, "TRACE: Type-aware lookup result: %s\n", entry ? "found" : "not found");
    fflush(stderr);

    if (!entry) {
        DEBUG_COMPILER("Failed to find match for word: %s", name);
        debug_dump_type_stack("Type stack at failure", comp->type_stack, comp->type_stack_depth);
        fprintf(stderr, "Type error: no matching overload for word: %s\n", name);
        return false;
    }

    /* If immediate word, call its handler (after type-aware selection!) */
    if (entry->is_immediate) {
        fprintf(stderr, "TRACE: Calling immediate handler for '%s'\n", name);
        fflush(stderr);
        if (!entry->handler) {
            fprintf(stderr, "Internal error: immediate word '%s' has no handler\n", name);
            return false;
        }
        return entry->handler(comp);
    }

    /* Design B: Check if this word needs monomorphization */
    if (entry->word_def) {
        /* This word has stored tokens - compile it with concrete types */
        if (comp->verbose) {
            printf("  Monomorphizing '%s' with current stack types\n", name);
        }

        /* Get concrete input types from current type stack */
        int input_count = entry->signature.input_count;
        if (comp->type_stack_depth < input_count) {
            fprintf(stderr, "Type error: '%s' needs %d inputs but stack has %d\n",
                    name, input_count, comp->type_stack_depth);
            return false;
        }

        /* Extract input types (bottom N items on stack) */
        type_id_t concrete_inputs[MAX_TYPE_STACK];
        int start_idx = comp->type_stack_depth - input_count;
        for (int i = 0; i < input_count; i++) {
            concrete_inputs[i] = comp->type_stack[start_idx + i].type;
        }

        /* Phase 3: Check specialization cache */
        unsigned char* cached_cid = specialization_lookup(comp, name, concrete_inputs, input_count);

        unsigned char* cid;
        if (cached_cid) {
            /* Cache hit! Reuse existing specialization */
            if (comp->verbose) {
                printf("  Cache HIT: Reusing specialization of '%s'\n", name);
            }
            cid = malloc(CID_SIZE);
            if (!cid) {
                return false;
            }
            memcpy(cid, cached_cid, CID_SIZE);
        } else {
            /* Cache miss - compile the word with concrete types */
            if (comp->verbose) {
                printf("  Cache MISS: Compiling specialization of '%s'\n", name);
            }

            blob_buffer_t* compiled_blob = word_compile_with_context(comp, entry->word_def,
                                                                      concrete_inputs, input_count);
            if (!compiled_blob) {
                fprintf(stderr, "Failed to compile word '%s' with concrete types\n", name);
                return false;
            }

            /* Store the compiled blob in database */
            /* Build type signature string for this specialization */
            char type_sig_str[256];
            char *p = type_sig_str;
            for (int i = 0; i < input_count; i++) {
                switch (concrete_inputs[i]) {
                    case TYPE_I64: p += sprintf(p, "i64 "); break;
                    case TYPE_U64: p += sprintf(p, "u64 "); break;
                    case TYPE_F64: p += sprintf(p, "f64 "); break;
                    case TYPE_PTR: p += sprintf(p, "ptr "); break;
                    case TYPE_BOOL: p += sprintf(p, "bool "); break;
                    case TYPE_STR: p += sprintf(p, "str "); break;
                    case TYPE_ARRAY: p += sprintf(p, "array "); break;
                    default: p += sprintf(p, "? "); break;
                }
            }
            p += sprintf(p, "-> ");
            for (int i = 0; i < comp->type_stack_depth; i++) {
                type_id_t t = comp->type_stack[i].type;
                switch (t) {
                    case TYPE_I64: p += sprintf(p, "i64 "); break;
                    case TYPE_U64: p += sprintf(p, "u64 "); break;
                    case TYPE_F64: p += sprintf(p, "f64 "); break;
                    case TYPE_PTR: p += sprintf(p, "ptr "); break;
                    case TYPE_BOOL: p += sprintf(p, "bool "); break;
                    case TYPE_STR: p += sprintf(p, "str "); break;
                    case TYPE_ARRAY: p += sprintf(p, "array "); break;
                    default: p += sprintf(p, "? "); break;
                }
            }

            unsigned char* sig_cid = db_store_type_sig(comp->db, NULL, type_sig_str);
            if (!sig_cid) {
                fprintf(stderr, "Failed to store type signature for specialization\n");
                blob_buffer_free(compiled_blob);
                return false;
            }

            cid = db_store_blob(comp->db, BLOB_WORD, sig_cid,
                                compiled_blob->data, compiled_blob->size);
            free(sig_cid);
            blob_buffer_free(compiled_blob);

            if (!cid) {
                fprintf(stderr, "Failed to store compiled specialization\n");
                return false;
            }

            /* Phase 3: Store in specialization cache for future reuse */
            specialization_store(comp, name, concrete_inputs, input_count, cid);
        }

        /* Apply type signature to update type stack */
        if (!apply_signature(comp, &entry->signature)) {
            free(cid);
            fprintf(stderr, "Type error in word: %s\n", name);
            return false;
        }

        /* Emit call to the specialized version */
        cell_buffer_append(comp->cells, encode_xt(NULL));
        encode_cid_ref(comp->blob, BLOB_WORD, cid);
        free(cid);

        if (comp->verbose) {
            printf("  Compiled and stored specialization of '%s'\n", name);
        }

        return true;
    }

    /* Not a stored word - proceed with normal compilation */
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
            DEBUG_COMPILER("Encoding call to user word '%s'", name);
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

/* Free quotation tokens and their strings (for QUOT_LITERAL) */
static void quot_free_tokens(quotation_t* quot) {
    if (quot->tokens) {
        for (int i = 0; i < quot->token_count; i++) {
            if (quot->tokens[i].text) {
                free(quot->tokens[i].text);
            }
        }
        free(quot->tokens);
        quot->tokens = NULL;
        quot->token_count = 0;
        quot->token_capacity = 0;
    }
}

/* Append token to quotation's token array (for QUOT_LITERAL) */
static bool quot_append_token(quotation_t* quot, const token_t* tok) {
    /* Grow array if needed */
    if (quot->token_count >= quot->token_capacity) {
        int new_capacity = quot->token_capacity * 2;
        token_t* new_tokens = realloc(quot->tokens, sizeof(token_t) * new_capacity);
        if (!new_tokens) {
            fprintf(stderr, "Failed to grow quotation token array\n");
            return false;
        }
        quot->tokens = new_tokens;
        quot->token_capacity = new_capacity;
    }

    /* Deep copy the token */
    quot->tokens[quot->token_count] = *tok;
    if (tok->text) {
        quot->tokens[quot->token_count].text = strdup(tok->text);
        if (!quot->tokens[quot->token_count].text) {
            fprintf(stderr, "Failed to copy token text\n");
            return false;
        }
    }
    quot->token_count++;

    return true;
}

/* Compile QUOT_LITERAL with provided type context
 * This takes saved tokens and compiles them using the type stack
 * from the immediate word that consumes the quotation */
static bool quot_compile_with_context(
    compiler_t* comp,
    quotation_t* quot,
    type_id_t* type_stack,
    int type_stack_depth)
{
    if (quot->kind != QUOT_LITERAL) {
        fprintf(stderr, "quot_compile_with_context: quotation is not QUOT_LITERAL\n");
        return false;
    }

    if (comp->verbose) {
        printf("  Compiling QUOT_LITERAL with context: %d tokens, stack depth=%d\n",
               quot->token_count, type_stack_depth);
    }

    /* Create compilation buffers */
    quot->cells = cell_buffer_create();
    quot->blob = blob_buffer_create();
    if (!quot->cells || !quot->blob) {
        if (quot->cells) cell_buffer_free(quot->cells);
        if (quot->blob) blob_buffer_free(quot->blob);
        return false;
    }

    /* Save compiler state */
    cell_buffer_t* saved_cells = comp->cells;
    blob_buffer_t* saved_blob = comp->blob;
    int saved_type_depth = comp->type_stack_depth;
    type_stack_entry_t saved_type_stack[MAX_TYPE_STACK];
    for (int i = 0; i < saved_type_depth; i++) {
        saved_type_stack[i] = comp->type_stack[i];
    }

    /* Set up quotation compilation context */
    comp->cells = quot->cells;
    comp->blob = quot->blob;
    comp->type_stack_depth = type_stack_depth;
    for (int i = 0; i < type_stack_depth; i++) {
        /* Convert type_id_t input to type_stack_entry_t (no heap tracking for inputs) */
        comp->type_stack[i].type = type_stack[i];
        comp->type_stack[i].slot_id = -1;
    }

    /* Compile each token */
    bool success = true;
    for (int i = 0; i < quot->token_count && success; i++) {
        token_t* tok = &quot->tokens[i];

        if (comp->verbose) {
            printf("    compiling token %d: type=%d text='%s'\n",
                   i, tok->type, tok->text ? tok->text : "NULL");
        }

        if (tok->type == TOK_NUMBER) {
            success = compile_number(comp, tok->number);
        } else if (tok->type == TOK_STRING) {
            success = compile_string(comp, tok->text);
        } else if (tok->type == TOK_WORD) {
            success = compile_word(comp, tok->text);
        } else if (tok->type == TOK_LPAREN) {
            success = compile_lparen(comp);
        } else if (tok->type == TOK_RPAREN) {
            success = compile_rparen(comp);
        } else if (tok->type == TOK_LBRACKET) {
            success = compile_lbracket(comp);
        } else if (tok->type == TOK_RBRACKET) {
            success = compile_rbracket(comp);
        } else {
            fprintf(stderr, "Unexpected token type %d in quotation\n", tok->type);
            success = false;
        }
    }

    if (success) {
        /* Emit EXIT */
        cell_buffer_append(quot->cells, encode_exit());

        /* Capture output types */
        quot->output_count = comp->type_stack_depth;
        for (int i = 0; i < comp->type_stack_depth; i++) {
            quot->outputs[i] = comp->type_stack[i].type;
        }

        /* Mark as compiled (upgrade to QUOT_TYPED) */
        quot->kind = QUOT_TYPED;

        if (comp->verbose) {
            printf("  Compiled quotation: ");
            for (int i = 0; i < quot->input_count; i++) {
                printf("%d ", quot->inputs[i]);
            }
            printf("-> ");
            for (int i = 0; i < quot->output_count; i++) {
                printf("%d ", quot->outputs[i]);
            }
            printf("(%zu cells, %zu blob bytes)\n", quot->cells->count, quot->blob->size);
        }
    }

    /* Restore compiler state */
    comp->cells = saved_cells;
    comp->blob = saved_blob;
    comp->type_stack_depth = saved_type_depth;
    for (int i = 0; i < saved_type_depth; i++) {
        comp->type_stack[i] = saved_type_stack[i];
    }

    return success;
}

/* Design B: Compile a word with concrete type context
 * This is called at call-site with known input types to monomorphize the word
 * Phase 5: Made public for on-demand compilation in runner */
blob_buffer_t* word_compile_with_context(compiler_t* comp, word_definition_t* word_def,
                                          type_id_t* input_types, int input_count) {
    fprintf(stderr, "TRACE: word_compile_with_context() entry for '%s'\n", word_def ? word_def->name : "NULL");
    fflush(stderr);

    if (!word_def) {
        fprintf(stderr, "word_compile_with_context: NULL word_def\n");
        return NULL;
    }

    if (comp->verbose) {
        printf("  Compiling '%s' with concrete types: ", word_def->name);
        for (int i = 0; i < input_count; i++) {
            printf("%d ", input_types[i]);
        }
        printf("\n");
    }

    /* Save current compiler state */
    cell_buffer_t* saved_cells = comp->cells;
    blob_buffer_t* saved_blob = comp->blob;
    ref_graph_t* saved_ref_graph = comp->ref_graph;  /* Save ref_graph too! */
    int saved_type_depth = comp->type_stack_depth;
    type_stack_entry_t saved_type_stack[MAX_TYPE_STACK];
    for (int i = 0; i < saved_type_depth; i++) {
        saved_type_stack[i] = comp->type_stack[i];
    }
    int saved_slot_count = comp->slot_count;
    bool saved_slots[MAX_SLOTS];
    memcpy(saved_slots, comp->slot_used, sizeof(saved_slots));

    /* Create fresh buffers for this compilation */
    cell_buffer_t* fresh_cells = cell_buffer_create();
    blob_buffer_t* fresh_blob = blob_buffer_create();
    if (!fresh_cells || !fresh_blob) {
        fprintf(stderr, "Failed to create compilation buffers\n");
        if (fresh_cells) cell_buffer_free(fresh_cells);
        if (fresh_blob) blob_buffer_free(fresh_blob);
        return NULL;
    }

    comp->cells = fresh_cells;
    comp->blob = fresh_blob;

    /* Initialize type stack with concrete input types */
    comp->type_stack_depth = 0;
    comp->slot_count = 0;
    memset(comp->slot_used, 0, sizeof(comp->slot_used));

    /* Initialize reference graph for this word compilation */
    comp->ref_graph = ref_graph_create();
    if (!comp->ref_graph) {
        fprintf(stderr, "Failed to create reference graph\n");
        cell_buffer_free(fresh_cells);
        blob_buffer_free(fresh_blob);
        return NULL;
    }

    for (int i = 0; i < input_count; i++) {
        push_type(comp, input_types[i]);
    }

    /* Compile each token in the word definition */
    bool success = true;
    for (int i = 0; i < word_def->token_count && success; i++) {
        token_t* tok = &word_def->tokens[i];

        /* Compile based on token type */
        switch (tok->type) {
            case TOK_NUMBER:
                success = compile_number(comp, tok->number);
                break;
            case TOK_STRING:
                success = compile_string(comp, tok->text);
                break;
            case TOK_WORD:
                success = compile_word(comp, tok->text);
                break;
            case TOK_LPAREN:
                success = compile_lparen(comp);
                break;
            case TOK_RPAREN:
                success = compile_rparen(comp);
                break;
            case TOK_LBRACKET:
                success = compile_lbracket(comp);
                break;
            case TOK_RBRACKET:
                success = compile_rbracket(comp);
                break;
            default:
                fprintf(stderr, "Unexpected token type %d during word compilation\n", tok->type);
                success = false;
                break;
        }
    }

    blob_buffer_t* result = NULL;

    if (success) {
        /* Materialize any pending quotations */
        if (comp->quot_stack_depth > 0) {
            success = materialize_quotations(comp);
        }
    }

    if (success) {
        fprintf(stderr, "TRACE: About to call emit_free_for_dead_nodes\n");
        fflush(stderr);

        /* Emit FREE for dead nodes via liveness analysis */
        /* TEMPORARILY DISABLED FOR TESTING */
        // emit_free_for_dead_nodes(comp);

        fprintf(stderr, "TRACE: emit_free_for_dead_nodes SKIPPED (disabled for testing)\n");
        fflush(stderr);

        /* Emit EXIT */
        cell_buffer_append(comp->cells, encode_exit());

        /* Return the compiled blob (transfer ownership) */
        result = comp->blob;
        comp->blob = NULL;  /* Prevent double-free */

        /* Clean up cells (not needed - we only return blob) */
        cell_buffer_free(comp->cells);
        comp->cells = NULL;
    } else {
        /* Compilation failed - clean up */
        cell_buffer_free(fresh_cells);
        blob_buffer_free(fresh_blob);
    }

    /* Cleanup reference graph */
    if (comp->ref_graph) {
        ref_graph_free(comp->ref_graph);
    }

    /* Restore compiler state */
    comp->cells = saved_cells;
    comp->blob = saved_blob;
    comp->ref_graph = saved_ref_graph;  /* Restore ref_graph! */
    comp->type_stack_depth = saved_type_depth;
    for (int i = 0; i < saved_type_depth; i++) {
        comp->type_stack[i] = saved_type_stack[i];
    }
    comp->slot_count = saved_slot_count;
    memcpy(comp->slot_used, saved_slots, sizeof(saved_slots));

    return result;
}

/* Start quotation - save current state and begin new compilation */
static bool compile_lparen(compiler_t* comp) {
    crash_context_set_token("(");
    crash_context_set_stacks(comp->type_stack_depth,
                            comp->quot_stack_depth,
                            comp->buffer_stack_depth);

    if (comp->quot_stack_depth >= MAX_QUOT_DEPTH) {
        fprintf(stderr, "Quotation nesting too deep (max %d)\n", MAX_QUOT_DEPTH);
        return false;
    }

    /* Create new quotation as QUOT_LITERAL (lexical quotation) */
    quotation_t* quot = malloc(sizeof(quotation_t));
    if (!quot) return false;

    /* Initialize as QUOT_LITERAL - stores tokens, compiles at use site */
    quot->kind = QUOT_LITERAL;
    quot->token_capacity = 16;  /* Initial capacity */
    quot->token_count = 0;
    quot->tokens = malloc(sizeof(token_t) * quot->token_capacity);
    if (!quot->tokens) {
        free(quot);
        return false;
    }

    /* No cells/blob buffers for QUOT_LITERAL - compiled later at use site */
    quot->cells = NULL;
    quot->blob = NULL;

    /* Capture current type stack as quotation inputs */
    quot->input_count = comp->type_stack_depth;
    for (int i = 0; i < comp->type_stack_depth; i++) {
        quot->inputs[i] = comp->type_stack[i].type;
    }

    /* Output types unknown until compilation at use site */
    quot->output_count = 0;

    /* Push quotation onto stack */
    comp->quot_stack[comp->quot_stack_depth++] = quot;

    /* Increment buffer stack depth to signal we're inside a quotation,
     * but don't switch buffers - tokens will be captured instead */
    comp->buffer_stack_depth++;

    if (comp->verbose) {
        printf("  ( start QUOT_LITERAL (capture tokens, depth=%d)\n", comp->buffer_stack_depth);
    }

    return true;
}

/* End quotation - finalize and push onto quotation stack */
static bool compile_rparen(compiler_t* comp) {
    crash_context_set_token(")");
    crash_context_set_stacks(comp->type_stack_depth,
                            comp->quot_stack_depth,
                            comp->buffer_stack_depth);

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

    if (quot->kind == QUOT_LITERAL) {
        /* QUOT_LITERAL: tokens are already captured, just finalize */
        comp->buffer_stack_depth--;

        /* Restore type stack to pre-quotation state */
        comp->type_stack_depth = quot->input_count;
        for (int i = 0; i < quot->input_count; i++) {
            comp->type_stack[i].type = quot->inputs[i];
            comp->type_stack[i].slot_id = -1;
        }

        if (comp->verbose) {
            printf("  ) end QUOT_LITERAL: %d tokens captured\n", quot->token_count);
        }
    } else {
        /* QUOT_TYPED: compile immediately (future feature) */
        /* Emit EXIT */
        cell_buffer_append(quot->cells, encode_exit());

        /* Capture output types */
        quot->output_count = comp->type_stack_depth;
        for (int i = 0; i < comp->type_stack_depth; i++) {
            quot->outputs[i] = comp->type_stack[i].type;
        }

        /* Restore parent buffers */
        comp->cells = comp->buffer_stack[--comp->buffer_stack_depth];
        comp->blob = comp->blob_stack[--comp->blob_stack_depth];

        /* Restore type stack (quotation inputs + outputs become new stack) */
        comp->type_stack_depth = quot->input_count;
        for (int i = 0; i < quot->input_count; i++) {
            comp->type_stack[i].type = quot->inputs[i];
            comp->type_stack[i].slot_id = -1;
        }

        if (comp->verbose) {
            printf("  ) end QUOT_TYPED: ");
            for (int i = 0; i < quot->input_count; i++) {
                printf("%d ", quot->inputs[i]);
            }
            printf("-> ");
            for (int i = 0; i < quot->output_count; i++) {
                printf("%d ", quot->outputs[i]);
            }
            printf(" (%zu cells, %zu blob bytes)\n", quot->cells->count, quot->blob->size);
        }
    }

    /* Note: quotation stays on quot_stack for immediate word to consume */

    return true;
}

/* Start array literal [ - mark current stack depth as boundary */
static bool compile_lbracket(compiler_t* comp) {
    crash_context_set_token("[");
    crash_context_set_stacks(comp->type_stack_depth,
                            comp->quot_stack_depth,
                            comp->buffer_stack_depth);

    if (comp->array_marker_depth >= MAX_ARRAY_DEPTH) {
        fprintf(stderr, "Array nesting too deep (max %d)\n", MAX_ARRAY_DEPTH);
        return false;
    }

    /* Record current stack depth as the marker boundary */
    comp->array_marker_stack[comp->array_marker_depth] = comp->type_stack_depth;

    /* Initialize consumption counter for _ (underscore) tracking */
    comp->array_consumed_count[comp->array_marker_depth] = 0;

    comp->array_marker_depth++;

    if (comp->verbose) {
        printf("  [ mark array boundary at depth %d\n", comp->type_stack_depth);
    }

    return true;
}

/* End array literal ] - collect items and create array */
static bool compile_rbracket(compiler_t* comp) {
    crash_context_set_token("]");
    crash_context_set_stacks(comp->type_stack_depth,
                            comp->quot_stack_depth,
                            comp->buffer_stack_depth);

    if (comp->array_marker_depth == 0) {
        fprintf(stderr, "Unmatched ']' - no array literal in progress\n");
        return false;
    }

    /* Get marker depth */
    comp->array_marker_depth--;
    int marker_depth = comp->array_marker_stack[comp->array_marker_depth];
    int elem_count = comp->type_stack_depth - marker_depth;

    if (comp->verbose) {
        printf("  ] collect %d array elements from depth %d\n", elem_count, marker_depth);
    }

    /* Handle empty array case - runtime allocated (semantically immutable) */
    if (elem_count == 0) {
        /* Empty array: allocate header (32 bytes) with count=0 */
        dict_entry_t* alloc_prim = dict_lookup(comp->dict, "alloc");
        dict_entry_t* store_prim = dict_lookup(comp->dict, "!");
        if (!alloc_prim || !store_prim) {
            fprintf(stderr, "Internal error: ALLOC or STORE primitive not found\n");
            return false;
        }

        /* Allocate 32 bytes for header */
        cell_buffer_append(comp->cells, encode_lit(32));
        unsigned char* size_cid = db_store_literal(comp->db, 32, "i64");
        if (!size_cid) {
            fprintf(stderr, "Error: Failed to store array header size\n");
            return false;
        }
        encode_cid_ref(comp->blob, BLOB_DATA, size_cid);
        free(size_cid);

        /* Call ALLOC - stack: [ptr] */
        cell_buffer_append(comp->cells, encode_xt(alloc_prim->addr));
        encode_primitive(comp->blob, alloc_prim->prim_id);

        /* Write header: [count=0][elem_size=8][padding][elem_type=TYPE_ANY] */
        /* Store count=0 at offset 0 */
        dict_entry_t* dup_prim = dict_lookup(comp->dict, "dup");
        if (!dup_prim) return false;

        cell_buffer_append(comp->cells, encode_xt(dup_prim->addr));
        encode_primitive(comp->blob, dup_prim->prim_id);
        cell_buffer_append(comp->cells, encode_lit(0));
        size_cid = db_store_literal(comp->db, 0, "i64");
        encode_cid_ref(comp->blob, BLOB_DATA, size_cid);
        free(size_cid);

        /* Need SWAP to get correct stack order for STORE */
        /* Stack is [ptr, ptr, 0], need [ptr, 0, ptr] for STORE (value addr --) */
        dict_entry_t* swap_prim = dict_lookup(comp->dict, "swap");
        if (!swap_prim) return false;
        cell_buffer_append(comp->cells, encode_xt(swap_prim->addr));
        encode_primitive(comp->blob, swap_prim->prim_id);

        cell_buffer_append(comp->cells, encode_xt(store_prim->addr));
        encode_primitive(comp->blob, store_prim->prim_id);

        /* Update type stack: push array pointer */
        /* Note: Arrays are semantically immutable - use 'mut' for mutable copy */
        comp->type_stack_depth = marker_depth;
        push_type(comp, TYPE_ARRAY);

        if (comp->verbose) {
            printf("  ] created empty array (semantically immutable) → array\n");
        }

        return true;
    }

    /* Check if all elements have the same type (homogeneous array) */
    type_id_t elem_type = comp->type_stack[marker_depth].type;
    bool homogeneous = true;
    for (int i = marker_depth + 1; i < comp->type_stack_depth; i++) {
        if (comp->type_stack[i].type != elem_type) {
            homogeneous = false;
            break;
        }
    }

    if (!homogeneous) {
        fprintf(stderr, "Heterogeneous tuples not yet supported\n");
        return false;
    }

    /* Look up ALLOC primitive */
    dict_entry_t* alloc_prim = dict_lookup(comp->dict, "alloc");
    if (!alloc_prim) {
        fprintf(stderr, "Internal error: ALLOC primitive not found\n");
        return false;
    }

    /* Look up STORE primitive */
    dict_entry_t* store_prim = dict_lookup(comp->dict, "!");
    if (!store_prim) {
        fprintf(stderr, "Internal error: STORE (!) primitive not found\n");
        return false;
    }

    /* Calculate array size in bytes: 32-byte header + elem_count * 8 */
    int64_t data_size = elem_count * 8;
    int64_t total_size = 32 + data_size;

    /* Emit size literal and ALLOC call */
    /* Legacy: Emit LIT cell */
    cell_buffer_append(comp->cells, encode_lit(total_size));

    /* CID-based: Store literal and encode reference */
    unsigned char* size_cid = db_store_literal(comp->db, total_size, "i64");
    if (!size_cid) {
        fprintf(stderr, "Error: Failed to store array size literal\n");
        return false;
    }
    encode_cid_ref(comp->blob, BLOB_DATA, size_cid);
    free(size_cid);

    /* Call ALLOC primitive */
    cell_buffer_append(comp->cells, encode_xt(alloc_prim->addr));
    encode_primitive(comp->blob, alloc_prim->prim_id);

    /* Now we have: elem[0] elem[1] ... elem[n-1] array_ptr on runtime stack */
    /* Strategy: Write header, then move ptr to return stack, store elements, restore ptr */

    if (comp->verbose) {
        printf("    Allocating array: 32-byte header + %d elements * 8 = %ld bytes\n", elem_count, total_size);
    }

    /* Look up return stack primitives */
    dict_entry_t* tor_prim = dict_lookup(comp->dict, ">r");
    dict_entry_t* fromr_prim = dict_lookup(comp->dict, "r>");
    dict_entry_t* rfetch_prim = dict_lookup(comp->dict, "r@");
    dict_entry_t* add_prim = dict_lookup(comp->dict, "+");
    dict_entry_t* dup_prim = dict_lookup(comp->dict, "dup");
    dict_entry_t* cstore_prim = dict_lookup(comp->dict, "c!");

    if (!tor_prim || !fromr_prim || !rfetch_prim || !add_prim || !dup_prim || !cstore_prim) {
        fprintf(stderr, "Internal error: Primitives not found\n");
        return false;
    }

    /* Write header: [count: u64][elem_size: u8][padding: 7][elem_type: u64][reserved: u64] */
    /* Stack currently: elem[0] elem[1] ... elem[n-1] ptr */

    /* Write count at offset 0 */
    cell_buffer_append(comp->cells, encode_xt(dup_prim->addr));
    encode_primitive(comp->blob, dup_prim->prim_id);
    cell_buffer_append(comp->cells, encode_lit(elem_count));
    size_cid = db_store_literal(comp->db, elem_count, "i64");
    encode_cid_ref(comp->blob, BLOB_DATA, size_cid);
    free(size_cid);
    /* Need SWAP to get correct stack order for STORE (value addr --) */
    dict_entry_t* swap_prim_count = dict_lookup(comp->dict, "swap");
    if (!swap_prim_count) return false;
    cell_buffer_append(comp->cells, encode_xt(swap_prim_count->addr));
    encode_primitive(comp->blob, swap_prim_count->prim_id);
    cell_buffer_append(comp->cells, encode_xt(store_prim->addr));
    encode_primitive(comp->blob, store_prim->prim_id);

    /* Write elem_size=8 at offset 8 using c! */
    cell_buffer_append(comp->cells, encode_xt(dup_prim->addr));
    encode_primitive(comp->blob, dup_prim->prim_id);
    cell_buffer_append(comp->cells, encode_lit(8));
    size_cid = db_store_literal(comp->db, 8, "i64");
    encode_cid_ref(comp->blob, BLOB_DATA, size_cid);
    free(size_cid);
    cell_buffer_append(comp->cells, encode_xt(add_prim->addr));
    encode_primitive(comp->blob, add_prim->prim_id);
    cell_buffer_append(comp->cells, encode_lit(8));
    size_cid = db_store_literal(comp->db, 8, "i64");
    encode_cid_ref(comp->blob, BLOB_DATA, size_cid);
    free(size_cid);
    dict_entry_t* swap_prim = dict_lookup(comp->dict, "swap");
    if (!swap_prim) return false;
    cell_buffer_append(comp->cells, encode_xt(swap_prim->addr));
    encode_primitive(comp->blob, swap_prim->prim_id);
    cell_buffer_append(comp->cells, encode_xt(cstore_prim->addr));
    encode_primitive(comp->blob, cstore_prim->prim_id);

    /* Write elem_type at offset 16 */
    cell_buffer_append(comp->cells, encode_xt(dup_prim->addr));
    encode_primitive(comp->blob, dup_prim->prim_id);
    cell_buffer_append(comp->cells, encode_lit(16));
    size_cid = db_store_literal(comp->db, 16, "i64");
    encode_cid_ref(comp->blob, BLOB_DATA, size_cid);
    free(size_cid);
    cell_buffer_append(comp->cells, encode_xt(add_prim->addr));
    encode_primitive(comp->blob, add_prim->prim_id);
    cell_buffer_append(comp->cells, encode_lit(elem_type));
    size_cid = db_store_literal(comp->db, elem_type, "i64");
    encode_cid_ref(comp->blob, BLOB_DATA, size_cid);
    free(size_cid);
    /* Need SWAP to get correct stack order for STORE (value addr --) */
    dict_entry_t* swap_prim_type = dict_lookup(comp->dict, "swap");
    if (!swap_prim_type) return false;
    cell_buffer_append(comp->cells, encode_xt(swap_prim_type->addr));
    encode_primitive(comp->blob, swap_prim_type->prim_id);
    cell_buffer_append(comp->cells, encode_xt(store_prim->addr));
    encode_primitive(comp->blob, store_prim->prim_id);

    /* Move array pointer to return stack: >r */
    /* Stack: elem[0] elem[1] ... elem[n-1] */
    /* R-stack: ptr */
    cell_buffer_append(comp->cells, encode_xt(tor_prim->addr));
    encode_primitive(comp->blob, tor_prim->prim_id);

    /* Store each element in reverse order (from TOS down) */
    /* elem[n-1] is on TOS and should go to offset 32 + (n-1)*8 */
    for (int i = elem_count - 1; i >= 0; i--) {
        int64_t offset = 32 + (i * 8);

        /* Fetch array pointer from return stack: r@ */
        /* Stack: elem[0] ... elem[i] ptr */
        cell_buffer_append(comp->cells, encode_xt(rfetch_prim->addr));
        encode_primitive(comp->blob, rfetch_prim->prim_id);

        /* Push offset literal */
        cell_buffer_append(comp->cells, encode_lit(offset));
        unsigned char* offset_cid = db_store_literal(comp->db, offset, "i64");
        if (!offset_cid) {
            fprintf(stderr, "Error: Failed to store offset literal\n");
            return false;
        }
        encode_cid_ref(comp->blob, BLOB_DATA, offset_cid);
        free(offset_cid);

        /* Stack: elem[0] ... elem[i] ptr offset */

        /* Add offset to pointer: + */
        /* Stack: elem[0] ... elem[i] (ptr+offset) */
        cell_buffer_append(comp->cells, encode_xt(add_prim->addr));
        encode_primitive(comp->blob, add_prim->prim_id);

        /* Stack: elem[0] ... elem[i] (ptr+offset) */
        /* STORE expects (value addr --), so elem[i] at TOS-1, (ptr+offset) at TOS */
        /* This is already correct - NO SWAP needed */

        /* Store: ! (consumes value and address) */
        /* Stack: elem[0] ... elem[i-1] */
        cell_buffer_append(comp->cells, encode_xt(store_prim->addr));
        encode_primitive(comp->blob, store_prim->prim_id);

        if (comp->verbose) {
            printf("    Store element %d at offset %ld\n", i, offset);
        }
    }

    /* Restore array pointer from return stack: r> */
    /* Stack: <consumed-values> ptr */
    cell_buffer_append(comp->cells, encode_xt(fromr_prim->addr));
    encode_primitive(comp->blob, fromr_prim->prim_id);

    /* Calculate how many values before marker were consumed by _ */
    int consumed = comp->array_consumed_count[comp->array_marker_depth];

    /* If we consumed values, drop them now */
    /* Stack is: val1 val2 ... valN ptr */
    /* We want: ptr */
    /* Strategy: >r drop drop ... r> */
    if (consumed > 0) {
        dict_entry_t* tor_prim = dict_lookup(comp->dict, ">r");
        dict_entry_t* drop_prim = dict_lookup(comp->dict, "drop");
        if (!tor_prim || !drop_prim || !fromr_prim) {
            fprintf(stderr, "Internal error: >r, drop, or r> primitive not found\n");
            return false;
        }

        /* Move ptr to return stack: >r */
        cell_buffer_append(comp->cells, encode_xt(tor_prim->addr));
        encode_primitive(comp->blob, tor_prim->prim_id);

        /* Drop consumed values: drop drop ... */
        for (int i = 0; i < consumed; i++) {
            cell_buffer_append(comp->cells, encode_xt(drop_prim->addr));
            encode_primitive(comp->blob, drop_prim->prim_id);
        }

        /* Restore ptr: r> */
        cell_buffer_append(comp->cells, encode_xt(fromr_prim->addr));
        encode_primitive(comp->blob, fromr_prim->prim_id);
    }

    /* Update type stack: remove all elements, push array pointer */
    /* First, collect node_ids from array elements (for parent→child edges) */
    node_id_t child_nodes[MAX_TYPE_STACK];
    int child_count = 0;

    fprintf(stderr, "TRACE: compile_rbracket collecting child nodes, ref_graph=%p\n", (void*)comp->ref_graph);
    fflush(stderr);

    if (comp->ref_graph) {
        for (int i = marker_depth; i < comp->type_stack_depth; i++) {
            node_id_t child_node = comp->type_stack[i].node_id;
            if (child_node != NODE_ID_INVALID) {
                child_nodes[child_count++] = child_node;
            }
        }
        fprintf(stderr, "TRACE: collected %d child nodes\n", child_count);
        fflush(stderr);
    }

    fprintf(stderr, "TRACE: compile_rbracket resetting stack depth from %d to %d (consumed %d)\n",
            comp->type_stack_depth, marker_depth - consumed, consumed);
    fflush(stderr);

    /* Reset type stack, removing array elements AND consumed pre-marker values */
    comp->type_stack_depth = marker_depth - consumed;

    fprintf(stderr, "TRACE: compile_rbracket calling push_heap_value\n");
    fflush(stderr);

    /* TEMPORARILY REVERT TO ORIGINAL push_type FOR TESTING */
    push_type(comp, TYPE_ARRAY);
    // push_heap_value(comp, TYPE_ARRAY);

    fprintf(stderr, "TRACE: compile_rbracket push_type returned\n");
    fflush(stderr);

    /* Establish parent→child edges for nested structures */
    if (comp->ref_graph && child_count > 0) {
        node_id_t array_node = comp->type_stack[marker_depth].node_id;
        if (array_node != NODE_ID_INVALID) {
            for (int i = 0; i < child_count; i++) {
                ref_graph_add_child(comp->ref_graph, array_node, child_nodes[i]);
            }
            if (comp->verbose) {
                printf("  ] established %d parent→child edges\n", child_count);
            }
        }
    }

    if (comp->verbose) {
        printf("  ] created array of %d elements → array\n", elem_count);
    }

    fprintf(stderr, "TRACE: compile_rbracket returning true\n");
    fflush(stderr);

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

        /* Compile QUOT_LITERAL with empty type context before materializing */
        if (quot->kind == QUOT_LITERAL) {
            if (comp->verbose) {
                printf("  Materializing QUOT_LITERAL: compiling with empty context\n");
            }
            /* Empty context - no types needed */
            if (!quot_compile_with_context(comp, quot, NULL, 0)) {
                fprintf(stderr, "Failed to compile QUOT_LITERAL for materialization\n");
                return false;
            }
        }

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
            quot_free_tokens(quot);
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
            quot_free_tokens(quot);
            free(quot);
            return false;
        }

        /* Track CID for linking */
        if (comp->pending_quot_count >= MAX_QUOT_REFS) {
            fprintf(stderr, "Too many quotation references in word (max %d)\n", MAX_QUOT_REFS);
            free(cid);
            cell_buffer_free(quot->cells);
            blob_buffer_free(quot->blob);
            quot_free_tokens(quot);
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
        quot_free_tokens(quot);
        free(quot);
    }

    return true;
}

/* Dispatch times based on quotation stack depth */
static bool compile_times_dispatch(compiler_t* comp) {
    if (comp->quot_stack_depth >= 2) {
        /* Two quotations: until-style loop */
        return compile_times_until(comp);
    } else if (comp->quot_stack_depth >= 1) {
        /* One quotation: counted loop */
        return compile_times(comp);
    } else {
        fprintf(stderr, "times requires at least one quotation\n");
        return false;
    }
}

/* Immediate word: times - compile counted loop with inlined quotation */
static bool compile_times(compiler_t* comp) {
    /* Stack should be: count ( -- ) times */
    /* Quotation body can access loop counter via i0 word */
    /* Pop quotation */

    if (comp->quot_stack_depth < 1) {
        fprintf(stderr, "times requires a quotation: count ( body ) times\n");
        return false;
    }

    quotation_t* body_quot = pop_quotation(comp);

    if (!body_quot) {
        fprintf(stderr, "Internal error: quotation not available for times\n");
        return false;
    }

    /* Check that count is on type stack */
    if (comp->type_stack_depth < 1) {
        fprintf(stderr, "times requires a count on stack\n");
        return false;
    }

    /* Pop count type */
    pop_type(comp);

    /* Compile QUOT_LITERAL quotation with current type context (after popping count) */
    if (body_quot->kind == QUOT_LITERAL) {
        /* Extract types for compilation context */
        type_id_t types[MAX_TYPE_STACK];
        for (int i = 0; i < comp->type_stack_depth; i++) {
            types[i] = comp->type_stack[i].type;
        }
        if (!quot_compile_with_context(comp, body_quot, types, comp->type_stack_depth)) {
            fprintf(stderr, "Failed to compile body quotation with context\n");
            return false;
        }
    }

    if (comp->verbose) {
        printf("  TIMES compiling with body=%zu cells (%zu blob bytes)\n",
               body_quot->cells->count, body_quot->blob->size);
    }

    /* Look up primitives we need */
    dict_entry_t* tor_entry = dict_lookup(comp->dict, ">r");
    dict_entry_t* fromr_entry = dict_lookup(comp->dict, "r>");
    dict_entry_t* rfetch_entry = dict_lookup(comp->dict, "r@");
    dict_entry_t* rdrop_entry = dict_lookup(comp->dict, "rdrop");
    dict_entry_t* sub_entry = dict_lookup(comp->dict, "-");
    dict_entry_t* zerop_entry = dict_lookup(comp->dict, "0=");
    dict_entry_t* zbranch_entry = dict_lookup(comp->dict, "0branch");
    dict_entry_t* branch_entry = dict_lookup(comp->dict, "branch");

    if (!tor_entry || !fromr_entry || !rfetch_entry || !rdrop_entry ||
        !sub_entry || !zerop_entry || !zbranch_entry || !branch_entry) {
        fprintf(stderr, "Internal error: loop primitives not registered\n");
        return false;
    }

    /* ===== Legacy cell encoding ===== */

    /* Move count to return stack */
    cell_buffer_append(comp->cells, encode_xt(tor_entry->addr));

    /* Loop start */
    size_t loop_start_pos = comp->cells->count;

    /* Check if count is zero: r@ */
    cell_buffer_append(comp->cells, encode_xt(rfetch_entry->addr));

    /* If zero, branch to done */
    cell_buffer_append(comp->cells, encode_xt(zbranch_entry->addr));
    size_t exit_branch_pos = comp->cells->count;
    cell_buffer_append(comp->cells, encode_lit(0));  /* Placeholder */

    /* Decrement counter: r> 1 - >r */
    cell_buffer_append(comp->cells, encode_xt(fromr_entry->addr));
    cell_buffer_append(comp->cells, encode_lit(1));
    cell_buffer_append(comp->cells, encode_xt(sub_entry->addr));
    cell_buffer_append(comp->cells, encode_xt(tor_entry->addr));

    /* Inline quotation body (minus EXIT) - can access counter via i0 */
    for (size_t i = 0; i < body_quot->cells->count - 1; i++) {
        cell_buffer_append(comp->cells, body_quot->cells->cells[i]);
    }

    /* Branch back to loop start */
    cell_buffer_append(comp->cells, encode_xt(branch_entry->addr));
    int64_t loop_offset = (int64_t)(loop_start_pos - comp->cells->count - 1);
    cell_buffer_append(comp->cells, encode_lit(loop_offset));

    /* Done: calculate exit branch offset */
    int64_t exit_offset = (int64_t)(comp->cells->count - exit_branch_pos - 1);
    comp->cells->cells[exit_branch_pos] = encode_lit(exit_offset);

    /* Clean up return stack */
    cell_buffer_append(comp->cells, encode_xt(rdrop_entry->addr));

    /* ===== CID-based blob encoding ===== */

    /* Move count to return stack */
    encode_primitive(comp->blob, tor_entry->prim_id);

    /* Check if count is zero: r@ */
    encode_primitive(comp->blob, rfetch_entry->prim_id);

    /* 0branch to exit */
    encode_primitive(comp->blob, zbranch_entry->prim_id);

    /* We need to know the offset in cells, not bytes! */
    /* Calculate: body cells + decrement(4) + branch_back(2) */
    int64_t exit_cells_offset = (int64_t)(body_quot->cells->count - 1 + 6);
    encode_inline_literal(comp->blob, exit_cells_offset);

    /* Decrement counter: r> 1 - >r */
    encode_primitive(comp->blob, fromr_entry->prim_id);
    encode_inline_literal(comp->blob, 1);
    encode_primitive(comp->blob, sub_entry->prim_id);
    encode_primitive(comp->blob, tor_entry->prim_id);

    /* Inline quotation body - can access counter via i0 */
    if (comp->verbose) {
        printf("  TIMES inlining body: %zu blob bytes\n", body_quot->blob->size);
    }
    blob_buffer_append_bytes(comp->blob, body_quot->blob->data, body_quot->blob->size);

    /* Branch back to loop start */
    encode_primitive(comp->blob, branch_entry->prim_id);

    /* Calculate backward offset in cells */
    /* From after the branch offset LIT back to loop_start */
    /* We've emitted: r@ 0branch offset r> 1 - >r <body> branch */
    /* Loop start is at: r@ (first check) */
    /* Current position (after branch prim): need to count cells */
    int64_t back_cells = -(int64_t)(1 + 1 + 1 + 1 + 1 + 1 + 1 + (body_quot->cells->count - 1) + 1 + 1);
    encode_inline_literal(comp->blob, back_cells);

    /* Clean up return stack */
    encode_primitive(comp->blob, rdrop_entry->prim_id);

    /* Apply quotation output types */
    /* Quotation body can access loop counter via i0, but doesn't take it as input */
    /* Type stack effect depends on what the quotation body does */

    /* Free quotation */
    cell_buffer_free(body_quot->cells);
    blob_buffer_free(body_quot->blob);
    quot_free_tokens(body_quot);
    free(body_quot);

    if (comp->verbose) {
        printf("  TIMES compiled\n");
    }

    return true;
}

/* Immediate word: times (quotation-based) - compile until-style loop */
static bool compile_times_until(compiler_t* comp) {
    /* Stack should be: ( condition ) ( body ) times */
    /* Execute body, then condition; repeat until condition returns true */

    if (comp->quot_stack_depth < 2) {
        fprintf(stderr, "times requires two quotations: ( condition ) ( body ) times\n");
        return false;
    }

    quotation_t* body_quot = pop_quotation(comp);
    quotation_t* cond_quot = pop_quotation(comp);

    if (!body_quot || !cond_quot) {
        fprintf(stderr, "Internal error: quotations not available for times\n");
        return false;
    }

    /* Extract types for compilation context */
    type_id_t types[MAX_TYPE_STACK];
    for (int i = 0; i < comp->type_stack_depth; i++) {
        types[i] = comp->type_stack[i].type;
    }

    /* Compile QUOT_LITERAL quotations with current type context */
    if (cond_quot->kind == QUOT_LITERAL) {
        if (!quot_compile_with_context(comp, cond_quot, types, comp->type_stack_depth)) {
            fprintf(stderr, "Failed to compile condition quotation with context\n");
            return false;
        }
    }

    if (body_quot->kind == QUOT_LITERAL) {
        if (!quot_compile_with_context(comp, body_quot, types, comp->type_stack_depth)) {
            fprintf(stderr, "Failed to compile body quotation with context\n");
            return false;
        }
    }

    if (comp->verbose) {
        printf("  TIMES-UNTIL compiling with cond=%zu cells, body=%zu cells\n",
               cond_quot->cells->count, body_quot->cells->count);
    }

    /* Look up branch primitives */
    dict_entry_t* zbranch_entry = dict_lookup(comp->dict, "0branch");
    dict_entry_t* branch_entry = dict_lookup(comp->dict, "branch");

    if (!zbranch_entry || !branch_entry) {
        fprintf(stderr, "Internal error: branch primitives not registered\n");
        return false;
    }

    /* ===== Legacy cell encoding ===== */
    /* Loop start */
    size_t loop_start_pos = comp->cells->count;

    /* Inline body quotation (minus EXIT) */
    for (size_t i = 0; i < body_quot->cells->count - 1; i++) {
        cell_buffer_append(comp->cells, body_quot->cells->cells[i]);
    }

    /* Inline condition quotation (minus EXIT) */
    for (size_t i = 0; i < cond_quot->cells->count - 1; i++) {
        cell_buffer_append(comp->cells, cond_quot->cells->cells[i]);
    }

    /* 0branch back to loop start if condition is false (0) */
    cell_buffer_append(comp->cells, encode_xt(zbranch_entry->addr));
    int64_t loop_offset = (int64_t)(loop_start_pos - comp->cells->count - 1);
    cell_buffer_append(comp->cells, encode_lit(loop_offset));

    /* ===== CID-based blob encoding ===== */
    /* Loop start marker */
    size_t blob_loop_start = comp->blob->size;

    /* Inline body quotation */
    if (comp->verbose) {
        printf("  TIMES-UNTIL inlining body: %zu blob bytes\n", body_quot->blob->size);
    }
    blob_buffer_append_bytes(comp->blob, body_quot->blob->data, body_quot->blob->size);

    /* Inline condition quotation */
    if (comp->verbose) {
        printf("  TIMES-UNTIL inlining condition: %zu blob bytes\n", cond_quot->blob->size);
    }
    blob_buffer_append_bytes(comp->blob, cond_quot->blob->data, cond_quot->blob->size);

    /* 0branch back to loop start if condition is false */
    encode_primitive(comp->blob, zbranch_entry->prim_id);

    /* Calculate backward offset in cells */
    /* From after the branch offset back to loop_start */
    /* We've emitted: <body> <cond> 0branch */
    int64_t back_cells = -(int64_t)((body_quot->cells->count - 1) +
                                    (cond_quot->cells->count - 1) +
                                    1 + 1);  /* 0branch prim + offset */
    encode_inline_literal(comp->blob, back_cells);

    /* Free quotations */
    cell_buffer_free(body_quot->cells);
    blob_buffer_free(body_quot->blob);
    quot_free_tokens(body_quot);
    free(body_quot);
    cell_buffer_free(cond_quot->cells);
    blob_buffer_free(cond_quot->blob);
    quot_free_tokens(cond_quot);
    free(cond_quot);

    if (comp->verbose) {
        printf("  TIMES-UNTIL compiled\n");
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

    /* Extract types for compilation context */
    type_id_t types[MAX_TYPE_STACK];
    for (int i = 0; i < comp->type_stack_depth; i++) {
        types[i] = comp->type_stack[i].type;
    }

    /* Compile QUOT_LITERAL quotations with current type context (after popping flag) */
    if (true_quot->kind == QUOT_LITERAL) {
        if (!quot_compile_with_context(comp, true_quot, types, comp->type_stack_depth)) {
            fprintf(stderr, "Failed to compile true quotation with context\n");
            return false;
        }
    }

    if (false_quot->kind == QUOT_LITERAL) {
        if (!quot_compile_with_context(comp, false_quot, types, comp->type_stack_depth)) {
            fprintf(stderr, "Failed to compile false quotation with context\n");
            return false;
        }
    }

    if (comp->verbose) {
        printf("  IF compiling with true=%zu cells (%zu blob bytes), false=%zu cells (%zu blob bytes)\n",
               true_quot->cells->count, true_quot->blob->size,
               false_quot->cells->count, false_quot->blob->size);
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

    /* ===== CID-based blob encoding ===== */
    /* Emit 0BRANCH primitive */
    encode_primitive(comp->blob, zbranch_entry->prim_id);

    /* Calculate and emit 0branch offset */
    /* Offset = true branch size + 2 (for BRANCH primitive + its offset) */
    int64_t zbranch_offset_blob = (int64_t)(true_quot->cells->count - 1 + 2);
    encode_inline_literal(comp->blob, zbranch_offset_blob);

    /* Inline true quotation blob data */
    if (comp->verbose) {
        printf("  IF inlining true branch: %zu blob bytes\n", true_quot->blob->size);
    }
    blob_buffer_append_bytes(comp->blob, true_quot->blob->data, true_quot->blob->size);

    /* Emit BRANCH primitive */
    encode_primitive(comp->blob, branch_entry->prim_id);

    /* Calculate and emit branch offset */
    /* Offset = false branch size */
    int64_t branch_offset_blob = (int64_t)(false_quot->cells->count - 1);
    encode_inline_literal(comp->blob, branch_offset_blob);

    /* Inline false quotation blob data */
    if (comp->verbose) {
        printf("  IF inlining false branch: %zu blob bytes\n", false_quot->blob->size);
    }
    blob_buffer_append_bytes(comp->blob, false_quot->blob->data, false_quot->blob->size);

    /* Apply quotation output types to current stack */
    /* Both branches should have same output types - use true_quot */
    for (int i = 0; i < true_quot->output_count; i++) {
        push_type(comp, true_quot->outputs[i]);
    }

    /* Free quotations */
    cell_buffer_free(true_quot->cells);
    blob_buffer_free(true_quot->blob);
    quot_free_tokens(true_quot);
    free(true_quot);
    cell_buffer_free(false_quot->cells);
    blob_buffer_free(false_quot->blob);
    quot_free_tokens(false_quot);
    free(false_quot);

    if (comp->verbose) {
        printf("  IF compiled: 0branch offset=%lld, branch offset=%lld\n",
               (long long)zbranch_offset, (long long)branch_offset);
    }

    return true;
}

/* Immediate word: true - emit literal -1 */
static bool compile_true(compiler_t* comp) {
    /* Legacy: Emit LIT cell */
    cell_buffer_append(comp->cells, encode_lit(-1));

    /* CID-based: Store literal and encode reference */
    unsigned char* cid = db_store_literal(comp->db, -1, "i64");
    if (!cid) {
        fprintf(stderr, "Error: Failed to store true literal\n");
        return false;
    }
    encode_cid_ref(comp->blob, BLOB_DATA, cid);
    free(cid);

    push_type(comp, TYPE_I64);

    if (comp->verbose) {
        printf("  LIT -1 (true)\n");
    }

    return true;
}

/* Immediate word: false - emit literal 0 */
static bool compile_false(compiler_t* comp) {
    /* Legacy: Emit LIT cell */
    cell_buffer_append(comp->cells, encode_lit(0));

    /* CID-based: Store literal and encode reference */
    unsigned char* cid = db_store_literal(comp->db, 0, "i64");
    if (!cid) {
        fprintf(stderr, "Error: Failed to store false literal\n");
        return false;
    }
    encode_cid_ref(comp->blob, BLOB_DATA, cid);
    free(cid);

    push_type(comp, TYPE_I64);

    if (comp->verbose) {
        printf("  LIT 0 (false)\n");
    }

    return true;
}

/* ============================================================================
 * Stack Primitive Immediate Handlers
 *
 * These implement compile-time reference counting for stack manipulation.
 * Structure mirrors future March self-hosted implementation.
 * ============================================================================ */

/* March equivalent:
 * : drop immediate ( -- )
 *   ct-stack-pop           \ Pop compile-time stack entry
 *   dup .alloc-id @ ?dup if
 *     ( heap allocated )
 *     ct-rc-dec            \ Decrement refcount
 *     0= if emit-drop.free else emit-drop.raw then
 *   else
 *     ( not heap )
 *     drop emit-drop.raw
 *   then ;
 */
static bool compile_drop(compiler_t* comp) {
    fprintf(stderr, "TRACE: compile_drop() entry\n");
    fflush(stderr);

    if (comp->type_stack_depth < 1) {
        fprintf(stderr, "drop: stack underflow\n");
        return false;
    }

    fprintf(stderr, "TRACE: compile_drop() about to pop\n");
    fflush(stderr);

    /* Pop compile-time stack entry (just removes from type stack) */
    pop_type_entry(comp);

    fprintf(stderr, "TRACE: compile_drop() popped, looking up primitive\n");
    fflush(stderr);

    /* Note: In slot model, we don't free here. Slots stay allocated
     * until word end and are freed if not returned. */

    /* Emit runtime drop */
    dict_entry_t* drop_prim = dict_lookup(comp->dict, "drop");
    if (!drop_prim) {
        fprintf(stderr, "Error: drop primitive not found\n");
        return false;
    }

    fprintf(stderr, "TRACE: compile_drop() emitting code\n");
    fflush(stderr);

    cell_buffer_append(comp->cells, encode_xt(drop_prim->addr));
    encode_primitive(comp->blob, drop_prim->prim_id);

    if (comp->verbose) {
        printf("  XT drop\n");
    }

    fprintf(stderr, "TRACE: compile_drop() success\n");
    fflush(stderr);

    return true;
}

/* March equivalent:
 * : dup immediate ( -- )
 *   ct-stack-top dup ct-stack-push  \ Duplicate entry on CT stack (incl slot_id)
 *   emit-dup.raw ;
 */
static bool compile_dup(compiler_t* comp) {
    if (comp->type_stack_depth < 1) {
        fprintf(stderr, "dup: stack underflow\n");
        return false;
    }

    /* Get top entry and duplicate it (including slot_id) */
    type_stack_entry_t top = comp->type_stack[comp->type_stack_depth - 1];

    if (comp->type_stack_depth >= MAX_TYPE_STACK) {
        fprintf(stderr, "Type stack overflow\n");
        return false;
    }
    comp->type_stack[comp->type_stack_depth] = top;  /* Copy entire entry */
    comp->type_stack_depth++;

    /* Emit runtime dup */
    dict_entry_t* dup_prim = dict_lookup(comp->dict, "dup");
    if (!dup_prim) {
        fprintf(stderr, "Error: dup primitive not found\n");
        return false;
    }

    cell_buffer_append(comp->cells, encode_xt(dup_prim->addr));
    encode_primitive(comp->blob, dup_prim->prim_id);

    if (comp->verbose) {
        printf("  XT dup\n");
    }

    return true;
}

/* March equivalent:
 * : swap immediate ( -- )
 *   ct-stack-pop ct-stack-pop  \ Pop b, then a
 *   swap ct-stack-push ct-stack-push  \ Push b, then a (swapped)
 *   emit-swap.raw ;
 */
static bool compile_swap(compiler_t* comp) {
    if (comp->type_stack_depth < 2) {
        fprintf(stderr, "swap: stack underflow\n");
        return false;
    }

    /* Swap top two entries (preserves slot_ids) */
    type_stack_entry_t top = comp->type_stack[comp->type_stack_depth - 1];
    type_stack_entry_t second = comp->type_stack[comp->type_stack_depth - 2];

    comp->type_stack[comp->type_stack_depth - 2] = top;
    comp->type_stack[comp->type_stack_depth - 1] = second;

    /* Emit runtime swap */
    dict_entry_t* swap_prim = dict_lookup(comp->dict, "swap");
    if (!swap_prim) {
        fprintf(stderr, "Error: swap primitive not found\n");
        return false;
    }

    cell_buffer_append(comp->cells, encode_xt(swap_prim->addr));
    encode_primitive(comp->blob, swap_prim->prim_id);

    if (comp->verbose) {
        printf("  XT swap\n");
    }

    return true;
}

/* March equivalent:
 * : over immediate ( -- )
 *   ct-stack-second dup ct-stack-push  \ Push copy of second entry (incl slot_id)
 *   emit-over.raw ;
 */
static bool compile_over(compiler_t* comp) {
    if (comp->type_stack_depth < 2) {
        fprintf(stderr, "over: stack underflow\n");
        return false;
    }

    /* Get second entry and push copy (including slot_id) */
    type_stack_entry_t second = comp->type_stack[comp->type_stack_depth - 2];

    if (comp->type_stack_depth >= MAX_TYPE_STACK) {
        fprintf(stderr, "Type stack overflow\n");
        return false;
    }
    comp->type_stack[comp->type_stack_depth] = second;  /* Copy entire entry */
    comp->type_stack_depth++;

    /* Emit runtime over */
    dict_entry_t* over_prim = dict_lookup(comp->dict, "over");
    if (!over_prim) {
        fprintf(stderr, "Error: over primitive not found\n");
        return false;
    }

    cell_buffer_append(comp->cells, encode_xt(over_prim->addr));
    encode_primitive(comp->blob, over_prim->prim_id);

    if (comp->verbose) {
        printf("  XT over\n");
    }

    return true;
}

/* March equivalent:
 * : rot immediate ( -- )
 *   ct-stack-pop ct-stack-pop ct-stack-pop  \ c b a
 *   swap rot ct-stack-push ct-stack-push ct-stack-push  \ b c a
 *   emit-rot.raw ;
 */
static bool compile_rot(compiler_t* comp) {
    if (comp->type_stack_depth < 3) {
        fprintf(stderr, "rot: stack underflow\n");
        return false;
    }

    /* Rotate top three entries: ( a b c -- b c a ) */
    type_stack_entry_t c = comp->type_stack[comp->type_stack_depth - 1];
    type_stack_entry_t b = comp->type_stack[comp->type_stack_depth - 2];
    type_stack_entry_t a = comp->type_stack[comp->type_stack_depth - 3];

    comp->type_stack[comp->type_stack_depth - 3] = b;
    comp->type_stack[comp->type_stack_depth - 2] = c;
    comp->type_stack[comp->type_stack_depth - 1] = a;

    /* Emit runtime rot */
    dict_entry_t* rot_prim = dict_lookup(comp->dict, "rot");
    if (!rot_prim) {
        fprintf(stderr, "Error: rot primitive not found\n");
        return false;
    }

    cell_buffer_append(comp->cells, encode_xt(rot_prim->addr));
    encode_primitive(comp->blob, rot_prim->prim_id);

    if (comp->verbose) {
        printf("  XT rot\n");
    }

    return true;
}

/* _ (underscore) - Pull value from before [ marker into array construction
 * Explicit array comprehension: 2 3 [ _ _ + ] → [5]
 * Sequential consumption: first _ pulls -1, second _ pulls -2, etc.
 */
static bool compile_underscore(compiler_t* comp) {
    /* Check if we're inside an array literal */
    if (comp->array_marker_depth == 0) {
        fprintf(stderr, "Error: _ (underscore) can only be used inside array literals []\n");
        return false;
    }

    /* Get the current array marker depth and consumption count */
    int marker_idx = comp->array_marker_depth - 1;
    int marker_depth = comp->array_marker_stack[marker_idx];
    int consumed = comp->array_consumed_count[marker_idx];

    /* Calculate source index: work backwards from marker */
    int source_index = marker_depth - consumed - 1;

    /* Validate that we have a value to pull */
    if (source_index < 0) {
        fprintf(stderr, "Error: _ (underscore) - no more values before [ marker (tried to access index %d)\n", source_index);
        return false;
    }

    /* Get the value from before the marker */
    type_stack_entry_t entry = comp->type_stack[source_index];

    /* Emit runtime code to copy the value from the calculated depth */
    /* Calculate how deep we need to pick from current runtime stack */
    /* NOTE: Calculate BEFORE incrementing type_stack_depth, as runtime hasn't executed copy yet */
    int runtime_depth = comp->type_stack_depth - source_index - 1;

    /* Push copy onto current type stack (like dup, but from specific position) */
    if (comp->type_stack_depth >= MAX_TYPE_STACK) {
        fprintf(stderr, "Type stack overflow\n");
        return false;
    }
    comp->type_stack[comp->type_stack_depth] = entry;  /* Copy entire entry including slot_id, node_id */
    comp->type_stack_depth++;

    /* Increment consumption counter for this nesting level */
    comp->array_consumed_count[marker_idx]++;

    /* Emit appropriate stack operation based on depth */
    if (runtime_depth == 0) {
        /* Depth 0: copy TOS - use dup */
        dict_entry_t* dup_prim = dict_lookup(comp->dict, "dup");
        if (!dup_prim) {
            fprintf(stderr, "Error: dup primitive not found\n");
            return false;
        }
        cell_buffer_append(comp->cells, encode_xt(dup_prim->addr));
        encode_primitive(comp->blob, dup_prim->prim_id);
    } else if (runtime_depth == 1) {
        /* Depth 1: copy second item - use over */
        dict_entry_t* over_prim = dict_lookup(comp->dict, "over");
        if (!over_prim) {
            fprintf(stderr, "Error: over primitive not found\n");
            return false;
        }
        cell_buffer_append(comp->cells, encode_xt(over_prim->addr));
        encode_primitive(comp->blob, over_prim->prim_id);
    } else {
        /* General case: use pick primitive */
        dict_entry_t* pick_prim = dict_lookup(comp->dict, "pick");
        if (!pick_prim) {
            fprintf(stderr, "Error: pick primitive not found (needed for _ at depth %d)\n", runtime_depth);
            return false;
        }

        /* Emit: <depth> pick */
        cell_buffer_append(comp->cells, encode_lit(runtime_depth));
        unsigned char* depth_cid = db_store_literal(comp->db, runtime_depth, "i64");
        if (!depth_cid) {
            fprintf(stderr, "Error: Failed to store pick depth literal\n");
            return false;
        }
        encode_cid_ref(comp->blob, BLOB_DATA, depth_cid);
        free(depth_cid);

        cell_buffer_append(comp->cells, encode_xt(pick_prim->addr));
        encode_primitive(comp->blob, pick_prim->prim_id);
    }

    if (comp->verbose) {
        printf("  _ pull value from index %d (before marker at %d, consumed %d)\n",
               source_index, marker_depth, consumed + 1);
    }

    return true;
}

/* march.alloc - Allocate heap memory with type graph tracking
 * Stack: ( size:i64 -- ptr:ptr )
 * This is a dual word: immediate handler + runtime primitive
 * - Emits runtime allocation code
 * - Updates type graph to track the allocated object
 */
static bool compile_march_alloc(compiler_t* comp) {
    crash_context_set_token("march.alloc");
    crash_context_set_stacks(comp->type_stack_depth,
                            comp->quot_stack_depth,
                            comp->buffer_stack_depth);

    /* Type check: need i64 size on stack */
    if (comp->type_stack_depth < 1) {
        fprintf(stderr, "Error: march.alloc requires size (i64) on stack\n");
        return false;
    }

    type_id_t size_type = comp->type_stack[comp->type_stack_depth - 1].type;
    if (size_type != TYPE_I64) {
        fprintf(stderr, "Error: march.alloc requires i64 size, got type %d\n", size_type);
        return false;
    }

    /* Look up the runtime primitive */
    dict_entry_t* alloc_prim = dict_lookup(comp->dict, "alloc");
    if (!alloc_prim) {
        fprintf(stderr, "Internal error: alloc primitive not found\n");
        return false;
    }

    /* Emit runtime code: call alloc primitive */
    cell_buffer_append(comp->cells, encode_xt(alloc_prim->addr));
    encode_primitive(comp->blob, alloc_prim->prim_id);

    /* Update type stack and type graph */
    pop_type(comp);  /* Remove size */
    push_heap_value(comp, TYPE_PTR);  /* Add allocated pointer with tracking */

    if (comp->verbose) {
        printf("  march.alloc emitted with type graph tracking\n");
    }

    return true;
}

/* Compile type signature declaration: $ i64 -- i64 i64 ; */
static bool compile_type_sig_decl(compiler_t* comp, token_stream_t* stream) {
    /* Build type signature string from tokens until ';' */
    char sig_buffer[256];
    sig_buffer[0] = '\0';
    size_t sig_len = 0;

    token_t tok;
    while (token_stream_next(stream, &tok)) {
        if (tok.type == TOK_SEMICOLON) {
            token_free(&tok);
            break;
        }

        /* Append token text to signature (with space separator) */
        if (sig_len > 0 && sig_len < 255) {
            sig_buffer[sig_len++] = ' ';
        }

        size_t tok_len = strlen(tok.text);
        if (sig_len + tok_len < 255) {
            strcpy(sig_buffer + sig_len, tok.text);
            sig_len += tok_len;
        }

        /* Replace -- with -> for consistency */
        if (strcmp(tok.text, "--") == 0 && sig_len >= 2) {
            sig_buffer[sig_len - 2] = '-';
            sig_buffer[sig_len - 1] = '>';
        }

        token_free(&tok);
    }

    if (comp->verbose) {
        printf("\nType signature declaration: %s\n", sig_buffer);
    }

    /* Allocate and parse type signature */
    if (comp->pending_type_sig) {
        free(comp->pending_type_sig);
    }
    comp->pending_type_sig = malloc(sizeof(type_sig_t));
    if (!comp->pending_type_sig) {
        fprintf(stderr, "Failed to allocate type signature\n");
        return false;
    }

    if (!parse_type_sig(sig_buffer, comp->pending_type_sig)) {
        fprintf(stderr, "Failed to parse type signature: %s\n", sig_buffer);
        free(comp->pending_type_sig);
        comp->pending_type_sig = NULL;
        return false;
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

    /* Track current word in crash context */
    crash_context_set_word(word_name);

    if (comp->verbose) {
        printf("\nDefining word: %s (collecting tokens)\n", word_name);
    }

    /* Create word definition to store tokens (Design B: lazy compilation) */
    word_definition_t* word_def = word_definition_create(word_name);
    if (!word_def) {
        free(word_name);
        return false;
    }

    /* Reset buffers and type stack for new definition */
    cell_buffer_clear(comp->cells);
    blob_buffer_clear(comp->blob);
    comp->type_stack_depth = 0;

    /* If there's a pending type signature, store it with the word definition */
    if (comp->pending_type_sig) {
        word_def->type_sig = malloc(sizeof(type_sig_t));
        if (word_def->type_sig) {
            memcpy(word_def->type_sig, comp->pending_type_sig, sizeof(type_sig_t));
        }
        if (comp->verbose) {
            printf("  Stored type signature with %d inputs → %d outputs\n",
                   comp->pending_type_sig->input_count,
                   comp->pending_type_sig->output_count);
        }
        /* Clear pending signature */
        free(comp->pending_type_sig);
        comp->pending_type_sig = NULL;
    }

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
        fprintf(stderr, "TRACE: In word '%s', token type=%d text='%s'\n",
                word_name, tok.type, tok.text ? tok.text : "NULL");
        fflush(stderr);

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

        /* Handle quotation delimiters specially */
        if (tok.type == TOK_LPAREN) {
            success = compile_lparen(comp);
        } else if (tok.type == TOK_RPAREN) {
            success = compile_rparen(comp);
        }
        /* If inside a quotation, capture tokens to quotation */
        else if (comp->buffer_stack_depth > 0) {
            quotation_t* quot = comp->quot_stack[comp->quot_stack_depth - 1];
            success = quot_append_token(quot, &tok);

            if (comp->verbose && success) {
                printf("    captured to quotation: type=%d text='%s'\n",
                       tok.type, tok.text ? tok.text : "NULL");
            }
        }
        /* Design B: Collect all other tokens to word definition */
        else {
            success = word_def_append_token(word_def, &tok);

            if (comp->verbose && success) {
                printf("    captured to word: type=%d text='%s'\n",
                       tok.type, tok.text ? tok.text : "NULL");
            }
        }

        token_free(&tok);

        if (!success) {
            free(source_text);
            free(word_name);
            return false;
        }
    }

    /* Design B: Store word definition in compiler cache for later compilation */
    if (comp->word_def_count >= MAX_WORD_DEFS) {
        fprintf(stderr, "Too many word definitions (max %d)\n", MAX_WORD_DEFS);
        word_definition_free(word_def);
        free(source_text);
        free(word_name);
        return false;
    }

    comp->word_defs[comp->word_def_count++] = word_def;

    if (comp->verbose) {
        printf("  Stored %d tokens in word definition cache\n", word_def->token_count);
    }

    /* Add placeholder dictionary entry so the word can be referenced */
    type_sig_t placeholder_sig;

    if (word_def->type_sig) {
        /* Use provided type signature */
        placeholder_sig = *word_def->type_sig;
        if (comp->verbose) {
            printf("  Using explicit type signature: %d inputs → %d outputs\n",
                   placeholder_sig.input_count, placeholder_sig.output_count);
        }
    } else {
        /* Generic signature: -> ? (unknown outputs, will be determined at call site) */
        placeholder_sig.input_count = 0;
        placeholder_sig.output_count = 1;
        placeholder_sig.outputs[0] = TYPE_UNKNOWN;
        if (comp->verbose) {
            printf("  No type signature provided, using placeholder: -> ?\n");
        }
    }

    /* Add dict entry with NULL addr (indicates uncompiled word) */
    dict_add(comp->dict, word_name, NULL, NULL, 0, &placeholder_sig, false, false, NULL, word_def);

    free(source_text);
    free(word_name);

    /* Clear pending type signature after using it */
    if (comp->pending_type_sig) {
        free(comp->pending_type_sig);
        comp->pending_type_sig = NULL;
    }

    /* Clear word context */
    crash_context_set_word(NULL);

    return true;
}

/* Compile a file */
bool compiler_compile_file(compiler_t* comp, const char* filename) {
    fprintf(stderr, "TRACE: compiler_compile_file entry\n");
    fflush(stderr);

    token_stream_t* stream = token_stream_create(filename);
    if (!stream) {
        fprintf(stderr, "Cannot open file: %s\n", filename);
        return false;
    }

    fprintf(stderr, "TRACE: Token stream created\n");
    fflush(stderr);

    if (comp->verbose) {
        printf("Compiling: %s\n", filename);
    }

    DEBUG_COMPILER("Starting token loop");
    token_t tok;
    while (token_stream_next(stream, &tok)) {
        fprintf(stderr, "TRACE: Token type=%d text='%s'\n", tok.type, tok.text ? tok.text : "NULL");
        fflush(stderr);

        DEBUG_COMPILER("Processing token type=%d text='%s'", tok.type, tok.text ? tok.text : "NULL");
        bool success = true;

        if (tok.type == TOK_DOLLAR) {
            /* Type signature declaration */
            success = compile_type_sig_decl(comp, stream);
        } else if (tok.type == TOK_COLON) {
            /* Start word definition */
            fprintf(stderr, "TRACE: Starting word definition\n");
            fflush(stderr);
            DEBUG_COMPILER("Calling compile_definition");
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
