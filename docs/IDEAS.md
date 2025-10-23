# IDEAS

## Computed GOTO

```
// GCC/Clang (computed goto). 64-bit target assumed.
#include <stdint.h>
#include <stdio.h>

typedef uint64_t Cell;   // raw 64-bit cell from image
typedef int64_t  S64;
typedef uint64_t U64;

enum { TAG_XT=0, TAG_LIT=1, TAG_LITN=2, TAG_LITSYM=3 };
#define TAG_MASK   0x3u
#define TAG(x)     ((unsigned)((x) & TAG_MASK))
#define PAYLOAD(x) ((x) >> 2)              // upper 62 bits
#define SEXT62(x)  ((S64)((S64)(x) >> 2))  // arithmetic shift for signed LIT

// Simple stacks (add bounds checks in real code)
#define DS_MAX (1<<16)
#define RS_MAX (1<<16)
static S64  ds[DS_MAX], *sp = ds;
static Cell *rs[RS_MAX], **rp = rs;

// VM ip points into your prelinked cell stream
static Cell *ip;

// Stack ops
#define PUSH(x)    (*sp++ = (x))
#define POP()      (*--sp)
#define RPUSH(x)   (*rp++ = (x))
#define RPOP()     (*--rp)

// Dispatch helpers
#define NEXT()     goto *tagtab[TAG(*ip++)]  // fetch & dispatch by 2-bit tag
#define JUMP(ptr)  goto *(void*)(ptr)        // raw pointer jump (XT==00)

// Tag dispatch table
static void *tagtab[4];

// --------- Primitives (labels) ---------
prim_add: { S64 b=POP(), a=POP(); PUSH(a+b); NEXT(); }
prim_mul: { S64 b=POP(), a=POP(); PUSH(a*b); NEXT(); }
prim_dup: { S64 a=*(sp-1);        PUSH(a);          NEXT(); }

// Return from user word (colon def)
prim_exit: { ip = (Cell*) *RPOP(); NEXT(); }

// --------- User word entry stub (per-word DOCOL) ---------
// Each colon word gets a tiny stub that saves ip and jumps into its body array.
#define DEFINE_WORD(NAME, /*cells...*/ ...)       \
    NAME##_entry: {                               \
        RPUSH((Cell*)ip);                         \
        ip = NAME##_body;                         \
        NEXT();                                   \
    }                                             \
    static Cell NAME##_body[] = { __VA_ARGS__ }

// --------- Tag handlers ---------
void run(Cell *program) {
    ip = program;

    // Build tag table
    tagtab[TAG_XT]    = &&DISP_XT;
    tagtab[TAG_LIT]   = &&DISP_LIT;
    tagtab[TAG_LITN]  = &&DISP_LITN;
    tagtab[TAG_LITSYM]= &&DISP_LITSYM;

    // --- Define user words (entry stub + body) right here ---
    // : square  dup *  exit ;
    DEFINE_WORD(square,
        (Cell)&&prim_dup,   // XT cells are raw pointers with tag 00
        (Cell)&&prim_mul,
        (Cell)&&prim_exit
    );

    // --- Example top-level program stream ---
    //   5 square 3 +  EXIT
    static Cell program_image[] = {
        ((U64)5 << 2) | TAG_LIT,         // LIT 5
        (Cell)&&square_entry,            // XT of user word entry stub
        ((U64)3 << 2) | TAG_LIT,         // LIT 3
        (Cell)&&prim_add,                // XT add
        (Cell)0                          // XT == 0 → EXIT sentinel
    };

    ip = program_image;
    NEXT();

DISP_XT: { // XT path: *ip just advanced; the cell we fetched is at ip-1
    Cell xt = *(ip - 1);                // raw pointer or 0
    if (__builtin_expect(xt == 0, 0)) {
        // All-zero cell means EXIT at top level: stop VM
        // (optionally: if rp not empty, you could pop and continue)
        return;
    }
    JUMP(xt);                           // jump to primitive or entry stub
}

DISP_LIT: {
    Cell raw = *(ip - 1);
    PUSH(SEXT62(raw));
    NEXT();
}

DISP_LITN: {
    U64 n = PAYLOAD(*(ip - 1));         // count in upper 62 bits
    for (U64 i = 0; i < n; ++i) PUSH((S64)*ip++);
    NEXT();
}

DISP_LITSYM: {
    U64 sym = PAYLOAD(*(ip - 1));       // small symbol id
    PUSH((S64)sym);                     // or look up handle
    NEXT();
}
}

// ----- demo -----
int main(void) {
    run(NULL); // run() builds and runs program_image internally
    printf("TOS = %lld\n", (long long)*(sp-1)); // should be (5*5)+3 = 28
    return 0;
}
```

