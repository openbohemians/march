/*
 * March Language - Primitive Registration
 */

#include "primitives.h"
#include "types.h"
#include <stdio.h>

/* ============================================================================ */
/* Primitive Dispatch Table */
/* ============================================================================ */
/* Maps primitive IDs to their runtime addresses.
 * This is a static table compiled into the binary - no database needed!
 */
void* primitive_dispatch_table[256] = {
    [PRIM_ADD]      = &op_add,
    [PRIM_SUB]      = &op_sub,
    [PRIM_MUL]      = &op_mul,
    [PRIM_DIV]      = &op_div,
    [PRIM_MOD]      = &op_mod,
    [PRIM_DUP]      = &op_dup,
    [PRIM_DROP]     = &op_drop,
    [PRIM_SWAP]     = &op_swap,
    [PRIM_OVER]     = &op_over,
    [PRIM_ROT]      = &op_rot,
    [PRIM_EQ]       = &op_eq,
    [PRIM_NE]       = &op_ne,
    [PRIM_LT]       = &op_lt,
    [PRIM_GT]       = &op_gt,
    [PRIM_LE]       = &op_le,
    [PRIM_GE]       = &op_ge,
    [PRIM_AND]      = &op_and,
    [PRIM_OR]       = &op_or,
    [PRIM_XOR]      = &op_xor,
    [PRIM_NOT]      = &op_not,
    [PRIM_LSHIFT]   = &op_lshift,
    [PRIM_RSHIFT]   = &op_rshift,
    [PRIM_ARSHIFT]  = &op_arshift,
    [PRIM_LAND]     = &op_land,
    [PRIM_LOR]      = &op_lor,
    [PRIM_LNOT]     = &op_lnot,
    [PRIM_ZEROP]    = &op_zerop,
    [PRIM_ZEROGT]   = &op_zerogt,
    [PRIM_ZEROLT]   = &op_zerolt,
    [PRIM_FETCH]    = &op_fetch,
    [PRIM_STORE]    = &op_store,
    [PRIM_CFETCH]   = &op_cfetch,
    [PRIM_CSTORE]   = &op_cstore,
    [PRIM_TOR]      = &op_tor,
    [PRIM_FROMR]    = &op_fromr,
    [PRIM_RFETCH]   = &op_rfetch,
    [PRIM_RDROP]    = &op_rdrop,
    [PRIM_TWOTOR]   = &op_twotor,
    [PRIM_TWOFROMR] = &op_twofromr,
    [PRIM_BRANCH]   = &op_branch,
    [PRIM_0BRANCH]  = &op_0branch,
    [PRIM_EXECUTE]  = &op_execute,
    [PRIM_I0]       = &op_i0,
    [PRIM_ALLOC]    = &op_alloc,
    [PRIM_FREE]     = &op_free,
    [PRIM_IDENTITY] = &op_identity,
    [PRIM_MEMCPY]   = &op_memcpy,
    [PRIM_ARRAY_LEN] = &op_array_length,
    [PRIM_STR_LEN]  = &op_str_length,
    [PRIM_MUT]      = &op_mut,
    [PRIM_ARRAY_AT] = &op_array_at,
    [PRIM_ARRAY_SET] = &op_array_set,
    [PRIM_ARRAY_FILL] = &op_array_fill,
    [PRIM_ARRAY_REV] = &op_array_reverse,
    [PRIM_ARRAY_CONCAT] = &op_array_concat,
    [PRIM_MAP_NEW] = &op_map_new,
    [PRIM_MAP_GET] = &op_map_get,
    [PRIM_MAP_SET] = &op_map_set,
    [PRIM_MAP_REMOVE] = &op_map_remove,
    [PRIM_MAP_SIZE] = &op_map_size,
    [PRIM_MAP_FREE] = &op_map_free,
};

/* ============================================================================ */
/* Primitive Registration */
/* ============================================================================ */

/* Helper macro to register a primitive in the dictionary */
#define REG_PRIM(name_str, prim_id, op_func, type_str) \
    do { \
        parse_type_sig(type_str, &sig); \
        dict_add(dict, name_str, &op_func, NULL, prim_id, &sig, true, false, NULL, NULL); \
    } while(0)

