# Symbolic Forth

Status: design note, 2026-10-13, from a conversation with Thomas. A first
account of March's lowest form; a precise specification is to come.
March8 reads and writes it: `march8 lower` writes any March in Symbolic
Forth, `--symbolic` reads it, and march8/tests/symbolic.rs is its litmus
test (section 5).

## 1. What it is

Everything in Symbolic Forth is a number, a symbol, `.` or `!`:

- **A number** is a literal, unstructured until something structures it:
  `2` is a number before it is an i64.
- **A symbol** is any name. A name means nothing until it is applied:
  `dup` is the symbol `dup`, and so is `Snake`.
- **`.` applies** the value on top: a symbol, the word its name is defined
  as; a quotation, evaluated.
- **`!` converts** the value below by the type on top: `2 i64 !`.

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

A type is data, and is not called. Converting is applying another word,
`2 i64 . convert .`, which `!` shortens: `2 i64 !` (`convert` takes a type,
or a type's symbol). A type converts a literal by the clauses under its
name, a string by the number it writes, `"42" i64 !` being 42 or nil; a
class checks, `x num !`; a signature gives several values their types, `x
y < i64 . f64 . > !`. Which mark calls and which converts is only a
choice: `!` would serve as well for calling, a command (Thomas).

## 3. Structure

- **`[ … ]`** is a quotation: its words, unevaluated.
- **`( … )` and `{ … }`** are an array and a map literal, collecting what
  their code leaves.
- **`< … >`** is a bracket: code run while compiling, whose values are a
  type or a signature, `< string . i64 . map . k >`. `--` divides inputs
  from outputs. First in a definition's quotation it is the clause's
  context.
- **`"…"`** is a string, a literal, with holes of code: `"\[ _ . ] apples"`.

Inside each, the code is Symbolic Forth too. Whether these belong to the
lowest form or are structure built from numbers and symbols is open
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
| `2 i64!` | `2 i64 . !` | `!` converts |
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

## 6. Open, toward a specification

1. **Strings.** A literal, as a number is, or structure, text built from
   numbers (`utf8 str`)?
2. **Brackets.** Part of the lowest form, or words over it? FORTH builds
   its own from words, `:` and `;`, `[` and `]`.
3. **`!`.** A meta-operator in its own right, or only `convert .`?
   Lowering writes `2 i64 . !` where `2 i64 !` would do, since `convert`
   takes a type's symbol.
4. **Comments and `--`.** A comment runs from `--` to the line's end,
   except in a bracket, where `--` divides inputs from outputs, which
   leaves no comment inside a bracket.
5. **Symbols at run time.** Programs of symbols alone, a state machine or
   facts and rules, run while compiling today: a symbol cannot be left for
   run time (DOTS.md, finding 3), nor can a family that applies itself
   take symbols (TYPES.md 3.16).
6. **Namespaces.** A dot inside a word divides a path, `io.file.read`
   (TYPES.md 3.12): in Symbolic Forth a symbol with dots, applied whole.
