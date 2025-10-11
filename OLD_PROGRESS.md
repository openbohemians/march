# OLD PROGRESS

We started fresh, so this no longer applies, but can be used for reference.

### Current Status (2025-10-08)

#### Implemented Features ✅

**Core Stack Language:**
- ✅ Basic stack operations (dup, drop, swap, over, rot)
- ✅ Arithmetic (+, -, *, /)
- ✅ Comparison (eq, lt, gt, lte, gte)
- ✅ Logic (and, or, not)
- ✅ Comments: `--` line comments, `( )` block comments (with nesting)

**Data Types:**
- ✅ Numbers (i64)
- ✅ Strings (with escape sequences: `\n`, `\t`, `\"`, `\\`)
- ✅ Quotations (code as data: `[ ... ]`)
- ✅ **Adaptive Collections**: `{ }` markers create Arrays or Tuples automatically
  - Homogeneous → Array `[T]`: `{ 1 2 3 }` → `{1 2 3}`
  - Heterogeneous → Tuple `(T1, T2, ...)`: `{ 1 "x" 3 }` → `(1, "x", 3)`
- ✅ Type tracking (shadow type stack)

**Control Flow:**
- ✅ Quotations: `[ code ]`
- ✅ Execution: `#do` (N times), `call` (once)
- ✅ Conditional: `if` (cond {true} {false} if)
- ✅ Loop index: `i0`
- ✅ Sugar: `#[ code ]` → `[ code ] swap #do`

**State Variables:**
- ✅ Declaration: `$ varname = expression ;` (comptime evaluation)
- ✅ Initialization: supports expressions, arrays, quotations, references to other vars
- ✅ Auto-fetch: `varname` pushes value
- ✅ Store operators: `->` (pop & store), `=>` (copy & store)
- ✅ Type safety: enforced on store

**String Operations:**
- ✅ Concatenation: `++`
- ✅ Length: `str-len`

**Collection Operations:**
- ✅ Marker-based construction: `{ expr1 expr2 ... }` (adaptive: Array or Tuple)
- ✅ Length: `array-len`
- ✅ Get element: `@` or `array-get` (works on arrays and tuples)
- ✅ Set element: `!` or `array-set` (arrays only, type-checked)

**User-Defined Words:**
- ✅ Definition syntax: `: name ... ;`
- ✅ Type signatures: `= inputs -> outputs ;`
- ✅ Multi-methods: multiple signatures per word name
- ✅ Specificity-based dispatch
- ✅ Type variables: `a`, `b` for polymorphism

**Safety:**
- ✅ Recursion depth limit (1000)
- ✅ Iteration count limit (1,000,000)
- ✅ Type checking via shadow type stack

### Testing

- ✅ **15 test files** in `tests/` directory
- ✅ **Makefile** for running tests:
  - `make test` - Run all tests (silent)
  - `make test-verbose` - Run with output
  - `make test-one TEST=<file>` - Run specific test
- ✅ All tests passing!

### What's Next?

From the original plan:
- [x] if/then/else construct → Implemented as `if` word!
- [x] Arrays and tuples → Adaptive collections!
- [ ] More collection operations (set element, slice, etc.)
- [ ] Move to inet compilation phase

---

## 2025-10-06 - Session Notes

### Current Discussion: Comment Syntax

**User's Proposal:** Use `--` to start and end comments (delimiter-paired style)
- Example: `-- this is a comment --`
- Concern raised: Requires tracking mode state
- Concern raised: Cannot nest comments
- User unsure if nesting is needed

**Alternative Approaches:**
1. **Line comments:** `-- comment until end of line` (like SQL, Haskell)
2. **Parenthesis comments:** `( comment )` (traditional Forth style)
3. **Backslash comments:** `\ comment until end of line` (also traditional Forth)
4. **Block comments:** Paired delimiters with nesting support

**Current Implementation Status:**
- No comment syntax implemented yet
- Parser uses simple `split_whitespace()` tokenization (src/main.rs:320)
- Would need to filter comments before/during tokenization

