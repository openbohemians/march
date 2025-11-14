/*
 * March Language - Assembly Primitive Declarations
 */

#ifndef MARCH_PRIMITIVES_H
#define MARCH_PRIMITIVES_H

#include "dictionary.h"

/* External assembly primitives (from kernel/x86-64/ .asm files) */

/* Stack ops */
extern void op_dup(void);
extern void op_drop(void);
extern void op_swap(void);
extern void op_over(void);
extern void op_rot(void);
extern void op_pick(void);
extern void op_identity(void);

/* Arithmetic */
extern void op_add(void);
extern void op_sub(void);
extern void op_mul(void);
extern void op_div(void);
extern void op_mod(void);

/* Comparisons */
extern void op_eq(void);
extern void op_ne(void);
extern void op_lt(void);
extern void op_gt(void);
extern void op_le(void);
extern void op_ge(void);

/* Bitwise */
extern void op_and(void);
extern void op_or(void);
extern void op_xor(void);
extern void op_not(void);
extern void op_lshift(void);
extern void op_rshift(void);
extern void op_arshift(void);

/* Logical */
extern void op_land(void);
extern void op_lor(void);
extern void op_lnot(void);
extern void op_zerop(void);
extern void op_zerogt(void);
extern void op_zerolt(void);

/* Memory */
extern void op_fetch(void);
extern void op_store(void);
extern void op_cfetch(void);
extern void op_cstore(void);

/* Return stack */
extern void op_tor(void);
extern void op_fromr(void);
extern void op_rfetch(void);
extern void op_rdrop(void);
extern void op_twotor(void);
extern void op_twofromr(void);

/* Control flow */
extern void op_branch(void);
extern void op_0branch(void);

/* Loop control */
extern void op_i0(void);

/* Quotation execution */
extern void op_execute(void);

/* Memory management */
extern void op_alloc(void);
extern void op_free(void);
extern void op_memcpy(void);

/* Array/String operations */
extern void op_array_length(void);
extern void op_str_length(void);
extern void op_mut(void);
extern void op_array_at(void);
extern void op_array_set(void);
extern void op_array_fill(void);
extern void op_array_reverse(void);
extern void op_array_concat(void);

/* Map operations (HAMT - persistent hash maps) */
extern void op_map_new(void);
extern void op_map_get(void);
extern void op_map_set(void);
extern void op_map_remove(void);
extern void op_map_size(void);
extern void op_map_free(void);

/* ============================================================================ */
/* Primitive Dispatch Table */
/* ============================================================================ */

/* Static dispatch table: maps primitive ID → runtime address
 * This is a simple array lookup, much faster than CID-based hashtable
 */
extern void* primitive_dispatch_table[256];

/* Register all primitives in dictionary */
void register_primitives(dictionary_t* dict);

#endif /* MARCH_PRIMITIVES_H */
