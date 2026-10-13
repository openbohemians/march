# Dots or No Dots

Status: record of a comparison, 2026-10-12, made with Thomas to decide how
code marks applying a word. Decided: C with capitals. A name applies, a
capitalized name is a symbol, and a definition puts its name first. Built
in march8 (commit 0a80aba) and recorded in TYPES.md 2.2. Three of the
programs run as march8/examples/summary.march, door.march and
syllogism.march.

The comparison is kept as it was, except that "today" now reads "before":
the notation march8 had until then, every applied name marked by a dot.
Section 4 notes where each finding stands since.

Since, the brackets changed too. C kept "names stay names" in brackets, so
code and brackets read differently, and its constructors kept their dots,
`< string i64 map. >`. Thomas then saw types as structure built by words:
a type's name gives the type, so names apply in brackets as in code, and
the dots there went, `< string i64 map >`; and parsing became converting,
`"42" i64!`, `!` converting by a type, which is data and not called
(TYPES.md 2.2).
The programs below are as they were compared.

## 1. The forms

The same four programs in the notation before and in the two candidates,
and the logic notation we sketched. A mark is a dot that applies a word, or
a `!` or prime that marks a symbol. The `name:` labels are the same in both
candidates, so they aren't counted.

- **Before.** Names stay names and a dot applies one. The name comes last:
  `[ … ] fib def.`
