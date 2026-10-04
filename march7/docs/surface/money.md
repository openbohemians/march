# money

Exact money arithmetic, counted in cents. This file is a literate module: its
headings are namespaces, its `march` code blocks are the code, and the loader
skips the prose. Notes marked F1, F2 and so on point to the findings in
[README.md](README.md).

## The type

Money is a distinct type represented as an int, so a count of cents cannot be
mixed up with a plain number by accident. (F1)

```march
Money : distinct int ;
```

## Units

Units are ordinary words. `Money !` converts an int to Money (F2), and
`dollars` is built on `cents`.

```march
= int ;
cents   : Money ! ;
dollars : 100 * cents ;
```

Both are made of invertible words, so their inverses come for free. Undoing
`cents` unwraps an amount to its int, and undoing `dollars` converts to whole
dollars, failing when the amount is not a whole number of dollars. (F3)

```text
5 dollars [ cents ] undo        -- 500
550 cents [ dollars ] undo      -- fails: not a whole number of dollars
```

## Arithmetic

Money adds to Money. Each clause joins the `+` and `-` families for Money
inputs. (F5)

```march
= Money Money ;
+ : [ cents ] undo swap [ cents ] undo + cents ;
- : [ cents ] undo swap [ cents ] undo swap - cents ;
```

If a context could take its inputs apart, as Factor's `case` does, the same
two clauses would read like this (F4):

```text
= cents cents ;          -- both inputs were made by cents; the bodies get ints
+ : + cents ;
- : - cents ;
```

Money scales by an int. Multiplying two amounts of money means nothing, so
there is no clause for it, and `price price *` is a compile error.

```march
= Money int ;
* : swap [ cents ] undo * cents ;
```

Division rounds toward zero, like integer division.

```march
/ : swap [ cents ] undo swap / cents ;
```

The block above has no `=` line of its own. It is still under
`= Money int`, set two blocks earlier, with prose in between. (F7)

## Prices

`price` is a role, another name for Money, so signatures can say what an
amount means.

```march
price : Money ;

= price int -> price ;
discount : 100 swap - * 100 / ;      -- percent off
```

## Printing

Printing is a clause of the `print` family for Money. The calls to `print`
inside it resolve to the string and int clauses. (F6)

```march
= Money ;
print : [ cents ] undo 100 /mod  "$" print  print  "." print  >str 2 "0" pad-left print ;
```

## Example

```text
3 dollars 50 cents +  2 *  15 discount  print      -- $5.95
```

3 dollars and 50 cents is 350 cents. Doubled it is 700, and 15% off leaves
700 × 85 / 100 = 595 cents.
