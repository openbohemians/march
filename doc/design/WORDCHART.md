| Meaning              | March Word | March  | APL   | Uiua  | NOTES  |
|----------------------|------------|--------|-------|-------|--------|
| add                  | `add`      |  `+`   | `+`   | `+`   |        |
| subtract             | `sub`      |  `-`   | `-`   | `-`   |        |
| multiply             | `mul`      |  `⋅`   | `×`   | `×`   |        |
| divide               | `div`      |  `÷`   | `÷`   | `÷`   |        |
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
| element at           | `at`       |  `⊡`   | `⌷`   | `⊡`   |        |
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
| word on array's end  | `within`   |        |       | `⍜`   | Uiua's under is nearest equivalent |
| elements onto stack  | `spread`   |        |       |       |        |
| insert in gap k      | `insert`   |        |       |       | 0 prepends, -1 appends |
| remove element k     | `remove`   |        |       |       |        |
| map keys             | `keys`     |        |       |       |        |
| map values           | `values`   |        |       |       |        |
| each, collect        | `map`      |  `∵`   | `¨`   | `∵`   |        |
| each pair, in step   | `zip`      |        |       |       |        |
| outer product        | `table`    |  `×`   | `∘.`  | `⊞`   | `×` is the table of products |
| dot product          | `dot`      |  ?     | `+.×` |       | contraction: matrix · matrix is the matrix product; `⊙`? (Hadamard in math) |
| each, thread stack   | `each`     |        |       |       |        |
| fold from left       | `fold`     |        |       | `∧`   |        |
| fold from right      | `rfold`    |        | `/`   |       |        |
| reduce from left     | `reduce`   | `/` ?  |       | `/`   |        |
| reduce from right    | `rreduce`  |        | `/`   |       |        |
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
| power                | `pow`      |  `ⁿ`   | `*`   | `ⁿ`   |        |

| **add to March**     |           |        |       |       |
|----------------------|-----------|--------|-------|-------|
| not                  | `not`     |  `¬`   | `~`   | `¬`   |
| and                  | `and`     |  `∧`   | `∧`   |  —    |
| or                   | `or`      |  `∨`   | `∨`   | `∨`   |
| square root          | `sqrt`    |  `√`   | `*.5` | `√`   |

| **not yet in March** |           |        |       |       |
|----------------------|-----------|--------|-------|-------|
| convert to match     |  ?        | `!`    |       |       |
| reciprocal           |           |  ?     |       |       |
| negate               | `neg`     | `¯`    | `-`   | `¯`   |
| at most              |           | `≤`    | `≤`   | `≤`   |
| at least             |           | `≥`    | `≥`   | `≥`   |
| not equal            | `neq`     | `≠`    | `≠`   | `≠`   |
| keep by mask         | `keep`    | `▽`    | `/`   | `▽`   |
| take                 | `take`    | `↙`    | `↑`   | `↙`   |
| drop                 |           | `↘`    | `↓`   | `↘`   |
| member of            |           | `∈`    | `∊`   | `∊`   |
| index of             | `index`   | `⊗`    | `⍳`   | `⊗`   |
| unique               | `uniq`    | `◴`    | `∪`   | `◴`   |
| where                |           | `⊚`    | `⍸`   | `⊚`   |
| shape                | `shape`   | `△`    | `⍴`   | `△`   |
| reshape              |           | `↯`    | `⍴`   | `↯`   |
| transpose            | `transpose` | `⍉`  | `⍉`   | `⍉`   |
| rotate               |           | `↻`    | `⌽`   | `↻`   |
| grade up             |           | `⍏`    | `⍋`   | `⍏`   |
| grade down           |           | `⍖`    | `⍒`   | `⍖`   |
| dip                  |           | `⊙`    |  —    | `⊙`   |
| under                |           | `⍜`    |  —    | `⍜`   |

| **not needed**       |              |        |       |       | NOTES  |
|----------------------|--------------|--------|-------|-------|--------|
| swap subtract        | `subadd`  |  `-+`  |       |       | `~ -` |
| subtract             | `addsub`  |  `+-`  |       |       | `-` |
| swap divide          | `divmul`  |  `÷×`  |       |       | `~ ÷` |
| divide               | `muldiv`  |  `×÷`  |       |       | `÷` |
| text of a value      | `>string`    |        | `⍕`   |       |        |
| same, from last      | `each-right` |        |       |       |        |