- Notes for later (from trampoline experiment):
  - Keep return-stack bookkeeping explicit if we reintroduce runtime-generated stubs.
  - Decide whether loader owns DOCOL-style entry stubs or whether a shared helper suffices.


## Iteraction Nets


// inet_exec.c — tiny parallel executor skeleton for interaction nets
// Build: cc -O3 -std=c11 -pthread inet_exec.c -o inet_exec
// Demo main() at bottom.

#include <stdio.h>
#include <stdlib.h>
#include <stdatomic.h>
#include <pthread.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>

// ------------------------- node model -------------------------

typedef enum { COST_CHEAP = 0, COST_HEAVY = 1 } cost_t;

// Bitmask of effect classes (set multiple bits to serialize on several)
// e.g., 1<<0 = IO, 1<<1 = DB, etc.
typedef uint32_t effect_t;

typedef struct Node Node;
typedef void (*node_fn)(void *ctx);

// A node is ready when deps==0. On completion, it decrements deps of its succs.
struct Node {
    node_fn   fn;
    void     *ctx;
    _Atomic int deps;     // remaining incoming edges
    cost_t    cost;
    effect_t  effects;    // effect mask; 0 = pure
    int      *succ;       // indices of successor nodes
    int       succ_cnt;
};

// ------------------------- ready deques -------------------------

typedef struct {
    int *buf;                 // circular buffer of indices
    int cap;
    int head, tail;           // head pops from head, pushes at tail
    pthread_mutex_t mu;
} Deque;

static void dq_init(Deque *q, int cap) {
    q->buf = malloc(sizeof(int)*cap);
    q->cap = cap; q->head = q->tail = 0;
    pthread_mutex_init(&q->mu, NULL);
}
static void dq_free(Deque *q){ pthread_mutex_destroy(&q->mu); free(q->buf); }

static int dq_push_tail(Deque *q, int v){
    pthread_mutex_lock(&q->mu);
    int next = (q->tail + 1) % q->cap;
    if (next == q->head) { pthread_mutex_unlock(&q->mu); return 0; } // full
    q->buf[q->tail] = v; q->tail = next;
    pthread_mutex_unlock(&q->mu);
    return 1;
}
static int dq_pop_head(Deque *q, int *out){
    pthread_mutex_lock(&q->mu);
    if (q->head == q->tail){ pthread_mutex_unlock(&q->mu); return 0; }
    int v = q->buf[q->head]; q->head = (q->head + 1) % q->cap;
    pthread_mutex_unlock(&q->mu);
    *out = v; return 1;
}
static int dq_steal_head(Deque *q, int *out){ return dq_pop_head(q, out); } // same lock for simplicity

// ------------------------- executor state -------------------------

typedef struct {
    Node     *nodes;
    int       node_cnt;
    int       nworkers;
    Deque    *queues;             // one per worker
    _Atomic int remaining;        // nodes left to execute
    _Atomic int done;             // global completion flag

    // effect tokens: one atomic flag per bit position (32 max here)
    atomic_flag effect_token[32];
} Exec;

typedef struct {
    Exec *E;
    int   wid;
} WorkerCtx;

// Try to acquire all needed effect tokens; return 1 on success, else 0.
static int acquire_effects(Exec *E, effect_t eff){
    if (!eff) return 1;
    // optimistic: acquire in ascending bit order; on failure, release acquired ones
    int acquired[32], ac = 0;
    for (int bit=0; bit<32; ++bit){
        if (!(eff & (1u<<bit))) continue;
        if (atomic_flag_test_and_set(&E->effect_token[bit]) == 0){
            acquired[ac++] = bit;
        } else {
            // release earlier
            for (int i=0;i<ac;i++) atomic_flag_clear(&E->effect_token[acquired[i]]);
            return 0;
        }
    }
    return 1;
}
static void release_effects(Exec *E, effect_t eff){
    if (!eff) return;
    for (int bit=0; bit<32; ++bit) if (eff & (1u<<bit)) atomic_flag_clear(&E->effect_token[bit]);
}

