# DESIGN 3

**Project**: Symbolic Stack Language (working title)
**Tagline**: “Symbols all the way down” — a disciplined, homoiconic, Forth-like language with explicit execution and state.

---

## 1. Philosophy

This language borrows the minimalism of Forth but removes implicit execution. Everything is a **value** on the stack: either a **Symbol** or a **literal** (number, string, container). All effects are **explicit**:

- Execute a word: `sym .`
- Access a variable: `sym @` (read), `value sym !` (write)

Goals:
- **Predictable semantics** (no hidden calls or reads)
- **Static stack/type checking**
- **Ergonomic metaprogramming** (symbols are first-class)
- **Straightforward compilation**: interpreter → substitution to primitives → interaction net → executable

---

## 2. Core Concepts

### 2.1 Symbols
A bare identifier pushes a **Symbol** (not executed automatically):

foo -- pushes Symbol("foo")

Symbols are **first-class** but **not constructible from strings**; they appear literally in source.

### 2.2 Execution
Use `.` to execute the word bound to a symbol:

```
foo .  -- execute word 'foo'
```

### 2.3 Variables
Declare with `$`:

```
$ name = "George" ;   -- ( `=` is just for readability. )
```

This binds the symbol `name` to a **mutable cell** of type `string`.

- **Read**: `name @` → value
- **Write**: `value name !`

If implementation uses lvalues (should it?). Then `@` means `: @ ref-lookup . value-get . ;`. 
Something like that. This obviously depends on type. Need to work out exactly.

---

## 3. Containers and Tuples

- **Literal arrays**: `[ "Mr." "Ms." "Dr." ]` → type `[string;3]`
- **Tuples**: `( "Ada" 42 )` → `(string, i64)`

Need acutal syntax for tuple containers.

Containers are **reference types**; mutate them via lvalues:

```
salutes @ append . -- append mutates in place
salutes @ 1 elem . "Dr." swap . !  -- set element 1 to "Dr."
```

(Case in point of how lookup might differ based on type. How to handle? Note the language does not allow address manipulation.
This is not a systems language.)

Read values from containers:

```
salutes @ 1 @  -- value at index 1

-- or lvalue then load:  ( DON'T GET THIS. Why? )
salutes @ 1 elem . @
```

**No auto-deref**: reads and writes are explicit (`@`/`!`). This keeps dispatch and reasoning simple.

---

## 5. Definitions & Signatures

### 5.1 Stack Signatures (`=`)
Set the current expected stack effect for subsequent `:` definitions:

```
= string --
: hello
"Hello, " swap . concat . print .
```


**Type-DSL** inside `=`:
- `T n *` → `[T;n]` (single composite)
- `T n ×` → `T` repeated `n` times in the effect list
- `--` separates inputs from outputs
- **Parens optional**; use to emphasize “one composite”: `= ( string 3 * ) --`

The signature applies to all following `:` words until a new `=` appears.

### 5.2 Word Definitions (`:`)
A `:` line introduces a word bound to a symbol. Indented lines form the body; any line in column 0 ends the definition. No `;` needed.
But maybe explict `;` is better.

```
= string --
: hello
  "Hello, " swap concat . print .

= --
: say
  name @ @ hello .
```

---

## 6. Type & Execution Model

- A **data stack** carries runtime values.
- A **type stack** mirrors it, enabling static checking and multimethod dispatch.
- Values include `T`, lvalues `&T` (safe cells), tuples, fixed arrays `[T;N]`, vectors, options, etc.
- Dispatch keys on required input types (including `&` vs value). There is no implicit deref; the program’s `@`/`!` make intent explicit.

Again, not sure about lvalues stuff.

---

## 7. State & Mutability

- `$ x = <expr>`: initializer is const-evaluated; the resulting type becomes the variable’s type.
- Rarifiers (e.g., `i64 !`, `f64 !`) can disambiguate literal types in initializers.
- **Immutability (optional)**: future `$const x = …` would bind `&const T`; `!` and mutators reject it.

Need sigil op for constants, I think. `¢ X = 100`, perhaps.

---

## 8. Compilation Pipeline

1. **Interpret (dev)**: run the same primitive set the compiler targets.
2. **Substitute**: expand `sym .` calls into their bodies until only primitives remain (plus control ops).
3. **Primitive Stream**: `PUSH`, `LOAD(@)`, `STORE(!)`, `ADD_I64`, `SCONCAT`, `ELEM`, `INDEX`, `BRANCH`, etc.
4. **Interaction Net**: nodes/wires mirror primitives and lvalues.
5. **Snippets**: select backend code (WASM/x86/etc.) per node shape.

Because calls and state ops are explicit (`.`/`@`/`!`), the prim-stream is clear and optimizable.

---

## 9. Advantages

- **Uniform semantics**: bare id = symbol; execution and state are explicit.
- **Type safety**: abstract cell—no raw addresses exposed.
- **Toolability**: errors can show expected/actual stack types precisely.
- **Composability**: symbols can be passed, stored, inspected without `eval`.
- **Compilation-ready**: easy substitution to primitives and net construction.

---

## 10. Example

```txt
♢ hello
> std/io
< name say demo

$ name    = "George"
$ salutes = [ "Mr." "Ms." "Dr." ]

= string --
: hello
  "Hello, " swap concat . print .

= --
: say
  name @ @ hello .

: demo
  "Mrs." salutes @ append .
  salutes @ 1 elem . @ println .

: main
  say .
  demo .

## Improving readability.

The main symbols `. @ !` can be abutted to words, e.g. `salutes@ append.`. 
Word names ending in those characters are illegal.


## Qustions

### lvalues

ChatGPT proposed using *lvalue* handles. Proposal:

```
$ name = "George" ;
```

This binds the symbol `name` to a **mutable cell** of type `string`.

- **Read**: `name @ @` → value
  - First `@`: `( Symbol -- &T )` resolves to a **safe cell handle** (lvalue), *not* a raw address.
  - Second `@`: `( &T -- T )` loads the value from that cell.
- **Write**: `value name !` or `value name @ !`  
  - `!` accepts `( T Symbol -- )` *or* `( T &T -- )`.

> **No raw addresses**: `&T` is an abstract, type-checked lvalue. Only `@`, `!`, and a few container ops can use it.

But I do not see the point of this at all. Why would we want to "double reference" like this? Do we need to deal with this?
If so I imagine the type system can make it easier. If need be we can have another op for get lvalue. `.@` or something, not sure.

### New Project

Should I start a new project for this? Maybe develop in parrallel with prior non-symbol design, and see which turns out best?

