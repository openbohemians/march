/*
 * March Language - Core Type Definitions
 * Common types and constants used throughout the compiler
 */

#ifndef MARCH_TYPES_H
#define MARCH_TYPES_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

/* Cell tags - variable-bit encoding */
#define TAG_XT   0x0  /* 00  - Execute word (EXIT if addr=0) */
#define TAG_LIT  0x1  /* 01  - Immediate 62-bit signed literal */
#define TAG_LST  0x2  /* 010 - Symbol ID literal */
#define TAG_LNT  0x6  /* 110 - Next N literals (raw 64-bit values) */

/* Blob kind identifiers (for database storage) */
/* Legacy blob kinds (cell-based storage) */
#define BLOB_CODE       1    /* Cell stream (compiled word/quotation) */
#define BLOB_STRING     2    /* UTF-8 string data */
#define BLOB_ARRAY      3    /* Array header (count + element sig_cid) */
#define BLOB_STRUCT     4    /* Struct data (field values) */
#define BLOB_BINARY     5    /* Raw binary data (untyped) */

/* CID-based blob kinds (LINKING.md design) */
#define BLOB_PRIMITIVE  0    /* Assembly primitive (fixed ID) */
#define BLOB_WORD       1    /* User-defined word (CID sequence) */
#define BLOB_QUOTATION  2    /* Quotation (CID sequence, push address not call) */
#define BLOB_DATA       3    /* Literal data (serialized value) */

/* Fixed primitive ID table (LINKING.md design) */
/* These IDs are stable and never change - assembly can be updated without breaking compiled code */
#define PRIM_LIT        0    /* i64 literal (8 bytes follow tag) */
#define PRIM_ADD        1    /* + */
#define PRIM_SUB        2    /* - */
#define PRIM_MUL        3    /* * */
#define PRIM_DIV        4    /* / */
#define PRIM_MOD        5    /* % */
#define PRIM_DUP        6    /* dup */
#define PRIM_DROP       7    /* drop */
#define PRIM_SWAP       8    /* swap */
#define PRIM_OVER       9    /* over */
#define PRIM_ROT        10   /* rot */
#define PRIM_EQ         11   /* = */
#define PRIM_NE         12   /* != */
#define PRIM_LT         13   /* < */
#define PRIM_GT         14   /* > */
#define PRIM_LE         15   /* <= */
#define PRIM_GE         16   /* >= */
#define PRIM_AND        17   /* & (bitwise) */
#define PRIM_OR         18   /* | (bitwise) */
#define PRIM_XOR        19   /* ^ (bitwise) */
#define PRIM_NOT        20   /* ~ (bitwise) */
#define PRIM_LSHIFT     21   /* << */
#define PRIM_RSHIFT     22   /* >> (logical) */
#define PRIM_ARSHIFT    23   /* >>> (arithmetic) */
#define PRIM_LAND       24   /* and (logical) */
#define PRIM_LOR        25   /* or (logical) */
#define PRIM_LNOT       26   /* not (logical) */
#define PRIM_ZEROP      27   /* 0? */
#define PRIM_ZEROGT     28   /* 0> */
#define PRIM_ZEROLT     29   /* 0< */
#define PRIM_FETCH      30   /* @ */
#define PRIM_STORE      31   /* ! */
#define PRIM_CFETCH     32   /* c@ */
#define PRIM_CSTORE     33   /* c! */
#define PRIM_TOR        34   /* >r */
#define PRIM_FROMR      35   /* r> */
#define PRIM_RFETCH     36   /* r@ */
#define PRIM_RDROP      37   /* rdrop */
#define PRIM_TWOTOR     38   /* 2>r */
#define PRIM_TWOFROMR   39   /* 2r> */
#define PRIM_BRANCH     40   /* branch */
#define PRIM_0BRANCH    41   /* 0branch */
#define PRIM_EXECUTE    42   /* execute */
#define PRIM_I0         43   /* i0 - loop counter */
#define PRIM_FREE       44   /* free - deallocate slot */
#define PRIM_ALLOC      45   /* alloc - allocate memory */
#define PRIM_IDENTITY   46   /* _ - identity function */
#define PRIM_MEMCPY     47   /* memcpy - copy memory block */
#define PRIM_ARRAY_LEN  48   /* array-length - get array count */
#define PRIM_STR_LEN    49   /* str-length - get string count */
#define PRIM_MUT        50   /* mut - create mutable copy */
#define PRIM_ARRAY_AT   51   /* march.array.at - read array element */
#define PRIM_ARRAY_SET  52   /* march.array.set! - write array element */
#define PRIM_ARRAY_FILL 53   /* march.array.fill! - fill with value */
#define PRIM_ARRAY_REV  54   /* march.array.reverse! - reverse in place */
#define PRIM_ARRAY_CONCAT 55 /* march.array.concat - concatenate arrays */

/* Map operations (HAMT - persistent hash maps) */
#define PRIM_MAP_NEW    56   /* march.map.new - create empty map */
#define PRIM_MAP_GET    57   /* march.map.get - lookup value by key */
#define PRIM_MAP_SET    58   /* march.map.set - insert/update key-value */
#define PRIM_MAP_REMOVE 59   /* march.map.remove - delete key */
#define PRIM_MAP_SIZE   60   /* march.map.size - get element count */
#define PRIM_MAP_FREE   61   /* march.map.free - free map memory */

/* Additional stack operations */
#define PRIM_PICK       62   /* pick - copy nth stack item (0=dup, 1=over) */