// Push a node that just became ready onto some queue (round-robin by hint worker)
static void enqueue_ready(Exec *E, int hint_wid, int node_idx){
    // try hint's queue first, fall back to others
    if (dq_push_tail(&E->queues[hint_wid], node_idx)) return;
    for (int i=0;i<E->nworkers;i++){
        if (dq_push_tail(&E->queues[(hint_wid+i)%E->nworkers], node_idx)) return;
    }
    // if all full, spin once then push (toy handling)
    usleep(10);
    dq_push_tail(&E->queues[hint_wid], node_idx);
}

// ------------------------- worker loop -------------------------

#define BATCH_MAX 32

static void *worker_main(void *arg){
    WorkerCtx *WC = (WorkerCtx*)arg;
    Exec *E = WC->E;
    int wid = WC->wid;

    int local[BATCH_MAX]; int nlocal = 0;

    while (!atomic_load(&E->done)){
        int idx = -1;

        // 1) Pop local work
        if (!dq_pop_head(&E->queues[wid], &idx)){
            // 2) Steal
            for (int s=1; s<=E->nworkers; ++s){
                if (dq_steal_head(&E->queues[(wid+s)%E->nworkers], &idx)) break;
            }
            if (idx < 0){
                // 3) Check for termination
                if (atomic_load(&E->remaining) == 0){
                    atomic_store(&E->done, 1);
                    break;
                }
                // Nothing now; yield
                sched_yield();
                continue;
            }
        }

        Node *n = &E->nodes[idx];

        // If effects present and cannot acquire now, requeue and continue
        if (n->effects && !acquire_effects(E, n->effects)){
            dq_push_tail(&E->queues[wid], idx);
            continue;
        }

        // Batch cheap nodes (no effects) to reduce overhead
        if (n->cost == COST_CHEAP && n->effects == 0){
            local[0] = idx; nlocal = 1;
            while (nlocal < BATCH_MAX){
                int j=-1;
                if (!dq_pop_head(&E->queues[wid], &j)) break;
                Node *m = &E->nodes[j];
                if (m->cost != COST_CHEAP || m->effects != 0){ // not batchable
                    dq_push_tail(&E->queues[wid], j);
                    break;
                }
                local[nlocal++] = j;
            }
            // Run batch
            for (int k=0;k<nlocal;k++){
                Node *b = &E->nodes[local[k]];
                b->fn(b->ctx);
                // fanout
                for (int s=0;s<b->succ_cnt;s++){
                    int tgt = b->succ[s];
                    int prev = atomic_fetch_sub(&E->nodes[tgt].deps, 1);
                    if (prev == 1) enqueue_ready(E, wid, tgt);
                }
                atomic_fetch_sub(&E->remaining, 1);
            }
            // no effects to release
        } else {
            // Run single node
            n->fn(n->ctx);
            for (int s=0;s<n->succ_cnt;s++){
                int tgt = n->succ[s];
                int prev = atomic_fetch_sub(&E->nodes[tgt].deps, 1);
                if (prev == 1) enqueue_ready(E, wid, tgt);
            }
            atomic_fetch_sub(&E->remaining, 1);
            // release effects if any
            if (n->effects) release_effects(E, n->effects);
        }
    }
    return NULL;
}

// ------------------------- public API -------------------------

