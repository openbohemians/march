# Built-in words and symbols

Status: draft for review, 2026-10-08. A table of March's built-in words, as
march8 defines them, with a proposed symbol for each where one fits, beside
APL's and Uiua's for the same thing. Nothing here is decided; section 3
lists the questions.

## 1. The rules so far

From march7/docs/SURFACE.md ("Symbols") and the conversation of 2026-10-08:

- **Every built-in word has a word name** (proposed). Its symbol is typed by
  that name, `\each`, and `march8 fmt` could turn either form into the other.
  A symbol's table name is the name of the word it stands for.
- **APL's glyphs join the table only as words are defined for them.**
- **Greek letters are letters, never core symbols,** nor APL's glyphs drawn
  from them (`⍳`, `⍴`, `⍺`, `⍵`, `⍷`, `⍸`).
- **Already taken in March:** `.` applies; `<` and `>` are brackets; `=` is
  `dup` and `~` is `swap` (adopted on trial, 2026-10-03); `!` converts to the
  type below; `#` is headings, perhaps count later; `\` starts a symbol name;
  `_` pulls a value into a literal.

APL is Dyalog APL. Uiua is its current main branch
(github.com/uiua-lang/uiua, `parser/src/defs.rs`, read 2026-10-08), where
every glyph also has a lowercase name and the formatter turns names into
glyphs, the scheme proposed for March.

## 2. The words

### Arithmetic

| March | Means     | Symbol | APL | Uiua         | Notes |
|-------|-----------|--------|-----|--------------|-------|
| `+`   | add       | `+`    | `+` | `+` add      |       |
| `-`   | subtract  | `-`    | `-` | `-` subtract | negating is APL's `-x`, Uiua's `¯` |
| `*`   | multiply  | `×` `\times` | `×` | `×` multiply | `\times` is `×` already, so the loop `times` needs another name (Q7) |
| `/`   | divide    | `÷` `\div` | `÷` | `÷` divide | frees `/` for reduce (Q8) |
| `mod` | remainder | `◿` | `\|` residue, arguments the other way | `◿` modulo | |

### Comparison

| March | Means | Symbol | APL | Uiua | Notes |
|---|---|---|---|---|---|
| `lt?` | less than | ? | `<` | `<` less than | `<` `>` are March's brackets (Q2) |
| `gt?` | greater than | ? | `>` | `>` greater than | |
| `eq?` | equal, whole values | `≡` `\equiv` | `≡` match; `=` is per element | `≍` match; `=` per element | `=` is `dup` in March (Q3) |

### Stack

| March | Means | Symbol | APL | Uiua | Notes |
|---|---|---|---|---|---|
| `dup` | copy the top | `=` | none | `.` duplicate | Uiua's `.` is March's apply |
| `drop` | discard the top | `◌` | none | `◌` pop | |
| `swap` | exchange the top two | `~` | `⍨` commute, an operator | `:` flip | |
| `over` | copy the second | none | none | none: removed, `⊸` by | |
| `rot` | third to the top | none | none | none | |

### Arrays and maps

| March | Means | Symbol | APL | Uiua | Notes |
|---|---|---|---|---|---|
| `length`, `count` | how many elements | `⧻` | `≢` tally | `⧻` length | `#` is J's tally, deferred |
| `at` | element at an index, value at a key | `⊡` | `⌷` index, `⊃` pick | `⊡` pick | |
| `concat` | join | `⊂` | `,` catenate | `⊂` join | `\subset` is `⊂` too |
| `slice` | elements from i to j | none | `↑` `↓` take, drop | `↙` `↘` take, drop | |
| `first` | | `⊢` | `⊃` first | `⊢` first | `\vdash` is `⊢` |
| `last` | | `⊣` | `⊃⌽` | `⊣` last | |
| `rest` | all but the first | none | `1↓` | `↘1` | |
| `most` | all but the last | none | `¯1↓` | `↘¯1` | |
| `reverse` | | `⇌` | `⌽` | `⇌` reverse | |
| `range` | 0 to n−1 | `⇡` | `⍳`, from Greek | `⇡` range | Uiua's replaces APL's Greek one |
| `sort` | ascending | `⍆` | `{⍵[⍋⍵]}`, by grade `⍋` | `⍆` sort | |
| `keys`, `values` | a map's | none | none | none | |

### Loops and consumers

