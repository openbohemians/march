# Strings

Status: first slice, built 2026-10-05, following doc/design/TEXT.md: UTF-8
throughout, no UTF-32 step and no separate ASCII type.

## What a string is

A string is one cell: a handle to UTF-8 text that the machine holds in a
region slot, as it holds arrays. The text is a merkle-champ
`Sequence<TextByte>` (merkle-champ FORMAT.md section 10):
- **Its shape depends only on the text.** Leaves end where a rolling hash
  over the bytes says, so equal text has one identity however it was built,
  and joining, slicing or inserting rewrites a few nodes near the change.
- **Its branches count characters and lines.** Each branch records, per
  child, how many code points and newlines it holds, so "the n-th character"
  is one descent, not a scan.

The checker types a string 253. It is not an array to the checker, so
families do not lift over its characters: `"abc" 1 +` is an error (trap 23).
A character read from a string is its code point, an i64.

## Literals

A token that starts with `"` is a string literal. The text runs from after
that quote to the next `"`, raw: it may hold spaces, other scripts and
newlines, and there are no escapes yet, so a literal cannot contain `"`.

```
"hello"
"héllo 中文 😀"
""
```

Compiling, the text becomes a data object in the code and primitive 52
(`text`) makes it a string each time the code runs; at top level the string
is made at once. `seed/system.march` itself keeps FORTH's `s" text"`, because
the frozen generation 0 reads it.

## Words

| Word | Effect | On strings | On arrays |
|---|---|---|---|
| `length` | `( a -- n )` | characters (code points) | elements |
| `at` | `( a i -- x )` | the i-th character's code point | the i-th element |
| `concat` | `( a b -- c )` | joined | joined |
| `slice` | `( a i j -- b )` | characters i to j - 1 | elements i to j - 1 |
| `same?` | `( a b -- flag )` | equal text | equal elements |

- `length` and `at` are the array words; the machine looks at what the handle
  holds. So `each`, `fold` and `map` walk a string's characters:
  `"abc" [ 1 + ] map` is `( 98 99 100 )`.
- **Strings are interned** (since maps, docs/MAPS.md): equal text is always
  the same handle, so `eq?` compares strings correctly, and strings serve as
  map keys. `same?` compares contents through identities, for arrays and
  maps as well as strings.
- `concat`, `slice` and `same?` are primitives 53, 54 and 55, and work on
  arrays too.
- Errors: reading or slicing past the end, a slice whose end is before its
  start, and joining a string with an array are memory errors. A literal with
  no closing quote traps 8.

## Showing strings

The command line shows a string as March writes it: `<1> "hello"`.

## Not yet

- **Byte positions and searching.** TEXT.md's `bytes` and `chars` words,
  slicing by byte position, and searching for a substring.
- **Characters as a type.** A character is an i64 for now; a type of its own
  needs a literal and display syntax (TEXT.md, open question 3).
- **Escapes in literals,** so a string can contain `"`.
- **Comparison and ordering** beyond `same?`, as family clauses.
- Grapheme clusters, normalization and custom glyphs (TEXT.md stages 4 and 5).