- **A.** The rule before, with the name first as a label: `fib: [ … ] def.`
- **A + punctuation.** A, where a word made only of punctuation applies
  without a dot (Thomas's rule): `a b + c ⋅`. Shown with core's `=` and `~`
  for `dup` and `swap`, its best case. Such a word is then a symbol only
  with a mark: `+:` as a label, `+!` in a bracket, `[ + ]` to pass it.
- **C.** In bodies a name applies and `name'` holds it back. In brackets
  names stay names and a dot applies, exactly as before: `fib: [ < n > dup
  1 -+ fib … ] def`
- **C + capitals.** C, where a capitalized word is a symbol: `Socrates Is
  Human fact`, `< string Snake >`. A capital adds no character, so it isn't
  counted. A type passed as a value is written in a bracket: `< f64 >
  parse`. One capital letter is a logic variable, as in Prolog.

## 2. Marks by program

| Program | A | A + punctuation | C | C + capitals |
|---|---:|---:|---:|---:|
| CSV summary | 57 | 50 | 8 | 6 |
| Library code | 57 | 41 | 10 | 10 |
| Door state machine | 24 | 24 | 17 | 0 |
| Syllogism as families | 25 | 25 | 12 | 0 |
| Logic notation (proposed) | 6 | 6 | 18 | 0 |

## 3. The programs

### CSV summary

Verb-heavy. Reads a CSV on standard input and prints the total and average
of each numeric column. It printed `qty: total 9.0, average 3.0` and
`price: total 3.25, average 1.083…` for a three-row fruit file.

**Before**, 57 marks:

```
[ < nil > drop. 0.0 ] or-zero def.
[ < f64 > ] or-zero def.
[ < nil > drop. false. ] number? def.
[ < f64 > drop. true. ] number? def.

[ < string ary. > rest. [ f64 parse. number?. ] map. true. [ and. ] fold. ] numeric? def.
[ < string ary. > rest. [ f64 parse. or-zero. ] map. ] numbers def.
[ < f64 ary. > [ drop. 1.0 ] map. sum. ] count def.
[ < f64 ary. > dup. sum. swap. count. /*. ] mean def.

[ < string ary. numeric? > dup. first. swap. numbers. dup. sum. swap. mean.
  "\_: total \_, average \_\n;" write. ] report def.
[ < string ary. > drop. ] report def.

stdin-read. lines. [ "," split. ] map. transpose. [ report. ] each.
```

**A**, 57 marks:

```
or-zero: [ < nil > drop. 0.0 ] def.
or-zero: [ < f64 > ] def.
number?: [ < nil > drop. false. ] def.
number?: [ < f64 > drop. true. ] def.

numeric?: [ < string ary. > rest. [ f64 parse. number?. ] map. true. [ and. ] fold. ] def.
numbers: [ < string ary. > rest. [ f64 parse. or-zero. ] map. ] def.
count: [ < f64 ary. > [ drop. 1.0 ] map. sum. ] def.
mean: [ < f64 ary. > dup. sum. swap. count. /*. ] def.

report: [ < string ary. numeric? > dup. first. swap. numbers. dup. sum. swap. mean.
  "\_: total \_, average \_\n;" write. ] def.
report: [ < string ary. > drop. ] def.

stdin-read. lines. [ "," split. ] map. transpose. [ report. ] each.
```

**A + punctuation**, 50 marks:

```
or-zero: [ < nil > drop. 0.0 ] def.
or-zero: [ < f64 > ] def.
number?: [ < nil > drop. false. ] def.
number?: [ < f64 > drop. true. ] def.

numeric?: [ < string ary. > rest. [ f64 parse. number?. ] map. true. [ and. ] fold. ] def.
numbers: [ < string ary. > rest. [ f64 parse. or-zero. ] map. ] def.
count: [ < f64 ary. > [ drop. 1.0 ] map. sum. ] def.
mean: [ < f64 ary. > = sum. ~ count. /* ] def.

report: [ < string ary. numeric? > = first. ~ numbers. = sum. ~ mean.
  "\_: total \_, average \_\n;" write. ] def.
report: [ < string ary. > drop. ] def.

stdin-read. lines. [ "," split. ] map. transpose. [ report. ] each.
```

**C**, 8 marks:

```
or-zero: [ < nil > drop 0.0 ] def
or-zero: [ < f64 > ] def
number?: [ < nil > drop false ] def
number?: [ < f64 > drop true ] def

numeric?: [ < string ary. > rest [ f64' parse number? ] map true [ and ] fold ] def
numbers: [ < string ary. > rest [ f64' parse or-zero ] map ] def
count: [ < f64 ary. > [ drop 1.0 ] map sum ] def
mean: [ < f64 ary. > dup sum swap count /* ] def

report: [ < string ary. numeric? > dup first swap numbers dup sum swap mean
  "\_: total \_, average \_\n;" write ] def
report: [ < string ary. > drop ] def

stdin-read lines [ "," split ] map transpose [ report ] each
```

**C + capitals**, 6 marks:

```
or-zero: [ < nil > drop 0.0 ] def
or-zero: [ < f64 > ] def
number?: [ < nil > drop false ] def
number?: [ < f64 > drop true ] def

numeric?: [ < string ary. > rest [ < f64 > parse number? ] map true [ and ] fold ] def
numbers: [ < string ary. > rest [ < f64 > parse or-zero ] map ] def
count: [ < f64 ary. > [ drop 1.0 ] map sum ] def
mean: [ < f64 ary. > dup sum swap count /* ] def

report: [ < string ary. numeric? > dup first swap numbers dup sum swap mean
  "\_: total \_, average \_\n;" write ] def
report: [ < string ary. > drop ] def

stdin-read lines [ "," split ] map transpose [ report ] each
```

### Library code

From core.march and the tests: a literal’s conversion, a constant, the
`num` class, arithmetic lifted over arrays, `take`, and Fibonacci by value
patterns. The `num` line moves into a type bracket, as agreed.

**Before**, 57 marks:

```
[ < int# -- i64 > int#>i64. ] i64 def.
[ 1 bool. ] true def.

i64 f64 or. money or. int# or. dec# or. num def.

[ < a ary. atom > swap. [ over. +. ] map. swap. drop. ] + def.
[ < atom a ary. > [ over. +. ] map. swap. drop. ] + def.

[ < a ary. string or. i64 negative? > -. over. length. min. over. length. dup. rot. -+. 1 +. swap. slice. ]
  take def.
[ < a ary. string or. i64 > over. length. min. 1 swap. slice. ] take def.

[ < 0 > ] fib def.
[ < 1 > ] fib def.
[ < n > dup. 1 -+. fib. swap. 2 -+. fib. +. ] fib def.
```

**A**, 57 marks:

```
i64: [ < int# -- i64 > int#>i64. ] def.
true: [ 1 bool. ] def.

num: < i64 f64 or. money or. int# or. dec# or. > def.

+: [ < a ary. atom > swap. [ over. +. ] map. swap. drop. ] def.
+: [ < atom a ary. > [ over. +. ] map. swap. drop. ] def.

take: [ < a ary. string or. i64 negative? > -. over. length. min. over. length. dup. rot. -+. 1 +. swap. slice. ] def.
take: [ < a ary. string or. i64 > over. length. min. 1 swap. slice. ] def.

fib: [ < 0 > ] def.
fib: [ < 1 > ] def.
fib: [ < n > dup. 1 -+. fib. swap. 2 -+. fib. +. ] def.
```

**A + punctuation**, 41 marks:

```
i64: [ < int# -- i64 > int#>i64. ] def.
true: [ 1 bool. ] def.

num: < i64 f64 or. money or. int# or. dec# or. > def.

+: [ < a ary. atom > ~ [ over. + ] map. ~ drop. ] def.
+: [ < atom a ary. > [ over. + ] map. ~ drop. ] def.

take: [ < a ary. string or. i64 negative? > - over. length. min. over. length. = rot. -+ 1 + ~ slice. ] def.
take: [ < a ary. string or. i64 > over. length. min. 1 ~ slice. ] def.

fib: [ < 0 > ] def.
fib: [ < 1 > ] def.
fib: [ < n > = 1 -+ fib. ~ 2 -+ fib. + ] def.
```

**C**, 10 marks:

```
i64: [ < int# -- i64 > int#>i64 ] def
true: [ 1 bool ] def

num: < i64 f64 or. money or. int# or. dec# or. > def

+: [ < a ary. atom > swap [ over + ] map swap drop ] def
+: [ < atom a ary. > [ over + ] map swap drop ] def

take: [ < a ary. string or. i64 negative? > - over length min over length dup rot -+ 1 + swap slice ] def
take: [ < a ary. string or. i64 > over length min 1 swap slice ] def

fib: [ < 0 > ] def
fib: [ < 1 > ] def
fib: [ < n > dup 1 -+ fib swap 2 -+ fib + ] def
```

**C + capitals**, 10 marks:

```
i64: [ < int# -- i64 > int#>i64 ] def
true: [ 1 bool ] def

num: < i64 f64 or. money or. int# or. dec# or. > def

+: [ < a ary. atom > swap [ over + ] map swap drop ] def
+: [ < atom a ary. > [ over + ] map swap drop ] def

take: [ < a ary. string or. i64 negative? > - over length min over length dup rot -+ 1 + swap slice ] def
take: [ < a ary. string or. i64 > over length min 1 swap slice ] def

fib: [ < 0 > ] def
fib: [ < 1 > ] def
fib: [ < n > dup 1 -+ fib swap 2 -+ fib + ] def
```

### Door state machine

Symbols with a few verbs. A state and an event make the next state. Every
event is known, so the machine runs while compiling: the whole program
compiles to the constant `true`. `locked open door.` is a compile error.

**Before**, 24 marks:

```
[ < closed! open! > drop. drop. opened ] door def.
[ < opened! close! > drop. drop. closed ] door def.
[ < closed! lock! > drop. drop. locked ] door def.
[ < locked! unlock! > drop. drop. closed ] door def.

closed open door. close door. lock door. locked eq?.
```

**A**, 24 marks:

```
door: [ < closed! open! > drop. drop. opened ] def.
door: [ < opened! close! > drop. drop. closed ] def.
door: [ < closed! lock! > drop. drop. locked ] def.
door: [ < locked! unlock! > drop. drop. closed ] def.

closed open door. close door. lock door. locked eq?.
```

**A + punctuation**, 24 marks:

```
door: [ < closed! open! > drop. drop. opened ] def.
door: [ < opened! close! > drop. drop. closed ] def.
door: [ < closed! lock! > drop. drop. locked ] def.
door: [ < locked! unlock! > drop. drop. closed ] def.

closed open door. close door. lock door. locked eq?.
```

**C**, 17 marks:

```
door: [ < closed' open' > drop drop opened' ] def
door: [ < opened' close' > drop drop closed' ] def
door: [ < closed' lock' > drop drop locked' ] def
door: [ < locked' unlock' > drop drop closed' ] def

closed' open' door close' door lock' door locked' eq?
```

**C + capitals**, 0 marks:

```
door: [ < Closed Open > drop drop Opened ] def
door: [ < Opened Close > drop drop Closed ] def
door: [ < Closed Lock > drop drop Locked ] def
door: [ < Locked Unlock > drop drop Closed ] def

Closed Open door Close door Lock door Locked eq?
```

### Syllogism as families

Facts and a rule written by hand as the clauses `fact` and `rule` would
make. It ran, and compiled to three constants: `true false true`.

**Before**, 25 marks:

```
[ < socrates! human! > drop. drop. true. ] known def.
[ < plato! human! > drop. drop. true. ] known def.
[ < x y > drop. drop. false. ] known def.

[ < x mortal! > drop. human known. ] is def.
[ < x y > known. ] is def.

socrates mortal is. zeus mortal is. plato human is.
```

**A**, 25 marks:

```
known: [ < socrates! human! > drop. drop. true. ] def.
known: [ < plato! human! > drop. drop. true. ] def.
known: [ < x y > drop. drop. false. ] def.

is: [ < x mortal! > drop. human known. ] def.
is: [ < x y > known. ] def.

socrates mortal is. zeus mortal is. plato human is.
```

**A + punctuation**, 25 marks:

```
known: [ < socrates! human! > drop. drop. true. ] def.
known: [ < plato! human! > drop. drop. true. ] def.
known: [ < x y > drop. drop. false. ] def.

is: [ < x mortal! > drop. human known. ] def.
is: [ < x y > known. ] def.

socrates mortal is. zeus mortal is. plato human is.
```

**C**, 12 marks:

```
known: [ < socrates' human' > drop drop true ] def
known: [ < plato' human' > drop drop true ] def
known: [ < x y > drop drop false ] def

is: [ < x mortal' > drop human' known ] def
is: [ < x y > known ] def

socrates' mortal' is zeus' mortal' is plato' human' is
```

**C + capitals**, 0 marks:

```
known: [ < Socrates Human > drop drop true ] def
known: [ < Plato Human > drop drop true ] def
known: [ < x y > drop drop false ] def

is: [ < x Mortal > drop Human known ] def
is: [ < x y > known ] def

Socrates Mortal is Zeus Mortal is Plato Human is
```

### Logic notation, proposed

What `fact`, `if`, `rule` and `ask` would let you write. Not runnable yet:
they need a word that builds a clause from symbols.

**A**, 6 marks:

```
socrates is human fact.
plato is human fact.
x is mortal if. x is human rule.

socrates is mortal ask.
zeus is mortal ask.
```

**C**, 18 marks:

```
socrates' is' human' fact
plato' is' human' fact
x' is' mortal' if x' is' human' rule

socrates' is' mortal' ask
zeus' is' mortal' ask
```

**C + capitals**, 0 marks:

```
Socrates Is Human fact
Plato Is Human fact
X Is Mortal if X Is Human rule

Socrates Is Mortal ask
Zeus Is Mortal ask
```

## 4. What the samples turned up

1. **A family can't apply itself to symbols.** A family that applies itself
   is compiled as a word of its own, and symbols exist only while
   compiling, so `is` couldn't call itself. The facts had to become a
   separate family, `known`. With known symbols, such a family should
   unfold while compiling, under the depth limit. The logic notation needs
   this first. *Since:* a symbol is a value at run time, so a family
   applies itself to symbols as to numbers, compiled as a word that tests
   them as it runs; the syllogism is one family, `is`. Unfolding a
   recursion on known inputs is still open, TYPES.md 3.16.
2. **Whole programs fold.** The door machine compiles to `true` and the
   syllogism to three constants. The machine and the logic run entirely
   while compiling. *Since:* the syllogism runs: its rule applies `is`
   again, which makes `is` a word of its own (finding 1).
3. **A symbol can't be left for run time.** The door ends with `locked
   eq?.` because a final state left on the stack has no run-time value.
   Showing one needs `show` for symbols. *Since:* a symbol is its name at
   run time, an interned string, and shows as it reads back; the door ends
   with `Locked`, and runs a second time on events made symbols from text,
   `"Open" sym!`.
4. **The door's error is vague.** It says ``no word `door` for sym sym: no
   clause's guard holds``. It should name the symbols: ``no clause of
   `door` for locked open``. *Since:* fixed in 0a80aba; it says ``no word
   `door` for Locked Open``.
5. **There's no conversion from i64 to f64,** so the summary's `count` adds
   up ones. *Since:* there is, `12 i64! f64!`, a clause under `f64`, and the
   summary divides by `length f64!`.
6. **In C, a type passed as an argument takes a prime:** `f64' parse`. It
   could also be written `< f64 > parse`. *Since:* `parse` is gone; a string
   converts as a literal does, `[ f64! ] map` (TYPES.md 2.2).

## 5. Claude's read

- Verb-heavy code, which is most code, loses about 85% of its marks in C:
  57 to 8, and 57 to 10.
- The punctuation rule takes A from 57 to 50 and 41 there. Most applied
  words have letters: `over`, `length`, `map`, `drop`, `slice`, `def`.
- The rule still earns its place on principle, in A or C. It is how math
  reads: operators act, and a named function must be applied, as f(x) is.
  It also fits "Greek letters are letters": `π` is a name and `×` an
  operator.
- The bracket marks, `!` or prime, are the same in A and C, so they don't
  decide anything. Without them, the door is 16 to 9 and the syllogism 20
  to 7.
- Without capitals, the proposed logic notation favors A, 6 to 18. Its
  lines are almost all symbols.
- Capitals settle it. The symbolic programs and the logic notation need no
  marks at all, and in a bracket a capital tells a symbol from a type by
  spelling, so `!` retires.

## 6. The decision

Thomas, on C without capitals: "I am not writing `socrates' is' human'
fact.` So that means we have two choices: 1 we have alternative brackets
that flip it around, … or 2 we use capitals. Yeah I never thought I'd say
that, but capitalization seems the clearer solution." Then, on C with
capitals: "I agree."

Under C the punctuation rule is moot, since every name applies. A
capitalized word as a symbol follows Haskell's constructors, Rust's
variants and Elixir's atoms; TYPES.md 2.2 gives the rule as built.
