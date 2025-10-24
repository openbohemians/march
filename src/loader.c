/*
 * March Language - Word Loader Implementation
 * Implements CID-based linking algorithm from LINKING.md
 */

#define _POSIX_C_SOURCE 200809L

#include "loader.h"
#include "cells.h"
#include "primitives.h"
#include <stdlib.h>
#include <string.h>
#include <stdio.h>

/* ============================================================================ */
/* CID Cache Management */
/* ============================================================================ */

static unsigned int hash_cid_binary(const unsigned char* cid) {
    unsigned int hash = 0;
    /* Hash first 8 bytes of CID */
    for (int i = 0; i < 8; i++) {
        hash = hash * 31 + cid[i];
    }
    return hash % 256;
}

static cid_cache_t* cid_cache_create(void) {
    cid_cache_t* cache = calloc(1, sizeof(cid_cache_t));
    return cache;
}

static void cid_cache_free(cid_cache_t* cache) {
    if (!cache) return;
    for (int i = 0; i < 256; i++) {
        cid_cache_entry_t* entry = cache->buckets[i];
        while (entry) {
            cid_cache_entry_t* next = entry->next;
            free(entry);
            entry = next;
        }
    }
    free(cache);
}

static void cid_cache_put(cid_cache_t* cache, const unsigned char* cid, void* value, int kind) {
    unsigned int bucket = hash_cid_binary(cid);
    cid_cache_entry_t* entry = malloc(sizeof(cid_cache_entry_t));
    memcpy(entry->cid, cid, CID_SIZE);
    entry->value = value;
    entry->kind = kind;
    entry->next = cache->buckets[bucket];
    cache->buckets[bucket] = entry;
}

static void* cid_cache_get(cid_cache_t* cache, const unsigned char* cid, int* kind_out) {
    unsigned int bucket = hash_cid_binary(cid);
    cid_cache_entry_t* entry = cache->buckets[bucket];
    while (entry) {
        if (memcmp(entry->cid, cid, CID_SIZE) == 0) {
            if (kind_out) *kind_out = entry->kind;
            return entry->value;
        }
        entry = entry->next;
    }
    return NULL;
}

/* Create loader */
loader_t* loader_create(march_db_t* db, dictionary_t* dict) {
    loader_t* loader = malloc(sizeof(loader_t));
    if (!loader) return NULL;

    loader->db = db;
    loader->dict = dict;

    /* Initialize CID cache */
    loader->cid_cache = cid_cache_create();
    if (!loader->cid_cache) {
        free(loader);
        return NULL;
    }

    /* Initialize buffer tracking */
    loader->buffer_capacity = 64;
    loader->buffer_count = 0;
    loader->allocated_buffers = calloc(loader->buffer_capacity, sizeof(void*));
    if (!loader->allocated_buffers) {
        cid_cache_free(loader->cid_cache);
        free(loader);
        return NULL;
    }

    loader->vm_entry_capacity = 32;
    loader->vm_entry_count = 0;
    loader->vm_entries = calloc(loader->vm_entry_capacity, sizeof(vm_word_entry_t*));
    if (!loader->vm_entries) {
        free(loader->allocated_buffers);
        cid_cache_free(loader->cid_cache);
        free(loader);
        return NULL;
    }

    /* Legacy word list */
    loader->word_capacity = 64;
    loader->word_count = 0;
    loader->words = calloc(loader->word_capacity, sizeof(loaded_word_t*));
    if (!loader->words) {
        free(loader->allocated_buffers);
        free(loader->vm_entries);
        cid_cache_free(loader->cid_cache);
        free(loader);
        return NULL;
    }

    return loader;
}

/* Free loaded word */
static void loaded_word_free(loaded_word_t* word) {
    if (word) {
        free(word->name);
        free(word->cells);
        free(word);
    }
}

/* Free loader */
void loader_free(loader_t* loader) {
    if (loader) {
        /* Free legacy loaded words */
        for (size_t i = 0; i < loader->word_count; i++) {
            loaded_word_free(loader->words[i]);
        }
        free(loader->words);

        for (size_t i = 0; i < loader->vm_entry_count; i++) {
            vm_free_entry(loader->vm_entries[i]);
        }
        free(loader->vm_entries);

        /* Free allocated buffers (from linking) */
        for (size_t i = 0; i < loader->buffer_count; i++) {
            free(loader->allocated_buffers[i]);
        }
        free(loader->allocated_buffers);

        /* Free CID cache */
        cid_cache_free(loader->cid_cache);

        free(loader);
    }
}

