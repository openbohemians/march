# Strings

Status: first slice built 2026-10-05, following doc/design/TEXT.md: UTF-8
throughout, no UTF-32 step and no separate ASCII type. Second slice the same
day: escapes, holes and raw literals; `print`; conversions; ordering and
search.

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
that quote to the closing `"`. It may hold spaces, other scripts and
newlines, and backslash escapes:

| Escape | Text |
|---|---|
| `\\`, `\"` | a backslash, a double quote |
| `\n;`, `\t;`, `\r;` | newline, tab, carriage return |
| `\name;` | the symbol `\name` names in code (docs/SURFACE.md): `\times;` is `×` |
| `\#9731;`, `\#x2603;` | a code point, in decimal or in hex: `☃` |
| `\[ code ]` | a hole: the code runs, and its value is written in |
| `\_` | the next input, written in; short for `\[ _ ]`, unless it names a subscript: `\_1;` is `₁` |

```
"hello"
"héllo 中文 😀"
"say \"hi\"\n;"
"2 \times; 3 = \[ 2 3 * ]"
3 "n = \_"
""
```

- **The `;` ends a named or numeric escape,** as `&times;` does in HTML, so
  the name cannot run into the text after it. `\\` and `\"` take none.
- **An unknown name, a missing `;`, a number that is not a code point** (over
  `0x10FFFF`, or a surrogate) **and an empty hole are errors** (trap 27).
- **`\_` is a subscript when a name and `;` follow,** as `\_1` is in code:
  `"H\_2;O"` is `"H₂O"`. That holds when the table has the name (`_0` to
  `_9`, `_n` and the rest); any other `\_` is an input, so `"\_ 1;"` writes
  an input and ` 1;`. An input followed directly by `1;` is `\[ _ ]1;`.

### Holes

A hole's code is a quotation that runs where the hole is, and leaves one
value. That value is written by its type: a string as its text, anything else
as the stack display writes it (`>string`).

- **`_` takes the literal's next input** from the stack. Inputs are taken in
  reading order, so the first `_` or `\_` is the deepest of them:
  `1 2 "\_ and \_"` is `"1 and 2"`, and `4 "\[ _ dup * ]"` is `"16"`.
- **In a definition, a literal with inputs makes the word take them,**
  resolved for their types as any other word's are:
  `: f "<\[ _ 1 + ]>" ;` makes `5 f` `"<6>"` and `1.5 f` `"<2.5>"`.
- **Text resumes right after the `]`.** At the hole's own depth, a `]` ends
  the hole wherever a word would start, so `"\[ n ]apples"` needs no space.
  Words inside a hole are separated by spaces as usual.
- **Literals nest:** `"in \[ "nested \[ 2 3 * ]" ] out"`.
- **`_` belongs to the hole itself,** not to a quotation inside it, which a
  loop may run with its own state on the scratch stack:
  `"\[ ( 1 2 ) [ _ + ] map ]"` is an error (trap 29), as is `_` outside any
  hole. Loop indices work in holes: `( 3 [ "\[ i0 ]" ] times )` in a
  definition is `( "0" "1" "2" )`.

### Raw literals

A token that starts with `'` and is longer than `'` alone is a raw string,
which runs to the next `'` with no escapes: `'C:\temp\new'`,
`'say "hi"'`. It cannot contain `'`, and cannot start with a space, since
`'` followed by a space still quotes a word. These are the same limits as
`%q` strings in Ruby; they may be revisited.

### Compiling

Compiling, a literal's text becomes a data object in the code, and primitive
52 (`text`) makes it a string each time the code runs. A literal with holes
or inputs compiles to code: the inputs move to the scratch stack, then each
part is made a string and joined to the last with `concat`. At top level, a
literal without holes is made at once, and one with holes compiles into a
temporary word that runs at once, as the consumers do. `seed/system.march`
itself keeps FORTH's `s" text"`, because the frozen generation 0 reads it.

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

### Text and numbers

| Word | Effect | Meaning |
|---|---|---|
| `>string` | `( x -- s )` | a value as text: a string as itself, anything else as the stack display writes it |
| `print` | `( x -- )` | write a value's text to standard output |
| `>integer` | `( s -- n )` | parse a decimal integer; trap 28 if the text is not one |
| `>float` | `( s -- f )` | parse a float; trap 28 if the text is not a finite one |

- `>string` and `print` are generic: each use is resolved for its input's
  type, which primitive 68 (`text-of`) takes as a literal operand. So the
  text of a value comes from its type as the checker knows it. A value whose
  type is unknown is written as an integer.
- `print` writes through primitive 69 (`write`) to the machine's output,
  which the command line sends to standard output after each evaluation,
  ending it with a newline if the text did not.
- Parsing (primitives 72 and 73) follows Rust's rules for `i64` and `f64`.

### Ordering and search

| Word | Effect | Meaning |
|---|---|---|
| `compare` | `( a b -- n )` | -1, 0 or 1, by bytes, which for UTF-8 is by code points |
| `lt?` `gt?` `lte?` `gte?` | `( a b -- flag )` | the comparison family, with string clauses |
| `search` | `( s t -- i )` | the character index of the first `t` in `s`, or -1 |

`compare` and `search` are primitives 70 and 71.

## Showing strings

The command line shows a string as a literal that reads back: `<1> "hello"`,
with `\\`, `\"`, `\n;`, `\t;` and `\r;` escaped, and other control
characters as `\#n;`. `print` and `>string` give a string's text as it is.

## Not yet

- **Byte positions.** TEXT.md's `bytes` and `chars` words, and slicing by
  byte position.
- **Characters as a type.** A character is an i64 for now; a type of its own
  needs a literal and display syntax (TEXT.md, open question 3).
- **Format directives in holes,** such as a width or a number of decimals.
- Grapheme clusters, normalization and custom glyphs (TEXT.md stages 4 and 5).
