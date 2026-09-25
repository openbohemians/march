# Reference seed surface syntax

Reference implementation only: these spellings describe the old seed, not
the current [conventional source compiler](../FAST-SPIKE.md). See the
[reference index](README.md). Source paths and commands are relative to `march6/`.

Status: provisional seed-language choices; the B0c subset is executable. The
Rust nucleus does not recognize these spellings. Changing them should require
changing an image, not changing the reducer.

See `SEED.md` for commands, tests, exact supported forms, and limitations.

```text
answer : 6 7 * ;
square : ( dup * ) ;
7 square
```

`name : expression ;` binds the value computed by the expression in the
construction context. Thus `answer` holds `42`, while `square` holds a closed
quotation. Parentheses construct deferred code: they do not enforce a fixed
compile/runtime boundary. A quotation can be invoked whenever its inputs are
available, including during construction.

Both top-level arithmetic and word calls build pending expressions. `;`
observes the bound value; successful EOF observes the remaining stack. Quoted
code stays dormant. Token-quota pauses and image snapshots do not force pending
work. See `SEED.md` for this B0d demand contract and its current limits.

When extracting a fragment into a callable word, keep the parentheses:
`name : ( fragment ) ;`. Writing `name : fragment ;` instead computes a
constant at `;`, which can expose a failure even if the name is never used.
The alternative FORTH spelling `: name fragment ;` constructs callable code.

Bare data names push their values; callable names invoke their code. Quotation
construction uses a symbolic stack, with shuffles becoming graph wiring. The
first equivalence target is that `square` has exactly the CID of the hand-built
`Quote(1, Mul(Param(0), Param(0)))`, and `7 square` yields `49`.

`quote square` is the working spelling for retrieving held code without
invoking it. `a square`, `the square`, and tick are possible later aliases;
none is settled by this checkpoint. Named inputs, surface context guards,
global immutable-store slots (`=`), strings, comments, arrays, and lossless
formatting remain separate decisions.

Code values are currently closed, with explicit inputs and no implicit lexical
capture. This preserves fragment extraction and composition without silently
depending on a surrounding local scope. Closures are not ruled out forever,
but would require concrete evidence that they fit March's other goals.

The initial tokenizer splits on six ASCII whitespace bytes only. For now,
delimiters need separating whitespace: `square : ( dup * ) ;`. It does not
split `square:` or assign any meaning to punctuation. More elaborate reading
belongs above this foundation.

Integer literals take precedence over dictionary names and cannot be bound as
definition names, per the user's decision. This includes equivalent spellings
such as `007`, `+7`, and `-0`. The current literal grammar is exactly the
checked signed-64-bit decimal grammar of `ParseInt`; digit strings outside that
range are not supported integer literals. Broader numeric syntax is deferred.
Keys use exact UTF-8 bytes, without Unicode normalization: canonically
equivalent spellings remain distinct. Unicode policy remains provisional.

Line comments and whitespace-exact strings need additional text access beyond
`NextToken`. The original text remains in state, but the seed cannot currently
slice it or search for a newline/delimiter. A future syntax-neutral slicing and
search/scanning operation can supply that capability; no delimiter spelling
needs to become a nucleus rule. Strings/comments remain deferred here.

The bootstrap experiment also accepts a conventional FORTH seed using
`: square dup * ;`, producing the same code CID with the same nucleus. The
representation is replaceable; the code identity is the comparison point.