/* Find loaded word by name */
loaded_word_t* loader_find_word(loader_t* loader, const char* name) {
    for (size_t i = 0; i < loader->word_count; i++) {
        if (strcmp(loader->words[i]->name, name) == 0) {
            return loader->words[i];
        }
    }
    return NULL;
}

/* Load a word from database into memory */
loaded_word_t* loader_load_word(loader_t* loader, const char* name, const char* namespace) {
    /* Check if already loaded */
    loaded_word_t* existing = loader_find_word(loader, name);
    if (existing) {
        return existing;
    }

    /* Load from database */
    size_t cell_count = 0;
    uint64_t* cells = db_load_word(loader->db, name, namespace, &cell_count);
    if (!cells) {
        fprintf(stderr, "Failed to load word: %s\n", name);
        return NULL;
    }

    /* Create loaded word structure */
    loaded_word_t* word = malloc(sizeof(loaded_word_t));
    if (!word) {
        free(cells);
        return NULL;
    }

    word->name = strdup(name);
    word->cells = cells;
    word->cell_count = cell_count;
    word->entry_point = (void*)cells;  /* Entry point is address of first cell */

    /* Add to loaded words cache */
    if (loader->word_count >= loader->word_capacity) {
        loader->word_capacity *= 2;
        loader->words = realloc(loader->words, loader->word_capacity * sizeof(loaded_word_t*));
    }
    loader->words[loader->word_count++] = word;

    return word;
}

/* Get entry point address for a word */
void* loader_get_entry_point(loader_t* loader, const char* name) {
    loaded_word_t* word = loader_find_word(loader, name);
    if (word) {
        return word->entry_point;
    }

    /* Try loading it */
    word = loader_load_word(loader, name, "user");
    if (word) {
        return word->entry_point;
    }

    return NULL;
}

/* Link/resolve XT references in loaded words (legacy - deprecated) */
bool loader_link(loader_t* loader) {
    /* Legacy function - with CID-based linking, this is no longer needed */
    return true;
}

/* ============================================================================ */
/* CID-Based Linking Implementation (LINKING.md design) */
/* ============================================================================ */

/* Helper: track allocated buffer for cleanup */
static void track_buffer(loader_t* loader, void* buffer) {
    if (loader->buffer_count >= loader->buffer_capacity) {
        loader->buffer_capacity *= 2;
        loader->allocated_buffers = realloc(loader->allocated_buffers,
                                           loader->buffer_capacity * sizeof(void*));
    }
    loader->allocated_buffers[loader->buffer_count++] = buffer;
}

static void track_vm_entry(loader_t* loader, vm_word_entry_t* entry) {
    if (loader->vm_entry_count >= loader->vm_entry_capacity) {
        loader->vm_entry_capacity *= 2;
        loader->vm_entries = realloc(loader->vm_entries,
                                     loader->vm_entry_capacity * sizeof(vm_word_entry_t*));
    }
    loader->vm_entries[loader->vm_entry_count++] = entry;
}

/* Get primitive runtime address by ID */
void* loader_get_primitive_addr(loader_t* loader, uint16_t prim_id) {
    /* Simple array lookup using the dispatch table */
    if (prim_id >= 256) {
        fprintf(stderr, "Error: Invalid primitive ID %u (max 255)\n", prim_id);
        return NULL;
    }

    void* addr = primitive_dispatch_table[prim_id];
    if (!addr) {
        fprintf(stderr, "Error: Primitive #%u not registered\n", prim_id);
    }
    return addr;
}

/* Core linking function - recursively link a CID
 * Implements the algorithm from LINKING.md
 */
