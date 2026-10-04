# Exact and lazy numbers

Status: design note, 2026-10-04. Nothing here is built, and there is no rush.
It records Thomas's idea of a number as a lazy structure rather than a fixed
bit encoding, the prior art, and how it would fit March.

## 1. The idea

A number could record how it was made instead of being rounded at once into
64 bits. Then the system can simplify it. `x + x` is `2x`, and a factor that
appears above and below a fraction line cancels. The result is not always
the fastest to compute, but it is exact, or as accurate as asked for.

Thomas, on what it is for: *what it is for is what it is good at.*
- Exact money and precise calculators are viable uses now.
- Each stage is also a building block for a computer algebra system, if we
  or someone else want one later.
- When a problem is known to lead to many reductions, the exact form may
  even be the cheaper one.

## 2. Two benefits, by two mechanisms

1. **Exact identities, by canonical forms.**
   - "x + x is 2x" and "a common factor cancels" are normalization: writing
     each value in one standard form, so that equal values look equal.
   - For fractions this is exact rationals reduced by their greatest common
     divisor, as in Scheme's `1/3` and Common Lisp's ratios.
   - For expressions with unknowns, polynomials and rational functions have
     canonical forms. Beyond them, simplification is a search, the work of a
     computer algebra system.
2. **Rounding chosen at the end.**
   - A number that records its computation can be evaluated at the end to
     whatever precision the answer needs.
   - Each operation passes the request down to its inputs with the extra
     digits it needs ("constructive" or "recursive" reals).
   - This is where the accuracy comes from, more than from simplification.

Floats lose accuracy in particular places, which say where exactness pays:
- `0.1 + 0.2 ≠ 0.3`, because 0.1 has no exact binary form.
- `(a + b) - a ≠ b` when a is much larger than b.
- `x / y * y ≠ x`.
- A sum depends on the order of its terms.

`x + x` is not among them: doubling a float is exact.

## 3. Prior art

- **Hans Boehm, "Small-data computing: correct calculator arithmetic"**
  (CACM, 2017), behind the Android calculator.
  - A number is a rational times a known constant (1, π, √2, e^k, …) where
    possible, with lazy constructive reals as the fallback.
  - The rational part decides most equality questions, and the display asks
    the lazy part for as many digits as it shows.
  - The closest existing design to this idea.
- **CGAL's lazy exact numbers.**
  - They compute fast with interval arithmetic and keep the expression graph.
  - They recompute exactly only when an interval is too wide to decide a
    comparison.
  - Fast in the common case, exact when it matters ("filtering").
