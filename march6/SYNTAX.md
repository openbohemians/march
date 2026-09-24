# Working surface syntax

Status: provisional seed-language choices, not yet implemented syntax. The
Rust nucleus does not recognize these spellings. Changing them should require
changing an image, not changing the reducer.

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

The bootstrap gate should also accept a conventional FORTH seed using
`: square dup * ;`, producing the same code CID with the same nucleus. The
representation is replaceable; the code identity is the comparison point.
