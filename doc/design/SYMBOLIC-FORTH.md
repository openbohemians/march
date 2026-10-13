# Symbolic Forth

Status: design note, 2026-10-13, from a conversation with Thomas. A first
account of March's lowest form; a precise specification is to come.
March8 reads and writes it: `march8 lower` writes any March in Symbolic
Forth, `--symbolic` reads it, and march8/tests/symbolic.rs is its litmus
test (section 5).

## 1. What it is

Everything in Symbolic Forth is a symbol, an ordered set of symbols, or
`.` (Thomas, 2026-10-13):

- **A symbol** is a name, `dup`, `Snake`; a number, `2`, a literal no type
  has structured yet; or, quoted, a symbol with spaces in it, `'this is
  one symbol'`, which is what text is before a type structures it. A
  symbol means nothing until it is applied.
- **An ordered set of symbols**, `( … )`, an "array" only in quotes:
  nothing here is a data structure, and structure comes only with types.
  A map is no fundamental of its own, but an ordered set of pairs read as
  keys and values. A quotation, `[ … ]`, holds code the same way.
- **`.` applies** the value on top: a symbol, the word its name is defined
  as; a quotation, evaluated.

And one convenience, so close to the fundamentals that the distinction is
not worth fretting over: **`!`**, which is `convert .`, a primitive word
applied. It structures the value below by the type on top, a number
symbol into a concrete i64, say: `2 i64 convert .`, written `2 i64 !`. No
meta-operator is needed for it; `!` is kept because it is convenient
(Thomas, 2026-10-13: the word's name is anyone's choice).

```
1 i64 ! 1 i64 ! + .
sq [ dup . * . ] def .
fib [ < n > dup . 1 -+ . fib . swap . 2 -+ . fib . + . ] def .
Closed Open door . Close door . Lock door . Locked eq? .
```

Nothing applies unless a dot says so: `3 dup .` is 3 and 3, and `3 dup`
leaves the symbol `dup`. A definition is data, a symbol, what it names,
and `def` applied: `sq [ dup . * . ] def .`.

Its nearest kin are Joy, where a program is a list and `i` applies one,
and FORTH's own words; the difference is that every name is a symbol, and
`.` is the only application.

## 2. Types

The only base things are literals and symbols (Thomas, 2026-10-12).
Everything else is structure, and words build it, types included: `i64 .`
is the type i64, the word `i64` applied, and `string . i64 . map .` a map
from strings to i64s. A type that bottoms out might show as `i64'`, were
it ever shown, but it never is, since every type is given by a word: even
`i64` might one day be `64 int .`, as `string` might be `utf8 str .`.

So types need no spelling of their own, and types and words are one
namespace.

A type is data, and is not called. Structuring a value by it is applying
another word, `convert`: `2 i64 . convert .`, or, since `convert` takes a
type's symbol too, `2 i64 convert .`, which `!` shortens to `2 i64 !`. A
type converts a literal by the clauses under its
name, a string by the number it writes, `"42" i64 !` being 42 or nil; a
class checks, `x num !`; a signature gives several values their types, `x
y < i64 . f64 . > !`. Which mark calls and which converts is only a
choice: `!` would serve as well for calling, a command (Thomas).

## 3. Structure

What march8 has today, beyond symbols and `.`:

- **`[ … ]`** is a quotation: its words, unevaluated.
- **`( … )` and `{ … }`** are an array and a map literal, collecting what
  their code leaves. In the lowest form the first is the ordered set, and
  the second a set of pairs (section 1).
- **`< … >`** is a bracket: code run while compiling, whose values are a
  type or a signature, `< string . i64 . map . k >`. `--` divides inputs
  from outputs. First in a definition's quotation it is the clause's
  context.
- **`"…"`** is a string, with holes of code: `"\[ _ . ] apples"`. In the
  lowest form text is a quoted symbol, `'…'`, which a type structures; the
  holes are code that builds one.

Inside each, the code is Symbolic Forth too. Which of these are
fundamental and which are words over the fundamentals is still open
(section 6).

## 4. The intermediate form

March8 reads an intermediate form over Symbolic Forth (TYPES.md 2.2). It
marks the rarer things: code applies words far more often than it passes
symbols, so a name applies by itself and a symbol is spelled as one
(DOTS.md). It lowers to Symbolic Forth by spelling alone:

| Intermediate | Symbolic Forth | |
|---|---|---|
| `dup` | `dup .` | a name applies |
| `Snake` | `Snake` | a capital marks a symbol |
| `+'` | `+` | a prime marks a word passed |
| `fib:` | `fib` | a label is a symbol |
| `a` | `a` | one lowercase letter is a variable |
| `[ 2 + ].` | `[ 2 + . ] .` | a dot at a word's end applies once more |
| `2 i64!` | `2 i64 . !` | `!` structures by a type: `convert .` |
| `"\_ apples"` | `"\[ _ . ] apples"` | a hole's input |

```
fib: [ < n > dup 1 -+ fib swap 2 -+ fib + ] def
fib [ < n > dup . 1 -+ . fib . swap . 2 -+ . fib . + . ] def .
```

A surface may come above the intermediate form, as SURFACE.md's headings
were to be. How many stages there are is still to be discovered; Symbolic
Forth is the one at the bottom.

## 5. The litmus test

What cannot be said in Symbolic Forth is not March. Every notation above
it must lower to it, mechanically, and mean the same.

March8 runs the test. `march8 lower` writes March as Symbolic Forth,
keeping its lines, and `--symbolic` reads it. march8/tests/symbolic.rs
lowers the core vocabulary and the examples, reads them back to the same
tokens, and runs the examples lowered to the same results. Code as data
is this form: what a program builds as code, it builds in Symbolic Forth.

## 6. Toward a specification

Settled (Thomas, 2026-10-13):

1. **Text** is a symbol that may hold white space, written quoted, `'this
   is symbols with spaces in it'`, and a type structures it into a
   string. An ordered set of code points would do too, but that is the
   long way round. March8 does not have it yet: there `'…'` is a raw
   string, and lowering writes strings as strings.
2. **Maps** are ordered sets of pairs, not a fundamental.
3. **`!`** is `convert .`, a convenience over a primitive word, which
   structures a value by a type; it needs no meta-operator of its own.
   Built: `2 i64 convert .` is `2 i64 !`.

Built since:

4. **Symbols at run time** (2026-10-13). A symbol is a value while a
   program runs too, its name, which march8 keeps as an interned string,
   so symbols compare and key maps by identity, and a family applies itself
   to them: `3 dup` leaves 3 and the symbol `dup`, and `"Open" sym!` makes
   a symbol from data. Applying one is choosing a word, which is done while
   compiling, so `.` applies a known symbol only.

Open:

5. **Comments and `--`.** A comment runs from `--` to the line's end,
   except in a bracket, where `--` divides inputs from outputs, which
   leaves no comment inside a bracket (Thomas is thinking on it).
6. **Namespaces.** A namespace is a map, so an ordered set of pairs, and
   may need no notation of its own: only a way to confine `.` to a
   namespace, which may well be a mode of the system (Thomas). Meanwhile a
   dot inside a word divides a path, `io.file.read` (TYPES.md 3.12).
7. **Quotations and ordered sets.** Joy has one thing, a list, which `i`
   applies; here `[ … ]` and `( … )` may be one ordered set, applied by
   `.`, and `< … >` built from words, as FORTH builds `:` and `;`.
8. **Symbols applied at run time.** A symbol known only as the program
   runs names a word that is not chosen yet. Applying one would take the
   words themselves at run time, a dictionary as FORTH has, or a choice
   among the words a symbol's type allows.
