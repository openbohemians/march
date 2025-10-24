#define _GNU_SOURCE
#include "vm_api.h"
#include "primitives.h"
#include <sys/mman.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <stdio.h>
#include <unistd.h>

#define DATA_STACK_WORDS   VM_DATA_STACK_WORDS
#define RETURN_STACK_WORDS VM_RETURN_STACK_WORDS

#define TAG_MASK 0x3u
#define TAG(cell) ((cell) & TAG_MASK)

uint64_t data_stack_base[DATA_STACK_WORDS];

static cell_t return_stack_base[RETURN_STACK_WORDS];

static cell_t *vm_ip_store;
static int64_t *vm_sp_store;
static cell_t **vm_rp_store;

static void vm_dispatch_loop(void);
void vm_resume_entry(void);

static inline void vm_sync_from_regs(void) {
    cell_t *ip;
    int64_t *sp;
    cell_t **rp;
    __asm__ volatile(
        "mov %%rbx, %0\n\t"
        "mov %%rsi, %1\n\t"
        "mov %%rdi, %2\n\t"
        : "=r"(ip), "=r"(sp), "=r"(rp)
        :
        : "rbx", "rsi", "rdi");
    vm_ip_store = ip;
    vm_sp_store = sp;
    vm_rp_store = rp;
}

static inline void vm_sync_to_regs(void) {
    cell_t *ip = vm_ip_store;
    int64_t *sp = vm_sp_store;
    cell_t **rp = vm_rp_store;
    __asm__ volatile(
        "mov %0, %%rbx\n\t"
        "mov %1, %%rsi\n\t"
        "mov %2, %%rdi\n\t"
        :
        : "r"(ip), "r"(sp), "r"(rp)
        : "rbx", "rsi", "rdi");
}

void vm_init(void) {
    vm_sp_store = (int64_t*)(data_stack_base + DATA_STACK_WORDS);
    vm_rp_store = (cell_t**)(return_stack_base + RETURN_STACK_WORDS);
    vm_ip_store = NULL;
}

void vm_run(cell_t *entry) {
    vm_sp_store = (int64_t*)(data_stack_base + DATA_STACK_WORDS);
    vm_rp_store = (cell_t**)(return_stack_base + RETURN_STACK_WORDS);
    vm_ip_store = entry;
    vm_dispatch_loop();
}

void vm_resume_entry(void) {
    vm_sync_from_regs();
    vm_dispatch_loop();
    vm_sync_to_regs();
}

static void vm_dispatch_loop(void) {
    static void *tagtab[] = { &&DISP_XT, &&DISP_LIT, &&DISP_MIX, &&DISP_EXT };
    goto *tagtab[TAG(*vm_ip_store++)];

DISP_XT: {
        cell_t xt = *(vm_ip_store - 1);
        printf("DISP_XT cell=%p\n", (void*)xt);
        if (xt == 0) {
            if (vm_rp_store == (cell_t**)(return_stack_base + RETURN_STACK_WORDS)) {
                return;
            }
            vm_ip_store = (cell_t*)(*vm_rp_store++);
            goto *tagtab[TAG(*vm_ip_store++)];
        }
        vm_sync_to_regs();
        ((void (*)(void))xt)();
        vm_sync_from_regs();
        printf("VM XT after call sp=%p value=%lld\n", (void*)vm_sp_store,
               (long long)*vm_sp_store);
        goto *tagtab[TAG(*vm_ip_store++)];
    }

DISP_LIT: {
        cell_t raw = *(vm_ip_store - 1);
        printf("DISP_LIT raw=%llu\n", (unsigned long long)raw);
        vm_sp_store--;
        *vm_sp_store = (int64_t)((int64_t)raw >> 2);
        goto *tagtab[TAG(*vm_ip_store++)];
    }

DISP_MIX: {
        cell_t raw = *(vm_ip_store - 1);
        printf("DISP_MIX raw=%llu\n", (unsigned long long)raw);
        if (raw & 0x4) {
            uint64_t count = raw >> 3;
            while (count--) {
                vm_sp_store--;
                *vm_sp_store = (int64_t)(*vm_ip_store++);
            }
            goto *tagtab[TAG(*vm_ip_store++)];
        } else {
            vm_sp_store--;
            *vm_sp_store = (int64_t)(raw >> 3);
            goto *tagtab[TAG(*vm_ip_store++)];
        }
    }

DISP_EXT:
    fprintf(stderr, "VM panic: unsupported EXT tag\n");
    return;
}

vm_word_entry_t* vm_make_entry(cell_t *body) {
    long page = sysconf(_SC_PAGESIZE);
    if (page < 0) page = 4096;
    uint8_t *stub = mmap(NULL, page, PROT_READ | PROT_WRITE | PROT_EXEC,
                         MAP_PRIVATE | MAP_ANON, -1, 0);
    if (stub == MAP_FAILED) {
        return NULL;
    }

    uint8_t *code = stub;
    /* sub rdi, 8 */
    *code++ = 0x48;
    *code++ = 0x83;
    *code++ = 0xEF;
    *code++ = 0x08;
    /* mov [rdi], rbx */
    *code++ = 0x48;
    *code++ = 0x89;
    *code++ = 0x1F;
    /* mov rbx, imm64 */
    *code++ = 0x48;
    *code++ = 0xBB;
    memcpy(code, &body, sizeof(body));
    code += sizeof(body);
    /* mov rax, vm_resume_entry */
    void *resume = (void*)&vm_resume_entry;
    *code++ = 0x48;
    *code++ = 0xB8;
    memcpy(code, &resume, sizeof(resume));
    code += sizeof(resume);
    /* jmp rax */
    *code++ = 0xFF;
    *code++ = 0xE0;

    __builtin___clear_cache((char*)stub, (char*)code);

    vm_word_entry_t *entry = malloc(sizeof(vm_word_entry_t));
    if (!entry) {
        munmap(stub, page);
        return NULL;
    }
    entry->body = body;
    entry->entry = (void (*)(void))stub;
    entry->stub_mem = stub;
    entry->stub_size = (size_t)page;
    return entry;
}

void vm_free_entry(vm_word_entry_t *entry) {
    if (!entry) return;
    if (entry->stub_mem) {
        munmap(entry->stub_mem, entry->stub_size);
    }
    free(entry);
}

uint64_t* vm_get_dsp(void) {
    return (uint64_t*)vm_sp_store;
}
