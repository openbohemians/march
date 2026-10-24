# Combinators

Status: a study, 2026-10-24. Nothing here is decided but one thing: a
superscript is a convention, its meaning the word's family's
(SYMBOLIC-FORTH.md 6.21, decided 2026-10-24), so a combinator's count,
section 2, would be its own.

A bracket may name its inputs, `[ rate term | … ]` (SYMBOLIC-FORTH.md
6.31). Thomas's concern (2026-10-24): "every definition with more than
one argument is going to turn into that form, b/c it easier to think
about. And honetly even allowing `[ r t | ... ] .` is probably going to
lead to the same." Names lower to the code a programmer would write, the
letters of their places and then the stack's own moves, so nothing is lost
as the program runs. What is lost is factoring: stack code cuts anywhere
into words, where a named bracket lifts out only whole, and with it the
pressure that keeps FORTH's words small. This looks at where names would
go in the core, and at what does their work without them: combinators,
which a subscript and a superscript may spell by a rule rather than a
list of names (Thomas).

## 1. Where names could go

Names never enter a quotation (6.31), so a word that maps, folds or hands
code on keeps its values on the stack: about half the core's words of two
or more inputs do (core.march, 2026-10-24), and names cannot reach them.
Of the rest, about half are as short as code gets, `divmod drop`, `swap
drop`, `drop drop false`, and names add nothing. The others, each row's
three giving the same in march8:

| word | today | names | combinators |
|---|---|---|---|
| `ge` | `over over gt? rot rot eq? or` | `[ x y \| x y gt? x y eq? or ] .` | `[ gt? ] [ eq? ] bi_2 or` |
| `le` | `over over lt? rot rot eq? or` | `[ x y \| x y lt? x y eq? or ] .` | `[ lt? ] [ eq? ] bi_2 or` |
| `thru`, up | `over -+ 1 + range swap 1 -+ +` | `[ lo hi \| hi lo -+ 1 + range lo 1 -+ + ] .` | `[ swap -+ 1 + range ] [ drop 1 -+ ] bi_2 +` |
| `thru`, down | `over swap -+ 1 + range - swap 1 + +` | `[ hi lo \| hi lo -+ 1 + range - hi 1 + + ] .` | `[ -+ 1 + range - ] [ drop 1 + ] bi_2 +` |
| `take` | `over length min 1 swap slice` | `[ xs n \| xs 1 n xs length min slice ] .` | `over length min [ 1 ] dip slice` |
| `take`, from the end | `- over length min over length dup rot -+ 1 + swap slice` | `[ xs n \| xs xs length n - over min -+ 1 + xs length slice ] .` | `- over length min over length [ swap -+ 1 + ] keep slice` |
| `skip` | `over length min 1 + over length slice` | `[ xs n \| xs n xs length min 1 + xs length slice ] .` | as today |
| `skip`, from the end | `- over length min over length swap -+ 1 swap slice` | `[ xs n \| xs 1 xs length n - xs length min -+ slice ] .` | as today |

- **A pair used twice,** `ge`, `le`, `thru`, is where FORTH juggles most,
  `over over … rot rot`, and where names read best. `bi_2` does it as
  well, with neither names nor juggling, and the code still factors: `[
  gt? ] [ eq? ] bi_2` lifts out whole or by its quotations.
- **A pipe,** a value passed along, `take` and `skip`, reads as well as it
  can already. Names make it longer.
- **A value worked out and used twice,** `take`'s count and the length,
  gains least: names cover only the inputs, so the named form still has
  an `over`, and the combinator's `keep` takes the place of `dup rot …
  swap`.
- **`thru`'s names read best** of the three.

## 2. The family by width and count

Factor's combinators say three things in their names: how many
quotations or groups, `bi` two and `tri` three; how they meet the values,
each quotation on the same values (cleave, `bi`), each on its own group
(spread, `bi*`) or one quotation on each group (apply, `bi@`); and how
wide a group is, `2bi`, `3tri`. March's subscript is a width already
(SYMBOLIC-FORTH.md 6.22): `dip_2`, `keep_2` and `bi_2` are Factor's
`2dip`, `2keep` and `2bi`, built, and a name that begins with a digit,
`2bi`, March reads as a number, so a subscript is how March must spell it
anyway. What is left is the count, and a superscript is the place left
for it:

| Factor | effect | march8 | by width and count |
|---|---|---|---|
| `dip` | x y q → q(x) y | `dip` | |
| `2dip` | x y z q → q(x) y z | `dip_2` | |
| `keep` | x q → q(x) x | `keep` | |
| `2keep` | x y q → q(x y) x y | `keep_2` | |
| `bi` | x p q → p(x) q(x) | `bi` | `cleave^2` |
| `2bi` | x y p q → p(x y) q(x y) | `bi_2` | `cleave_2^2` |
| `tri` | x p q r → p(x) q(x) r(x) | none | `cleave^3` |
| `2tri` | x y p q r → p(x y) q(x y) r(x y) | none | `cleave_2^3` |
| `bi*` | x y p q → p(x) q(y) | none | `spread^2` |
| `tri*` | x y z p q r → p(x) q(y) r(z) | none | `spread^3` |
| `2bi*` | w x y z p q → p(w x) q(y z) | none | `spread_2^2` |
| `bi@` | x y q → q(x) q(y) | none | `apply^2` |
| `tri@` | x y z q → q(x) q(y) q(z) | none | `apply^3` |
| `2bi@` | w x y z q → q(w x) q(y z) | none | `apply_2^2` |

Three words, a width and a count, where Factor has a name for each, and
both marks read as mathematics reads them. The names `cleave`, `spread`
and `apply` are Factor's, to be chosen: `.` already applies. `keep` and
`dip` stay words, being the commonest, though `keep` is a cleave whose
last quotation is `[ ]`, `[ p ] [ ] bi`, and `dip` a spread's, `[ p ] [ ]
spread^2`; `bi@` is a spread of copies, `dup spread^2`. Each is a line of
March over `dip` and `keep`, `spread^2` being `[ dip ] dip .` and `tri`
`[ [ keep ] dip keep ] dip .` (checked in march8, 2026-10-24).

What the superscript means is each word's own (decided 2026-10-24). A
superscript is a convention, as a subscript is (6.21, 22), so `cleave`,
`spread` and `apply` give theirs by a clause of the core's `power#`, for
the word and any count, the count of quotations or of groups, as `log`
gives its subscript the base. The core's default for a word handed code,
applied n times, is no use here, and march8 refuses it for `dip^2` and
`bi^2`, their effects not known. Weighed first and set aside the same day:
`cleave` as the operation that makes one quotation of two, `[ p ] [ q ]
cleave` being `[ [ p ] keep q ]`, so that 6.21's default for an
operation, n operands joined, would give `cleave^3` as `cleave cleave`.
Nested so, they are `tri` and `tri*`:

```
5 [ [ 1 + ] keep [ [ 2 ⋅ ] keep 3 -+ ] . ] .        -- 6 10 2, as tri
3 4 5 [ [ [ 1 + ] dip 2 ⋅ ] dip 3 -+ ] .            -- 4 8 2, as tri*
```

But it needed a `.` where the combinator is applied, `x [ p ] [ q ] [ r ]
cleave^3 .`, the joined quotation being a value, only to keep one rule
for every power, which a convention does not need.
