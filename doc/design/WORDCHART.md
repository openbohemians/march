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
| elements j->i        |  —         |        | `↓`   | `↘`   |        |
| first                | `first`    |  `⊢`   | `⊃`   | `⊢`   |        |
| last                 | `last`     |  `⊣`   | `⊃⌽`  | `⊣`   |        |
| all but first        | `rest`     |  `⫣`   | `1↓`  | `↘1`  |        |
| all but last         | `most`     |  `⊩`   | `¯1↓` | `↘¯1` |        |
| reverse              | `reverse`  |  `⇌`   | `⌽`   | `⇌`   | `flip` instead? |
| 1 to n               | `range`    |  `⇡`   | `⍳`   | `⇡`   |        |
| sort                 | `sort`     |  `⍆`   | `⍋`   | `⍆`   |        |
| grade up             | `grade`    |  `⍏`   | `⍋`   | `⍏`   | the positions that would sort |
| sort by a key        | `sort-by`  |        | `X[⍋K]` |     | `over swap map grade at`: keys, graded, read in order |
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
| truth                | `true` `false` |    | `1` `0` | `1` `0` | `bool`: 0 and 1, an i64 too |
| not                  | `not`      |  `¬`   | `~`   | `¬`   | 1 − x  |
| and                  | `and`      |  `∧`   | `∧`   |  —    | minimum |
| or                   | `or`       |  `∨`   | `∨`   | `∨`   | maximum; on two types, their union |

| **not yet in March** |           |        |       |       |
|----------------------|-----------|--------|-------|-------|
| convert to match     |  ?        |  ?     |       |       | `!` retired; `to`, `type-of` open |
| at most              |           | `≤`    | `≤`   | `≤`   |
| at least             |           | `≥`    | `≥`   | `≥`   |
| not equal            | `neq`     | `≠`    | `≠`   | `≠`   |
| take                 | `take`    | `↙`    | `↑`   | `↙`   |
| drop                 |           | `↘`    | `↓`   | `↘`   |
| member of            |           | `∈`    | `∊`   | `∊`   |
| index of             | `index`   | `⊗`    | `⍳`   | `⊗`   |
| unique               | `uniq`    | `◴`    | `∪`   | `◴`   |
| where                |           | `⊚`    | `⍸`   | `⊚`   |
| shape                | `shape`   | `△`    | `⍴`   | `△`   |
| reshape              |           | `↯`    | `⍴`   | `↯`   |
| rotate               |           | `↻`    | `⌽`   | `↻`   |
| grade down           |           | `⍖`    | `⍒`   | `⍖`   |
| dip                  |           | `⊙`    |  —    | `⊙`   |
| under                |           | `⍜`    |  —    | `⍜`   |

| **undecided**        |              |        |       |       | NOTES  |
|----------------------|--------------|--------|-------|-------|--------|
| fold from right      | `fold-rt`    |        | `/`   |       |        |
| reduce from right    | `reduce-rt`  |        | `/`   |       |        |

| **not needed**       |              |        |       |       | NOTES  |
|----------------------|--------------|--------|-------|-------|--------|
| text of a value      | `>string`    |        | `⍕`   |       |        |
| same, from last      | `each-right` |        |       |       |        |

