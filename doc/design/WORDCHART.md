| Meaning              | March Word | March  | APL   | Uiua  | NOTES  |
|----------------------|------------|--------|-------|-------|--------|
| add                  | `add`      |  `+`   | `+`   | `+`   |        |
| negate               | `neg`      |  `-`   | `-`   | `¯`   | APL's monadic `-` |
| subtract             | `sub`      |  `-+`  | `-`   | `-`   | negate, then add: `a b -+` is a − b |
| multiply             | `mul`      |  `*`   | `×`   | `×`   | glyph `⋅` |
| reciprocal           | `recip`    |  `/`   | `÷`   |  —    | APL's monadic `÷`; glyph `÷`; no integer has one |
| divide               |  —         |  `/*`  | `÷`   | `÷`   | reciprocal, then multiply: `a b /*` is a ÷ b |
| quotient, floored    | `div`      |        | `⌊÷`  |       | integers; of floats, their quotient |
| remainder            | `mod`      |  `◿`   | `\|`  | `◿`   |        |
| div remainder        | `divmod`   |   ?    |       |       |        |
| less than            | `lt?`      |   ?    | `<`   | `<`   |        |
| greater than         | `gt?`      |   ?    | `>`   | `>`   |        |
| equal                | `eq?`      |  `≡`   | `≡`   | `≍`   |        |
| at most              | `le?`      |  `≤`   | `≤`   | `≤`   | less, or equal: NaN is at most nothing |
| at least             | `ge?`      |  `≥`   | `≥`   | `≥`   |        |
| not equal            | `neq?`     |  `≠`   | `≠`   | `≠`   |        |
| copy top             | `dup`      |  `=`   |       | `.`   |        |
| discard top          | `drop`     |  `◌`   |       | `◌`   |        |
| exchange top         | `swap`     |  `~`   | `⍨`   | `:`   |        |
| copy second          | `over`     |        |       |       |        |
| third to top         | `rot`      |        |       |       |        |
| length               | `len`      |  `⧻`   | `≢`   | `⧻`   | or `count` ? |
| count                | `count`    |   ?    | `≢`   | `⧻`   |        |
| element at           | `at`       |  `⊡`   | `⌷`   | `⊡`   | at an array of positions, an array |
| join                 | `concat`   |  `⊂`   | `,`   | `⊂`   |        |
| elements i->j        | `slice`    |        | `↑`   | `↙`   |        |
| first n, last n      | `take`     |  `↑`   | `↑`   | `↙`   | negative n from the end; from SYMBOL |
| all but first n      | `skip`     |  `↓`   | `↓`   | `↘`   | negative n from the end; `drop` is the stack's |
| integers a to b      | `thru`     |  `‥`   |       |       | both included, down if a is larger; SYMBOL's `..` |
| elements j->i        |  —         |        | `↓`   | `↘`   |        |
| first                | `first`    |  `⊢`   | `⊃`   | `⊢`   |        |
| last                 | `last`     |  `⊣`   | `⊃⌽`  | `⊣`   |        |
| all but first        | `rest`     |  `⫣`   | `1↓`  | `↘1`  |        |
| all but last         | `most`     |  `⊩`   | `¯1↓` | `↘¯1` |        |
| reverse              | `reverse`  |  `⇌`   | `⌽`   | `⇌`   | `flip` instead? |
| 1 to n               | `range`    |  `⇡`   | `⍳`   | `⇡`   |        |
| sort                 | `sort`     |  `⍆`   | `⍋`   | `⍆`   |        |
| order (grade up)     | `order`    |  `<#`  | `⍋`   | `⍏`   | the positions that would sort; R's `order`; not "grade", near "gradient" |
| order, descending    | `dorder`   |  `#>`  | `⍒`   | `⍖`   | stable, as `order reverse` is not; the point is where the smallest go |
| sort by a key        | `sort-by`  |        | `X[⍋K]` |     | `over swap map order at`: keys, ordered, read in order |
| keep by mask         | `keep`     |  `▽`   | `/`   | `▽`   |        |
| word on array's end  | `within`   |        |       | `⍜`   | Uiua's under is nearest equivalent |
| elements onto stack  | `spread`   |        |       |       |        |
| insert in gap k      | `insert`   |        |       |       | 0 prepends, -1 appends |
| remove element k     | `remove`   |        |       |       |        |
| map keys             | `keys`     |        |       |       |        |
| map values           | `values`   |        |       |       |        |
| map value or nil     | `get`      |        |       |       | `v nil or` |
| map with key set     | `put`      |        |       |       |        |
| empty array or map   | `empty`    |        |       |       | of a type: `string i64 map empty`; name provisional |
| each, collect        | `map`      |  `∵`   | `¨`   | `∵`   |        |
| each pair, in step   | `zip`      |        |       |       |        |
| outer product        | `table`    |  `×`   | `∘.`  | `⊞`   | `×` is the table of products |
| dot product          | `dot`      |  ?     | `+.×` |       | contraction: matrix · matrix is the matrix product; `⊙`? (Hadamard in math) |
| transpose            | `transpose` | `⍉`   | `⍉`   | `⍉`   |        |
| each, thread stack   | `each`     |        |       |       |        |
| fold from left       | `fold`     |        |       | `∧`   |        |
| reduce from left     | `reduce`   |        |       | `/`   | `/` is the reciprocal |
| running reductions   | `scan`     |  ?     | `\`   | `\`   |        |
| repeat n times       | `repeat`   | `⍥`    | `⍣`   | `⍥`   |        |
| compose quotations   | `compose`  | `∘`    | `∘`   |  —    |        |
| value as text        | `show`     | `⍕`    | `⍕`   |  —    |        |
| number from text     | `parse`    |        | `⍎`   | `⋕`   | `"42" i64 parse`: `i64 nil or` |
| split at a separator | `split`    |        |       | `⊜`   | "" splits between characters |
| lines, words         | `lines` `words` |   |       |       | as Haskell's |
| lower, upper case    | `lower` `upper` |   |       |       | element by element over arrays |
| write any value      | `print`    |        | `⎕←`  | `&p`  |        |
| write a string       | `write`    |        | `⍞←`  | `&pf` |        |
| swap top two         | `swap2`    |  `~~`  |       |       |        |
| dup top two          | `dup2`     |  `==`  |       |       |        |
| absolute value       | `abs`      |  `⌵`   | `\|`  | `⌵`   |        |
| minimum              | `min`      |  `↧`   | `⌊`   | `↧`   |        |
| maximum              | `max`      |  `↥`   | `⌈`   | `↥`   |        |
| floor                | `floor`    |  `⌊`   | `⌊`   | `⌊`   |        |
| ceiling              | `ceil`     |  `⌈`   | `⌈`   | `⌈`   |        |
| power                | `pow`      |  `^`   | `*`   | `ⁿ`   | as LaTeX writes it |
| square root          | `sqrt`     |  `√`   | `*.5` | `√`   |        |
| sum                  | `sum`      |  `∑`   | `+/`  | `/+`  | ∑ U+2211, not Greek Σ; 0 when empty |
| product              | `prod`     |  `∏`   | `×/`  | `/×`  | ∏ U+220F, not Greek Π; 1 when empty |
| truth                | `true` `false` | `⊤` `⊥` | `1` `0` | `1` `0` | `bool`: 0 and 1, an i64 too |
| not                  | `not`      |  `¬`   | `~`   | `¬`   | 1 − x  |
| and                  | `and`      |  `∧`   | `∧`   |  —    | minimum |
| or                   | `or`       |  `∨`   | `∨`   | `∨`   | maximum; on two types, their union |

| **not yet in March** |           |        |       |       |
|----------------------|-----------|--------|-------|-------|
| convert to match     |  ?        |  ?     |       |       | `!` retired; `to`, `type-of` open |
| member of            |           | `∈`    | `∊`   | `∊`   |
| index of             | `index`   | `⊗`    | `⍳`   | `⊗`   |
| unique               | `uniq`    | `◴`    | `∪`   | `◴`   |
| where                |           | `⊚`    | `⍸`   | `⊚`   |
| shape                | `shape`   | `△`    | `⍴`   | `△`   |
| reshape              |           | `↯`    | `⍴`   | `↯`   |
| rotate               |           | `↻`    | `⌽`   | `↻`   |
| dip                  |           | `⊙`    |  —    | `⊙`   |
| under                |           | `⍜`    |  —    | `⍜`   |

| **undecided**        |              |        |       |       | NOTES  |
|----------------------|--------------|--------|-------|-------|--------|
| range type           |              |        |       |       | lazy; a subrange type, as Pascal's `0..100` (TYPES.md 3.11) |
| list edits by arrows |              | `+>` `<+` `->` `<-` |  |  | SYMBOL's: to think on (TODO in MACHINE.md) |
| fold from right      | `fold-rt`    |        | `/`   |       |        |
| reduce from right    | `reduce-rt`  |        | `/`   |       |        |

| **not needed**       |              |        |       |       | NOTES  |
|----------------------|--------------|--------|-------|-------|--------|
| text of a value      | `>string`    |        | `⍕`   |       |        |
| same, from last      | `each-right` |        |       |       |        |