void* loader_link_cid(loader_t* loader, const unsigned char* cid) {
    int cached_kind = -1;
    void* cached = cid_cache_get(loader->cid_cache, cid, &cached_kind);
    if (cached) {
        return cached;
    }

    /* Load blob metadata from database */
    int kind = 0;
    unsigned char* sig_cid = NULL;
    uint8_t* blob_data = NULL;
    size_t blob_len = 0;

    if (!db_load_blob_ex(loader->db, cid, &kind, &sig_cid, &blob_data, &blob_len)) {
        char* cid_hex = cid_to_hex(cid);
        fprintf(stderr, "Error: Blob not found for CID %s\n", cid_hex);
        free(cid_hex);
        return NULL;
    }

    void* result = NULL;

    switch (kind) {
        case BLOB_PRIMITIVE:
            /* Primitives should never be referenced by CID - they use 2-byte IDs in blob encoding */
            {
                char* cid_hex = cid_to_hex(cid);
                fprintf(stderr, "Error: Primitive referenced by CID %s (primitives should use ID-based encoding)\n", cid_hex);
                free(cid_hex);
            }
            break;

        case BLOB_WORD:
        case BLOB_QUOTATION:
            result = loader_link_code(loader, blob_data, blob_len, kind);
            break;

        case BLOB_DATA:
            /* Literal value - allocate and copy */
            result = malloc(blob_len);
            if (result) {
                memcpy(result, blob_data, blob_len);
                track_buffer(loader, result);
            }
            break;

        default:
            {
                char* cid_hex = cid_to_hex(cid);
                fprintf(stderr, "Error: Unknown blob kind %d for CID %s\n", kind, cid_hex);
                free(cid_hex);
            }
            break;
    }

    /* Free loaded data */
    if (sig_cid) free(sig_cid);
    if (blob_data) free(blob_data);

    /* Cache result */
    if (result) {
        cid_cache_put(loader->cid_cache, cid, result, kind);
    }

    return result;
}

/* Link a code blob (CID sequence) into runtime cells
 * Implements the algorithm from LINKING.md
 */
vm_word_entry_t* loader_link_code(loader_t* loader, const uint8_t* blob_data, size_t blob_len, int kind) {
    (void)kind;
    /* Allocate runtime cell buffer (estimate size, expand if needed) */
    size_t capacity = 64;
    size_t count = 0;
    cell_t* cells = malloc(capacity * sizeof(cell_t));
    if (!cells) return NULL;

    const uint8_t* ptr = blob_data;
    const uint8_t* end = blob_data + blob_len;

    while (ptr < end) {
        bool is_cid;
        uint16_t id_or_kind;
        const unsigned char* cid;

        /* Decode next tag */
        ptr = decode_tag_ex(ptr, &is_cid, &id_or_kind, &cid);

        /* Expand buffer if needed */
        if (count >= capacity) {
            capacity *= 2;
            cells = realloc(cells, capacity * sizeof(cell_t));
            if (!cells) return NULL;
        }

        if (!is_cid) {
            /* Primitive: look up runtime address by ID */
            void* prim_addr = loader_get_primitive_addr(loader, id_or_kind);
            if (!prim_addr) {
                free(cells);
                return NULL;
            }
            cells[count++] = encode_xt(prim_addr);
        } else {
            /* CID reference: recursively link */
            void* addr = loader_link_cid(loader, cid);
            if (!addr) {
                free(cells);
                return NULL;
            }

            /* The kind field determines how to use this reference */
            switch (id_or_kind) {
                case BLOB_WORD:
                {
                    vm_word_entry_t* entry = (vm_word_entry_t*)addr;
                    cells[count++] = encode_xt(entry->entry);
                    break;
                }
                case BLOB_PRIMITIVE:
                    fprintf(stderr, "Error: unexpected primitive CID reference\n");
                    free(cells);
                    return NULL;
                case BLOB_QUOTATION:
                {
                    vm_word_entry_t* entry = (vm_word_entry_t*)addr;
                    cells[count++] = encode_lit((int64_t)entry->entry);
                    break;
                }
                case BLOB_DATA:
                    /* Push the value */
                    {
                        int64_t value = *(int64_t*)addr;
                        cells[count++] = encode_lit(value);
                    }
                    break;

                default:
                    fprintf(stderr, "Error: Unknown blob kind %u in linking\n", id_or_kind);
                    free(cells);
                    return NULL;
            }
        }
    }

    /* Append EXIT */
    if (count >= capacity) {
        capacity++;
        cells = realloc(cells, capacity * sizeof(cell_t));
        if (!cells) return NULL;
    }
    cells[count++] = encode_exit();

    /* Shrink to exact size */
    cells = realloc(cells, count * sizeof(cell_t));

    /* Track for cleanup */
    track_buffer(loader, cells);

    if (kind == BLOB_WORD) {
        printf("Linked word cells (%zu):\n", count);
        for (size_t i = 0; i < count; i++) {
            printf("  cell[%zu] = %llu\n", i, (unsigned long long)cells[i]);
        }
    }
    vm_word_entry_t* entry = vm_make_entry(cells);
    if (!entry) {
        return NULL;
    }
    track_vm_entry(loader, entry);
    return entry;
}
