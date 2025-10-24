/*
 * March Language - Word Loader
 * Load compiled words from database into memory
 */

#ifndef MARCH_LOADER_H
#define MARCH_LOADER_H

#include "types.h"
#include "database.h"
#include "dictionary.h"
#include "vm_api.h"
#include <stddef.h>
#include <stdbool.h>

/* Loaded word structure */
typedef struct {
    char* name;
    cell_t* cells;
    size_t cell_count;
    void* entry_point;  /* Address of first cell */
} loaded_word_t;

/* CID-to-address cache entry (for linking) */
typedef struct cid_cache_entry {
    unsigned char cid[CID_SIZE];  /* Binary CID (32 bytes) */
    void* value;
    int kind;
    struct cid_cache_entry* next;
} cid_cache_entry_t;

/* CID cache hashtable */
typedef struct {
    cid_cache_entry_t* buckets[256];
} cid_cache_t;

/* Loader context (LINKING.md design) */
typedef struct {
    march_db_t* db;
    dictionary_t* dict;

    /* CID-to-address cache (for recursive linking) */
    cid_cache_t* cid_cache;

    /* Track allocated buffers for cleanup */
    void** allocated_buffers;
    size_t buffer_count;
    size_t buffer_capacity;

    vm_word_entry_t** vm_entries;
    size_t vm_entry_count;
    size_t vm_entry_capacity;

    /* Legacy: loaded words list (deprecated in favor of CID cache) */
    loaded_word_t** words;
    size_t word_count;
    size_t word_capacity;
} loader_t;

/* Create/free loader */
loader_t* loader_create(march_db_t* db, dictionary_t* dict);
void loader_free(loader_t* loader);

/* Load a word from database into memory */
loaded_word_t* loader_load_word(loader_t* loader, const char* name, const char* namespace);

/* Find loaded word by name */
loaded_word_t* loader_find_word(loader_t* loader, const char* name);

/* Link/resolve XT references in loaded words (legacy) */
bool loader_link(loader_t* loader);

/* Get entry point address for a word */
void* loader_get_entry_point(loader_t* loader, const char* name);

/* ============================================================================ */
/* CID-based linking functions (LINKING.md design) */
/* ============================================================================ */

/* Core linking function - recursively link a CID
 * Returns runtime address of linked code
 */
void* loader_link_cid(loader_t* loader, const unsigned char* cid);

/* Link a code blob (CID sequence) into runtime cells
 * Returns runtime address of linked code
 */
vm_word_entry_t* loader_link_code(loader_t* loader, const uint8_t* blob_data, size_t blob_len, int kind);

/* Helper: get primitive runtime address by ID */
void* loader_get_primitive_addr(loader_t* loader, uint16_t prim_id);

#endif /* MARCH_LOADER_H */