- **Exact geometric computation** (LEDA's `real`, the Core library): it
  evaluates to the precision a sign test needs, using root bounds to decide
  zero for algebraic numbers.
- **Gosper's continued-fraction arithmetic** (HAKMEM, 1972): lazy and exact,
  producing terms on demand.
- **E-graphs** (the `egg` library): equality saturation, rewriting with
  sharing, for when simplification rules interact.
- **Richardson's theorem** (1968): whether an expression built with exp,
  sin, absolute value and so on is zero is undecidable in general. This is
  the hard limit below.

## 4. How different the compute is

Rough orders of magnitude, to be measured; per operation on a modern CPU:

| Number | Add or multiply | Notes |
|---|---|---|
| i64, f64 in compiled code | about 1 ns | one instruction |
| i64 `+` in March today | tens of ns | a dozen machine steps with the overflow check |
| Small rational, host primitive | tens to 100 ns | a few multiplies and a gcd |
| Big rational | grows with size | gcd is quadratic in the length, naively |
| Lazy real node | 100 ns to build | evaluation costs per digit asked for |

Two observations:

- **In March the gap is small.** Each March operation already pays
  interpretive overhead, so a rational implemented in the host costs about
  as much as March's own checked `+`. In compiled code the same gap is 10 to
  100 times. Dynamic languages show the same effect: Python's integers are
  big integers, and nobody notices.
- **Reduction can make exact arithmetic cheaper, not just more accurate.**
  Reducing as you go keeps numbers small. Without it they grow:
  - a telescoping sum such as Σ 1/(k(k+1)), which is 1 − 1/(n+1);
  - a binomial coefficient built as a fraction;
  - products with many common factors.

  With content addressing (section 6), a repeated subexpression is computed
  once and then looked up. So for problems known to produce many reductions,
  as Thomas says, the exact form may well be the faster one.

Choosing the type is the opt-in. Code that never uses these types pays
nothing, since families resolve at compile time.

## 5. The hard limits

- **Equality is undecidable in general,** for numbers that leave the
  rationals (Richardson). Rationals compare exactly. Lazy reals can decide
  `x < y` when x and y differ, but may never finish when they are equal.
  An `if` on a real needs a rule:
  - a precision budget, beyond which equal is assumed or an error raised;
  - or a three-way answer: less, greater, undecided.

  Boehm's rational-times-constant form decides most cases met in practice.
- **Growth.** Exact rationals in iterated computations, such as Newton's
  method, can double in length each step. An expression graph grows with
  every operation. Parts with no structure worth keeping must collapse
  eagerly: rationals reduce at once, and only irrational parts stay lazy.
  This is a memory question, and memory management is 2.0.
- **Speed.** Exact numbers are never faster than floats on a single
  operation. They win only where they avoid work, as in section 4, or
  where accuracy is the requirement.

## 6. How it fits March

- **Content addressing gives sharing for free.**
  - An expression graph of hashed nodes stores each repeated subterm once.
  - "The same x" is an identity comparison.
  - Results can be remembered by identity.
  - A reduced rational has a canonical encoding, so equal rationals have
    equal identities, as merkle-champ's canonical shapes do.
  - A real's identity is its expression's, not its value's, since equality
    of values is undecidable.
- **Families give it a place.**
  - Each kind is a numeric type with its own clauses for `+ - * /` and the
    comparisons, resolved at compile time like f64.
  - i64 and f64 stay the fast default.
  - Literals take their type from context, as integer literals become floats
    beside a float. Promotion stays explicit (the agreed rule).
- **Laziness is shared work.** A lazy real is a deferred computation forced
  on demand, as are the lazy array views in stage 1 of `TENSORS.md`. One
  mechanism for deferring and forcing should serve both.
- **Roles.** `price = Money` should never be a float. Money is a decimal
  role over an exact type, with explicit rounding to a scale (cents,
  banker's rounding where required).
- **Values are cells.** A big integer, rational or real is a handle to a
  host-held object, as arrays are handles to vectors, with checker types of
  its own.

## 7. Stages

Each stage is useful by itself and is a building block for the next. None is
scheduled.

1. **Big integers and exact rationals.**
   - Host primitives, always reduced, with checker types and family clauses.
   - A ratio literal (`1/3`), and exact decimal literals in a rational
     context.
   - Rounding to a scale, for money.

   Covers money, fractions and exact counting.
2. **Reals, after Boehm.**
   - A rational times known constants, with lazy constructive reals as the
     fallback and interval filtering to keep the common case fast.
   - Comparisons decide by the rational part, then by precision, within a
     budget.

   Covers the precise calculator.
3. **Symbolic expressions.**
   - Hashed expression graphs with canonical forms for polynomials and
     rational functions: like terms collect, and common factors cancel by
     polynomial gcd.
   - Rewriting beyond that with e-graphs.

   The foundation of a computer algebra system.

## 8. Open questions

1. **Big integers in house, or a dependency** such as `num-bigint`?
   merkle-champ and the vector were built in house. Fast big-integer
   multiplication and gcd are a large, specialized field.
2. **Exact decimal literals.** Once compiled, `0.1` is an f64 and its exact
   value is gone. To read it exactly in a rational context, a float literal
   could keep its decimal digits and exponent beside its bits, or a context
   could be declared before the literal is read.
3. **The rule for `if` on a real:** a precision budget, or three-valued
   comparisons.
4. **One deferring mechanism** for lazy numbers and lazy array views, or
   two.
