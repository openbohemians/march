# `money`

Exact money arithmetic, counted in cents. This file is a literate module.
Headings whose text is code are March: `` `money` `` is the namespace, and
headings such as `` `< Money Money >` `` are contexts. Other headings are
prose. The loader feeds the March headings and the `march` code blocks to the
interpreter and skips the rest (F20). Notes marked F1, F2 and so on point to
the findings in [README.md](README.md). In the code, `=` is dup and `~` is
swap.

## The type

Money is a distinct type represented as an i64, so a count of cents cannot be
mixed up with a plain number by accident. (F1)

```march
Money : distinct i64 ;
```

## Units

Units are ordinary words. `Money !` converts an i64 to Money (F2), and
`dollars` is built on `cents`.

### `< i64 >`

```march
cents   : Money ! ;
dollars : 100 * cents ;
```

Both are made of invertible words, so their inverses come for free. Undoing
`cents` unwraps an amount to its i64, and undoing `dollars` converts to whole
dollars, failing when the amount is not a whole number of dollars. (F3)

```text
5 dollars [ cents ] undo        -- 500
550 cents [ dollars ] undo      -- fails: not a whole number of dollars
```

## Arithmetic

Money adds to Money. Each clause joins the `+` and `-` families for Money
inputs. (F5)

### `< Money Money >`

```march
+ : [ cents ] undo ~ [ cents ] undo + cents ;
- : [ cents ] undo ~ [ cents ] undo ~ - cents ;
```

If a context could take its inputs apart, as Factor's `case` does, the same
two clauses would read like this (F4):

```text
### < cents cents >      -- both inputs were made by cents; the bodies get i64s
+ : + cents ;
- : - cents ;
```

Money scales by an i64. Multiplying two amounts of money means nothing, so
there is no clause for it, and `price price *` is a compile error.

### `< Money i64 >`

```march
* : ~ [ cents ] undo * cents ;
```

Division rounds toward zero, like integer division.

```march
/ : ~ [ cents ] undo ~ / cents ;
```

The block above is still under `` `< Money i64 >` ``, with prose in between,
and the heading shows it. (F7)

## Prices

`price` is a role, another name for Money, so signatures can say what an
amount means.

```march
price : Money ;
```

### `< price i64 -> price >`

```march
discount : 100 ~ - * 100 / ;      -- percent off
```

## Printing

Printing is a clause of the `print` family for Money. The calls to `print`
inside it resolve to the string and i64 clauses. (F6)

### `< Money >`

```march
print : [ cents ] undo 100 /mod  "$" print  print  "." print  >str 2 "0" pad-left print ;
```

## Example

```text
3 dollars 50 cents +  2 *  15 discount  print      -- $5.95
```

3 dollars and 50 cents is 350 cents. Doubled it is 700, and 15% off leaves
700 × 85 / 100 = 595 cents.
