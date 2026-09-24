# Syntax-neutral reader foundation

Status: B0b implemented. This is the substrate for a seed interpreter, not a
parser for the provisional syntax in `SYNTAX.md`.

## Operations

| Node | Result |
| --- | --- |
| `NextToken { text, position }` | record `{ found: Bool, token: Text, position: Int }` |
| `ParseInt(text)` | record `{ found: Bool, value: Int or Unit }` |
| `Lookup { record, key }` | record `{ found: Bool, value: held value or Unit }` |
| `PutKey { record, key, value }` | a new record with that text key bound/replaced |

`NextToken` uses UTF-8 byte offsets. It skips ASCII space, tab, LF, CR, vertical
tab, and form feed, then consumes non-whitespace bytes. Its successor cursor
is at the token's end, before subsequent whitespace. EOF returns `false`, an
empty token, and the input's byte length. Negative, out-of-range, and interior
UTF-8 code-unit cursors are errors. Non-ASCII whitespace remains token content.
There is no punctuation, comment, number, string, or definition recognition.

`ParseInt` accepts only an optional ASCII `+`/`-` followed by one or more ASCII
decimal digits within signed 64-bit range. Whitespace, empty/sign-only text,
other characters, and overflow return `{found: false, value: Unit}`. Wrong
operand types are errors. The seed decides when conversion should be tried.

Dynamic lookup distinguishes an absent key from a present `Unit`. The returned
value is a graph edge to an already-held object, not a CID parsed from text.
Updates preserve the original record. Records remain strict containers, and
quotations/families retain their existing dormant-body boundaries.

All four operations reduce their operands and preserve unresolved operations
across epochs. Known record structure permits lookup/update even when a field
value is still unknown. Substitution, canonical images, reflection, and code
validation understand all four nodes. Token scanning, conversion, and lookup
are allowed in pure guards; dynamic update has the same conservative guard
restriction as static `Put`.

The node codec adds tags 20–23 without changing existing node identities or
the V2 image envelope. New readers can load existing V2 images; older readers
reject new tags rather than interpret them differently. Reducer identity is
v7, so cached specialization results from older reducers are not reused.

## Evidence and limits

`tests/reader_graph.rs` builds a token-frequency reader entirely from guarded
families, records, recursion, and these primitives. Rust constructs the graph
and starts reduction; it does not iterate over source tokens. Tests establish:

- immutable dictionary updates, including repeated token names;
- identical state/image CIDs after save/reload at each boundary of an
  eight-token source;
- residualization before source text is known and continuation after reload;
- a stored quotation retrieved by token-derived key remains callable after
  reload and computes `49` from parsed `7`;
- a 10,000-token reader run without native recursive evaluation.

The boundary test keeps the complete source text and a cursor in state, pausing
after a token quota. It does **not** yet establish appending source chunks,
splitting inside tokens, or the compiler's B0.5 law.

Each consumed/scanned byte and each traversed/copied dictionary field charges
the reducer budget. This is not a total host-resource quota: existing graph
validation, cloning, allocation, hashing, and some structural work are not
fully charged. Records are flat vectors; growing a dictionary can require
quadratic cumulative copying. The general evaluator also clones text values.
Stack safety and deterministic identity do not establish linear runtime or
memory growth. The B0 scaling gate remains open; persistent text/dictionary
representations may be needed.

The B0c seed now supplies a symbolic stack, definition/quotation words,
number/name dispatch, and parsing aliases. See `SEED.md` for the executable
subset and the remaining self-extension/scaling questions. The Rust nucleus
continues to know none of their spellings.