// Initialize exec with nodes[] (deps, succs must be prefilled). Seed ready queues.
static void exec_run_parallel(Node *nodes, int node_cnt, int nworkers){
    Exec E = {0};
    E.nodes = nodes; E.node_cnt = node_cnt; E.nworkers = nworkers;
    atomic_store(&E.remaining, node_cnt);
    for (int i=0;i<32;i++) atomic_flag_clear(&E.effect_token[i]);

    // Queues
    E.queues = calloc(nworkers, sizeof(Deque));
    for (int i=0;i<nworkers;i++) dq_init(&E.queues[i], 4096);

    // Seed READY nodes
    for (int i=0;i<node_cnt;i++){
        if (atomic_load(&nodes[i].deps) == 0){
            enqueue_ready(&E, i % nworkers, i);
        }
    }

    // Workers
    pthread_t *th = malloc(sizeof(pthread_t)*nworkers);
    WorkerCtx *wc = malloc(sizeof(WorkerCtx)*nworkers);
    for (int i=0;i<nworkers;i++){ wc[i].E=&E; wc[i].wid=i; pthread_create(&th[i], NULL, worker_main, &wc[i]); }
    for (int i=0;i<nworkers;i++) pthread_join(th[i], NULL);

    for (int i=0;i<nworkers;i++) dq_free(&E.queues[i]);
    free(E.queues); free(th); free(wc);
}

// ------------- helpers to build a tiny graph for demo -------------

typedef struct { int *dst; int cnt; } SuccBuf;

static void add_edge(Node *N, SuccBuf *buf, int from, int to){
    buf[from].dst = realloc(buf[from].dst, sizeof(int)*(buf[from].cnt+1));
    buf[from].dst[buf[from].cnt++] = to;
    atomic_fetch_add(&N[to].deps, 1);
}

static void finalize_succs(Node *N, SuccBuf *buf, int n){
    for (int i=0;i<n;i++){
        N[i].succ_cnt = buf[i].cnt;
        N[i].succ = buf[i].dst;
    }
    free(buf);
}

// ------------------------- demo program -------------------------

// Example kernels
static void mul_ctx(void *p){ int64_t *v=(int64_t*)p; *v = (*v) * (*v); }          // x = x*x
static void add3_ctx(void *p){ int64_t *v=(int64_t*)p; *v = (*v) + 3; }            // x += 3
static void add_ctx(void *p){ int64_t *ab=(int64_t*)p; ab[0] = ab[0] + ab[1]; }    // a+=b
static void print_ctx(void *p){ printf("result = %ld\n", *(int64_t*)p); fflush(stdout); } // IO

int main(void){
    // Graph:
    // t2 = x*x      (pure)
    // t3 = y*y      (pure)
    // t4 = t2 + t3  (pure)
    // t4 = t4 + 3   (pure)
    // print t4      (io)
    int Nn = 5;
    Node *N = calloc(Nn, sizeof(Node));
    SuccBuf *B = calloc(Nn, sizeof(SuccBuf));

    int64_t x=5, y=7;
    int64_t t2 = x, t3 = y;
    int64_t t4_pair[2]; // [t4, tmp]

    N[0] = (Node){ .fn=mul_ctx, .ctx=&t2, .deps=0, .cost=COST_CHEAP, .effects=0 };
    N[1] = (Node){ .fn=mul_ctx, .ctx=&t3, .deps=0, .cost=COST_CHEAP, .effects=0 };
    // t4 = t2 + t3 (use pair buffer where [0]=t2 gets updated by add_ctx, [1]=t3)
    t4_pair[0]=t2; t4_pair[1]=t3;
    N[2] = (Node){ .fn=add_ctx, .ctx=&t4_pair[0], .deps=0, .cost=COST_CHEAP, .effects=0 };
    // t4 += 3
    N[3] = (Node){ .fn=add3_ctx, .ctx=&t4_pair[0], .deps=0, .cost=COST_CHEAP, .effects=0 };
    // print(t4) — serialize on IO token bit 0
    N[4] = (Node){ .fn=print_ctx, .ctx=&t4_pair[0], .deps=0, .cost=COST_HEAVY, .effects=(1u<<0) };

    // Edges: 0->2, 1->2, 2->3, 3->4
    add_edge(N, B, 0, 2);
    add_edge(N, B, 1, 2);
    add_edge(N, B, 2, 3);
    add_edge(N, B, 3, 4);
    finalize_succs(N, B, Nn);

    int nworkers = (int)sysconf(_SC_NPROCESSORS_ONLN);
    if (nworkers < 2) nworkers = 2;
    exec_run_parallel(N, Nn, nworkers);

    free(N);
    return 0;
}