void register_primitives(dictionary_t* dict) {
    type_sig_t sig;

    /* Stack ops - polymorphic (work on any type) */
    REG_PRIM("dup", PRIM_DUP, op_dup, "a -> a a");
    REG_PRIM("drop", PRIM_DROP, op_drop, "a ->");
    REG_PRIM("swap", PRIM_SWAP, op_swap, "a b -> b a");
    REG_PRIM("over", PRIM_OVER, op_over, "a b -> a b a");
    REG_PRIM("rot", PRIM_ROT, op_rot, "a b c -> b c a");
    /* Note: _ is registered as immediate word in compiler.c, not as primitive */
    /* REG_PRIM("_", PRIM_IDENTITY, op_identity, "a -> a"); */

    /* Arithmetic */
    REG_PRIM("+", PRIM_ADD, op_add, "i64 i64 -> i64");
    REG_PRIM("-", PRIM_SUB, op_sub, "i64 i64 -> i64");
    REG_PRIM("*", PRIM_MUL, op_mul, "i64 i64 -> i64");
    REG_PRIM("/", PRIM_DIV, op_div, "i64 i64 -> i64");
    REG_PRIM("mod", PRIM_MOD, op_mod, "i64 i64 -> i64");

    /* Comparisons */
    REG_PRIM("=", PRIM_EQ, op_eq, "i64 i64 -> bool");
    REG_PRIM("<>", PRIM_NE, op_ne, "i64 i64 -> bool");
    REG_PRIM("<", PRIM_LT, op_lt, "i64 i64 -> bool");
    REG_PRIM(">", PRIM_GT, op_gt, "i64 i64 -> bool");
    REG_PRIM("<=", PRIM_LE, op_le, "i64 i64 -> bool");
    REG_PRIM(">=", PRIM_GE, op_ge, "i64 i64 -> bool");

    /* Bitwise */
    REG_PRIM("and", PRIM_AND, op_and, "i64 i64 -> i64");
    REG_PRIM("or", PRIM_OR, op_or, "i64 i64 -> i64");
    REG_PRIM("xor", PRIM_XOR, op_xor, "i64 i64 -> i64");
    REG_PRIM("not", PRIM_NOT, op_not, "i64 -> i64");
    REG_PRIM("<<", PRIM_LSHIFT, op_lshift, "i64 i64 -> i64");
    REG_PRIM(">>", PRIM_RSHIFT, op_rshift, "i64 i64 -> i64");
    REG_PRIM(">>>", PRIM_ARSHIFT, op_arshift, "i64 i64 -> i64");

    /* Logical */
    REG_PRIM("land", PRIM_LAND, op_land, "bool bool -> bool");
    REG_PRIM("lor", PRIM_LOR, op_lor, "bool bool -> bool");
    REG_PRIM("lnot", PRIM_LNOT, op_lnot, "bool -> bool");
    REG_PRIM("0=", PRIM_ZEROP, op_zerop, "i64 -> bool");
    REG_PRIM("0>", PRIM_ZEROGT, op_zerogt, "i64 -> bool");
    REG_PRIM("0<", PRIM_ZEROLT, op_zerolt, "i64 -> bool");

    /* Memory */
    REG_PRIM("@", PRIM_FETCH, op_fetch, "ptr -> i64");
    REG_PRIM("!", PRIM_STORE, op_store, "i64 ptr ->");
    REG_PRIM("c@", PRIM_CFETCH, op_cfetch, "ptr -> i64");
    REG_PRIM("c!", PRIM_CSTORE, op_cstore, "i64 ptr ->");

    /* Return stack - polymorphic */
    REG_PRIM(">r", PRIM_TOR, op_tor, "a ->");
    REG_PRIM("r>", PRIM_FROMR, op_fromr, "-> a");
    REG_PRIM("r@", PRIM_RFETCH, op_rfetch, "-> a");
    REG_PRIM("rdrop", PRIM_RDROP, op_rdrop, "->");
    REG_PRIM("2>r", PRIM_TWOTOR, op_twotor, "a b ->");
    REG_PRIM("2r>", PRIM_TWOFROMR, op_twofromr, "-> a b");

    /* Control flow */
    REG_PRIM("branch", PRIM_BRANCH, op_branch, "->");
    REG_PRIM("0branch", PRIM_0BRANCH, op_0branch, "i64 ->");

    /* Loop control */
    REG_PRIM("i0", PRIM_I0, op_i0, "-> i64");

    /* Quotation execution - polymorphic ptr */
    REG_PRIM("execute", PRIM_EXECUTE, op_execute, "a ->");

    /* Memory management */
    REG_PRIM("alloc", PRIM_ALLOC, op_alloc, "i64 -> ptr");
    REG_PRIM("free", PRIM_FREE, op_free, "i64 ->");
    REG_PRIM("memcpy", PRIM_MEMCPY, op_memcpy, "ptr ptr i64 -> ptr");

    /* Array/String operations */
    REG_PRIM("march.array.length", PRIM_ARRAY_LEN, op_array_length, "array -> i64");  /* Also accepts array! */
    REG_PRIM("str-length", PRIM_STR_LEN, op_str_length, "str -> i64");  /* Also accepts str! */
    REG_PRIM("mut", PRIM_MUT, op_mut, "array -> array!");  /* Also works with str -> str! */
    REG_PRIM("march.array.at", PRIM_ARRAY_AT, op_array_at, "array i64 -> i64");  /* Also accepts array! */

    /* Mutable array operations (in-place mutation) */
    REG_PRIM("march.array.mut.set", PRIM_ARRAY_SET, op_array_set, "array! i64 i64 -> array!");
    REG_PRIM("march.array.mut.fill", PRIM_ARRAY_FILL, op_array_fill, "array! i64 -> array!");
    REG_PRIM("march.array.mut.reverse", PRIM_ARRAY_REV, op_array_reverse, "array! -> array!");

    /* Immutable array operations */
    REG_PRIM("march.array.concat", PRIM_ARRAY_CONCAT, op_array_concat, "array array -> array");  /* Concatenate */

    /* Map operations (HAMT - persistent hash maps) */
    /* Note: maps are i64 pointers, not a separate type */
    REG_PRIM("march.map.new", PRIM_MAP_NEW, op_map_new, "-> i64");
    REG_PRIM("march.map.get", PRIM_MAP_GET, op_map_get, "i64 i64 -> i64");
    REG_PRIM("march.map.set", PRIM_MAP_SET, op_map_set, "i64 i64 i64 -> i64");
    REG_PRIM("march.map.remove", PRIM_MAP_REMOVE, op_map_remove, "i64 i64 -> i64");
    REG_PRIM("march.map.size", PRIM_MAP_SIZE, op_map_size, "i64 -> i64");
    REG_PRIM("march.map.free", PRIM_MAP_FREE, op_map_free, "i64 ->");
}

#undef REG_PRIM
