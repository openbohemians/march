# Numbers: integers, floats and their literals

Status: floats added 2026-10-03 (Thomas: "we will need them after all").

## Machine operations

Cells are 64 bits; an operation decides how to read them. Integer machine
arithmetic is unsigned and wrapping (`u+ u- u* u/ umod ult?`), and checked
signed arithmetic is March code over it (`+ - * / mod lt? …`).

Floats are IEEE-754 binary64 on a cell's bits, as eight primitives:

| Id | Word | Effect | Meaning |
|---|---|---|---|
| 36 | `f+` | `( a b -- c )` | Sum |
| 37 | `f-` | `( a b -- c )` | Difference |
| 38 | `f*` | `( a b -- c )` | Product |
| 39 | `f/` | `( a b -- c )` | Quotient: division by zero gives an infinity, 0/0 gives NaN |
| 40 | `feq?` | `( a b -- flag )` | IEEE equality: NaN is unequal to itself |
| 41 | `flt?` | `( a b -- flag )` | IEEE less-than |
| 42 | `i>f` | `( n -- f )` | Signed integer to float, round to nearest even |
| 43 | `f>i` | `( f -- n )` | Truncate toward zero; NaN, infinities and values outside i64 trap (Arithmetic) |

**Determinism.** Basic IEEE operations are correctly rounded, so they give the
same bits on every machine. The one variable part is a NaN's payload, so every
NaN result is replaced by a single canonical pattern (`0x7ff8000000000000`).
Equal computations therefore give equal bytes, which content addressing needs.

## Float literals in code

A float literal compiles to its own opcode, 10, with eight little-endian bytes
(docs/FIRST-SLICE.md). It runs exactly like the integer literal opcode 1. The
separate opcode keeps the literal's type in the canonical code that the
checker reads. Families will need that type to choose a clause for `+`.

## Literal syntax

`number` tries the integer parser, then the float parser. A quick pre-check
rejects most words at once: a number must start with a digit, or with a sign
followed by a digit.

- **Integers:** an optional sign and decimal digits, as before. An integer
  that does not fit i64 traps (2); it does not fall back to a word.
- **Floats:** an optional sign, digits, then a dot with digits, or an exponent
  (`e`/`E`, an optional sign, digits), or both. A float needs a dot or an
  exponent, so `1.`, `.5` and `1.2.3` are words, not numbers.

**Correct rounding.** Literals are parsed in March, with no big-number
arithmetic, using Clinger's fast path:

- The digits form an integer mantissa of at most 2^53, and the decimal
  exponent is at most 22 in size.
- The value is the mantissa times or divided by an exact power of ten: a
  single, correctly rounded operation.
- Zeros are deferred until a nonzero digit follows, so trailing zeros never use
  up the mantissa.
- Excess powers of ten move into the mantissa while it stays at most 2^53, the
  extended fast path. That covers literals such as `1e30`.

Anything outside the fast path traps (21) rather than round wrongly. Examples
are `1e40`, `1.5e-30`, and more than about 16 significant digits. A slow path
with big-number arithmetic can lift the limit later.

A test parses 400 random fast-path literals in varied spellings and requires
the bits to match Rust's correctly rounded parser exactly.

## Mixed types

There is no implicit promotion between values, decided 2026-10-01. Conversions
are explicit (`i>f`, `f>i`), and literals will take their type from context
once families resolve by type. The planned `!` operator converts a value to the
type of the value below it.
