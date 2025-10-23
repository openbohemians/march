#define _GNU_SOURCE
#include <stdint.h>
#include <stdio.h>
#include <inttypes.h>

typedef uint64_t Cell;
typedef int64_t  S64;
typedef uint64_t U64;

enum { TAG_XT = 0, TAG_LIT = 1, TAG_LST = 2, TAG_EXT = 3 }; /* EXT slot reused for experiments */

#define TAG_MASK   0x3u
#define TAG(x)     ((unsigned)((x) & TAG_MASK))
#define PAYLOAD(x) ((x) >> 2)
#define SEXT62(x)  ((S64)((S64)(x) >> 2))

#define DS_MAX 128
#define RS_MAX 128

#define PUSH(v)   (*sp++ = (v))
#define POP()     (*--sp)
#define RPUSH(v)  (*rp++ = (v))
#define RPOP()    (*--rp)

#define NEXT()    goto *tagtab[TAG(*ip++)]
#define JUMP(ptr) goto *(void*)(ptr)

#define XT(label)      ((Cell)(label))
#define LIT(v)         ((((Cell)(v)) << 2) | TAG_LIT)

int main(void) {
    /* VM state */
    static S64  ds[DS_MAX];
    static Cell *rs[RS_MAX];
    S64  *sp = ds;
    Cell **rp = rs;
    Cell *ip = NULL;

    /* Tag dispatch table */
    static void *tagtab[4];

    tagtab[TAG_XT]  = &&DISP_XT;
    tagtab[TAG_LIT] = &&DISP_LIT;
    tagtab[TAG_LST] = &&DISP_LNT;  /* reuse for literal run */
    tagtab[TAG_EXT] = &&DISP_BAD;

    static Cell square_body[] = {
        XT(&&prim_dup),
        XT(&&prim_mul),
        XT(&&prim_exit)
    };

    static Cell main_body[] = {
        LIT(5),
        XT(&&square_entry),
        LIT(3),
        XT(&&prim_add),
        XT(&&prim_exit)
    };

    goto VM_BEGIN;

    /* Primitive labels */
prim_add:
    {
        S64 b = POP();
        S64 a = POP();
        PUSH(a + b);
        NEXT();
    }

prim_mul:
    {
        S64 b = POP();
        S64 a = POP();
        PUSH(a * b);
        NEXT();
    }

prim_dup:
    {
        S64 a = *(sp - 1);
        PUSH(a);
        NEXT();
    }

prim_exit:
    {
        ip = RPOP();
        NEXT();
    }

square_entry:
    RPUSH(ip);
    ip = square_body;
    NEXT();

main_entry:
    RPUSH(ip);
    ip = main_body;
    NEXT();

VM_BEGIN:
    static Cell program[] = {
        XT(&&main_entry),
        (Cell)0  /* EXIT */
    };

    ip = program;
    /* ready to run */
    NEXT();

DISP_XT: {
    Cell xt = *(ip - 1);
    if (xt == 0) {
        if (rp == rs) {
            printf("Result = %" PRId64 "\n", (long long)*(sp - 1));
            return 0;
        }
        ip = RPOP();
        NEXT();
    }
    /* debug */
    /* printf("XT -> %p\n", (void*)xt); */
    JUMP(xt);
}

DISP_LIT: {
    Cell raw = *(ip - 1);
    PUSH(SEXT62(raw));
    NEXT();
}

DISP_LNT: {
    U64 n = PAYLOAD(*(ip - 1));
    for (U64 i = 0; i < n; ++i) {
        PUSH((S64)*ip++);
    }
    NEXT();
}

DISP_BAD:
    fprintf(stderr, "VM panic: tag=%u\n", TAG(*(ip - 1)));
    return 1;
}