/* Cell type */
typedef uint64_t cell_t;

/* Type identifiers for type checking */
typedef enum {
    TYPE_UNKNOWN = 0,
    TYPE_I64,      /* 64-bit signed integer */
    TYPE_U64,      /* 64-bit unsigned integer */
    TYPE_F64,      /* 64-bit float */
    TYPE_PTR,      /* Pointer/address */
    TYPE_BOOL,     /* Boolean */
    TYPE_STR,      /* String reference (immutable) */
    TYPE_STR_MUT,  /* Mutable string reference */
    TYPE_ARRAY,    /* Array reference (immutable by convention) */
    TYPE_ARRAY_MUT,/* Mutable array reference */
    TYPE_ANY,      /* Polymorphic type variable */
    /* Type variables (single-letter names: a, b, c, ...) */
    TYPE_VAR_A, TYPE_VAR_B, TYPE_VAR_C, TYPE_VAR_D, TYPE_VAR_E,
    TYPE_VAR_F, TYPE_VAR_G, TYPE_VAR_H, TYPE_VAR_I, TYPE_VAR_J,
    TYPE_VAR_K, TYPE_VAR_L, TYPE_VAR_M, TYPE_VAR_N, TYPE_VAR_O,
    TYPE_VAR_P, TYPE_VAR_Q, TYPE_VAR_R, TYPE_VAR_S, TYPE_VAR_T,
    TYPE_VAR_U, TYPE_VAR_V, TYPE_VAR_W, TYPE_VAR_X, TYPE_VAR_Y,
    TYPE_VAR_Z,
} type_id_t;

/* Type stack entry (compile-time only) */
typedef struct {
    type_id_t type;
    int slot_id;  /* -1 = not heap-allocated, >=0 = slot index for heap pointer */
    uint32_t node_id;  /* Reference graph node ID, 0 = not a heap reference */
} type_stack_entry_t;

/* Maximum sizes */
#define MAX_TYPE_STACK    256
#define MAX_WORD_NAME     64
#define MAX_TYPE_SIG      256
#define MAX_CELL_STREAM   4096

/* CID constants (SHA256 binary hashes are 32 bytes) */
#define CID_SIZE 32

/* Blob buffer for variable-length CID sequence encoding (LINKING.md design) */
/* This is separate from cell_buffer_t which is for runtime cells */
typedef struct {
    uint8_t* data;       /* Raw bytes */
    size_t size;         /* Current size */
    size_t capacity;     /* Allocated capacity */
} blob_buffer_t;

/* Blob buffer operations (implemented in cells.c) */
blob_buffer_t* blob_buffer_create(void);
void blob_buffer_free(blob_buffer_t* buf);
void blob_buffer_clear(blob_buffer_t* buf);
void blob_buffer_append_u16(blob_buffer_t* buf, uint16_t value);
void blob_buffer_append_bytes(blob_buffer_t* buf, const uint8_t* bytes, size_t len);

/* Blob encoding functions (LINKING.md design) */
void encode_primitive(blob_buffer_t* buf, uint16_t prim_id);
void encode_cid_ref(blob_buffer_t* buf, uint16_t kind, const unsigned char* cid);
void encode_inline_literal(blob_buffer_t* buf, int64_t value);

/* Blob decoding functions */
const uint8_t* decode_tag_ex(const uint8_t* ptr, bool* is_cid, uint16_t* id_or_kind, const unsigned char** cid);

/* ============================================================================ */
/* Reference Graph - Compile-Time Memory Management */
/* ============================================================================ */

/* Node ID type - 0 is reserved for NODE_ID_INVALID */
typedef uint32_t node_id_t;
#define NODE_ID_INVALID 0

/* Reference node - represents a heap-allocated object at compile time */
typedef struct {
    node_id_t node_id;           /* Unique identifier for this allocation */
    type_id_t object_type;       /* TYPE_ARRAY, TYPE_STR, etc. */
    int slot_id;                 /* Corresponding slot ID for FREE instruction */
    bool is_escaped;             /* True if escapes (FFI, async, closures) */

    /* Child relationships (for containers like arrays, records) */
    node_id_t* children;         /* Array of child node IDs */
    size_t child_count;          /* Number of children */
    size_t child_capacity;       /* Allocated capacity */
} ref_node_t;

/* Reference graph - tracks all heap objects for a word during compilation */
typedef struct {
    ref_node_t* nodes;           /* Array of all nodes in this graph */
    size_t node_count;           /* Number of nodes */
    size_t node_capacity;        /* Allocated capacity */

    node_id_t next_node_id;      /* Next node ID to allocate (starts at 1) */

    /* Quick lookup: node_id -> array index in nodes[] */
    size_t* node_index;          /* node_index[node_id] = index into nodes[] */
    size_t node_index_capacity;  /* Allocated capacity for lookup table */
} ref_graph_t;

/* Reference graph operations */
ref_graph_t* ref_graph_create(void);
void ref_graph_free(ref_graph_t* graph);
void ref_graph_clear(ref_graph_t* graph);

/* Node operations */
node_id_t ref_graph_alloc_node(ref_graph_t* graph, type_id_t obj_type, int slot_id);
ref_node_t* ref_graph_get_node(ref_graph_t* graph, node_id_t node_id);
void ref_graph_add_child(ref_graph_t* graph, node_id_t parent_id, node_id_t child_id);
void ref_graph_mark_escaped(ref_graph_t* graph, node_id_t node_id);

#endif /* MARCH_TYPES_H */