| March | Means | Symbol | APL | Uiua | Notes |
|---|---|---|---|---|---|
| `map` | each element's result, collected | `∵` | `¨` each | `∵` each, `≡` rows | APL's and Uiua's "each" is March's `map` (Q4) |
| `each` | each element in turn, the stack threaded | none | none | none | Factor's `each`; in an array literal, a comprehension |
| `each-right` | the same, from the last | none | none | none | |
| `fold` | from the left, from a start | none | none | `∧` fold | `∧` is "and" in APL and in the table (Q5) |
| `fold-right` | from the right, from a start | none | `/`, which reduces from the right | none | |
| `reduce` | from the left, from the first | `/` ? | `/`, from the right | `/` reduce | deferred (Q8) |
| `reduce-right` | from the right, from the last | none | `/` | none | |
| `scan` | the running reductions | ? | `\` | `\` scan | `\` starts symbol names in March (Q8) |
| `times` | a quotation n times | `⍥` | `⍣` power | `⍥` repeat | as `repeat` (Q7) |
| `compose` | two quotations as one | `∘` `\circ` | `∘` jot, compose | `∘` is identity | (Q6) |

### Text

| March | Means | Symbol | APL | Uiua | Notes |
|---|---|---|---|---|---|
| `show` | a value as the display writes it | `⍕` | `⍕` format | none | |
| `>string` | a string's text, anything else shown | none | `⍕` | none | |
| `print` | write any value | none | `⎕←` | `&p` | Uiua's system words start with `&` |
| `write` | write a string | none | `⍞←` | `&pf` | |

### Definitions and types

No symbols proposed: `def`, `.`, `_`, the types `i64`, `f64`, `money`,
`string`, `int#`, `dec#`, `type`, `symbol`, `quote`, and the constructors
`ary`, `vec` and `map`. The machine's primitives (`i64+`, `string-at`, …) are
not vocabulary and get none.

### Candidates: words March lacks

Common in APL or Uiua, with glyphs that are not Greek:

| Word | APL | Uiua | Notes |
|---|---|---|---|
| negate | `-` | `¯` | |
| absolute value | `\|` | `⌵` | |
| min, max | `⌊` `⌈` | `↧` `↥` | |
| floor, ceiling | `⌊` `⌈` | `⌊` `⌈` | |
| not, and, or | `~` `∧` `∨` | `¬`, none, `∨` | `\neg` `\wedge` `\vee` are in the table |
| square root | `*.5` | `√` | `\sqrt` is in the table |
| power | `*` | `ⁿ` | |
| at most, at least, not equal | `≤` `≥` `≠` | `≤` `≥` `≠` | in the table; words `lte?` `gte?` `neq?` |
| keep: filter by a mask | `/` replicate | `▽` keep | |
| take, drop | `↑` `↓` | `↙` `↘` | `\uparrow` `\downarrow` are `↑` `↓` |
| member of | `∊` | `∊` memberof | `\in` is `∈` |
| index of | `⍳`, from Greek | `⊗` indexin | |
| unique | `∪` | `◴` deduplicate | `\cup` is `∪` |
| where | `⍸`, from Greek | `⊚` where | |
| shape, reshape | `⍴`, from Greek | `△` `↯` | |
| transpose | `⍉` | `⍉` | |
| rotate | `⌽` | `↻` | |
| table, outer product | `∘.` | `⊞` | |
| grade up, down | `⍋` `⍒` | `⍏` `⍖` rise, fall | |
| dip, under | none | `⊙` `⍜` | |

## 3. Questions

1. **Names with punctuation.** The `\name` escape reads letters only, so
   `lt?`, `eq?`, `fold-right`, `each-right` and `>string` cannot be table names
   as they are. Either a table name drops the punctuation (`\eq` for `eq?`), or
   the escape reads a word's whole name, letters, digits, `-` and `?`, as far
   as a character that cannot be in one.
2. **Comparisons.** `<` and `>` are brackets, so `lt?` and `gt?` cannot take
   APL's and Uiua's symbols. Keep them as words, or choose others: `≺` `≻`
   (`\prec`, `\succ`)? `≤` `≥` `≠` are free.
3. **`=` and `~`.** On trial since 2026-10-03 as `dup` and `swap`. APL and Uiua
   read `=` as equality, and Uiua's `dup` is `.`, which is March's apply. Keep
   the trial, or give `=` back to comparison and `dup` a glyph?
4. **`map` and `each`.** APL's `¨` and Uiua's `∵`, "each", collect results:
   that is March's `map`. March's `each` threads the stack, as Factor's does.
   So `∵` would be `map`'s symbol, and `each` would have none, or the names
   might move.
5. **`∧`:** Uiua's fold, APL's and logic's "and". "And" is the stronger claim.
6. **`∘`:** APL's composition, Uiua's identity. Composition fits `compose`.
7. **`times`:** `\times` is `×`, multiplication, so the loop becomes `repeat`,
   `⍥`, as in Uiua; or `×` goes by another name.
8. **`/`, `\` and `÷`:** deferred. If `/` is reduce and `÷` divides, as in APL
   and Uiua, then scan, APL's and Uiua's `\`, needs another glyph, since `\`
   begins a symbol name; APL's `⍀` is one.
9. **Count:** `count` for now, `⧻` as its symbol, or `#` once headings start
   lines.