**Implementation Completed:**
- ✅ Implemented `--` line comments (comment until newline)
- ✅ Implemented `( )` block comments (traditional Forth, with nesting support)
- ✅ Both syntaxes tested and working

**Implementation Details:**
- Added `strip_comments()` method in src/main.rs:325-360
- Line comments: `--` skips until newline (newline preserved)
- Block comments: `( ` requires space after opening paren to distinguish from future syntax
- Block comments support nesting (depth tracking)
- Comment stripping happens before tokenization in `eval()` method

**Test Results:**
- `5 10 + -- comment` → works ✅
- `20 30 ( comment ) *` → works ✅
- `100 ( outer ( inner ) ) 50 +` → nested comments work ✅

---

## Implementation Strategy Discussion

### Current State (Phase 0)
- ✅ Basic FORTH parser with comments
- ✅ Basic stack operations, arithmetic, logic
- ✅ Quotations and control flow (#do, call)
- ✅ Type tracking (shadow type stack)
- ✅ User-defined words (`:` definitions)

### What to Build in Rust Interpreter Before Compiling to Inets?

**Option A: Minimal - Jump to Inets Soon**
- Keep current feature set minimal
- Move to inet compilation ASAP
- Implement advanced features (state, contexts, arrays) directly in inet IR
- Pros: Faster to compilation phase, less throwaway code
- Cons: Harder to test/validate advanced features

**Option B: Rich Interpreter - Validate Design First**
- Add more features to interpreter first:
  - Arrays and array literals
  - String type
  - State system
  - Context system (multiple word definitions with guards)
  - Type signatures and checking
- Then compile proven design to inets
- Pros: Validate language design, easier debugging
- Cons: More interpreter code to write/maintain

**Recommendation: Hybrid Approach (Option B-lite)**
Build enough in the interpreter to validate core concepts, then move to inets:

1. **Next in Rust interpreter:**
   - [x] Strings and string literals (syntax: `"normal string"`)
   - [x] Simple state variables
   - [ ] Arrays and array literals
   - [ ] Basic type signatures on word definitions
   - [ ] if/then/else construct (note: conditionals via `[ code ] -1 #do` already work!)

**String Implementation Complete:**
- String type added to Value/Type enums
- String literals: `"text with spaces"` (preserved through tokenization)
- Escape sequences: `\n`, `\t`, `\"`, `\\`
- Operations: `++` (concat), `str-len` (length → i64)
- Type checking: enforced via type stack
- Display: `.` prints strings directly

**State Variables Implementation Complete:**
- Declaration syntax: `$ varname = initialvalue ;`
  - Example: `$ count = 0 ;`
  - Type inferred from initial value (supports i64 and string)
- **Auto-fetch**: Variable names automatically push their value
  - `count` pushes the value directly (works naturally with all operations)
  - No special StateRef type, no transparency issues
- **Immediate store operators**: `->` and `=>` read variable name from token stream
  - `42 -> count` pops and stores 42 into count
  - `42 => count` copies (peeks) and stores 42 into count (leaves 42 on stack)
  - Works in definitions too: `: inc counter 1 + -> counter ;`
  - No symbols on stack, clean and simple
- Type safety: Store enforces declared type matches value type
- Separate namespace: State variables don't collide with word definitions
- Usage examples:
  ```forth
  $ count = 0 ;         -- declare
  count 1 + -> count    -- increment (auto-fetch, pop and store)
  count .               -- print

  10 20 30 => x -> y .  -- copy 30 to x, pop 30 to y, print 20
  ```

**Note on conditionals:** Already work via quotations:
- True: `[ do-this ] -1 #do` (executes once)
- False: `[ skip-this ] 0 #do` (executes zero times)
- No `else` branch yet

2. **Skip in interpreter, do in inet phase:**
   - Context system (multiple definitions with guards)
   - Dependent types
   - Advanced optimizations

3. **Then move to inets:**
   - Design inet IR representation
   - Implement FORTH→Inet compiler
   - Build inet optimizer
   - Generate assembly from inets

This validates the core language design without over-investing in the interpreter.
