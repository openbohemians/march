# Namespaces in March2

## Overview

Namespaces in March2 provide a way to organize words and avoid naming conflicts. They use a **stack-based architecture** with dotted qualified names (like `io.net.http.request`) for efficient lookup and scoping.

## Basic Concepts

### Namespace Stack

March2 maintains a **stack of namespaces**, where each namespace is a HashMap of word definitions. When looking up a word:

1. **Qualified names** (e.g., `io.net.request`) are looked up in the specific namespace - **O(1)**
2. **Unqualified names** (e.g., `request`) walk the stack backwards - **O(k)** where k is stack depth

The root namespace (empty name "") is always at the bottom of the stack.

### Namespace Names

Namespace names use **dotted notation**:
- `mylib` - single level
- `io.net` - nested namespace
- `io.net.http` - deeply nested

Namespaces are **hierarchical in name only** - `io.net.http` is a separate namespace from `io.net`, not a child.

## Creating Namespaces

### NAMESPACE. word

```forth
NAMESPACE. name ;
```

Creates or switches to a namespace. If the namespace already exists on the stack, it does nothing. Otherwise:
- Creates a new empty HashMap
- Pushes it onto the namespace stack
- Records the name in `namespace_names`

**Example:**
```forth
NAMESPACE. mylib ;
: greet "Hello from mylib" . ;
```

The word `greet` is now defined in the `mylib` namespace.

## Calling Words

### Qualified Names

Call words using dotted notation to specify the namespace:

```forth
mylib.greet    -- calls greet from mylib namespace
```

**Lookup process:**
1. Split on the rightmost `.` → namespace part and word part
2. Find namespace by name
3. Look up word in that namespace's HashMap

**Performance:** O(1) lookup in the target namespace.

### Unqualified Names

Call words without namespace prefix:

```forth
greet    -- searches namespace stack backwards
```

**Lookup process:**
1. Walk namespace stack from top to bottom
2. Return first matching word found

**Performance:** O(k) where k is the number of namespaces on the stack.

## Importing Namespaces

### IMPORT. word

```forth
IMPORT. namespace-name ;
```

Brings a namespace into scope for unqualified access by pushing it onto the namespace stack.

**Example:**
```forth
NAMESPACE. mylib ;
: greet "Hello" . ;

NAMESPACE. core ;    -- switch to different namespace (core now on top)
mylib.greet          -- qualified access required

IMPORT. mylib ;      -- import mylib (inserts below core)
                     -- stack is now: [root, mylib, core]
greet                -- now works unqualified!
: test greet ;       -- new definitions still go to core (top)
```

**Implementation:** Inserts the namespace index below the current namespace on the stack.

**Note:** Imported namespaces shadow earlier namespaces. The most recently imported namespace wins for unqualified lookups.

## Creating Aliases

### ALIAS. word

```forth
ALIAS. new-name existing.qualified.name ;
```

Creates an alias (copy) of a word in the current namespace.

**Example:**
```forth
NAMESPACE. mylib ;
: greet "Hello from mylib" . ;

NAMESPACE. core ;
ALIAS. hello mylib.greet ;
hello                      -- calls mylib.greet
```

**Implementation:** Looks up the qualified name and inserts a copy into the current (top) namespace.

## Namespace Stack Visualization

```
Stack (top → bottom):
┌─────────────────┐
│  current ns     │  ← NAMESPACE. working here (top = where definitions go)
├─────────────────┤
│  imported lib   │  ← IMPORT. added this (below current)
├─────────────────┤
│  root ("")      │  ← always present
└─────────────────┘

Unqualified lookup walks: current ns → imported lib → root
Qualified lookup goes directly to the named namespace
```

**Key insight:** New word definitions always go to the **top** namespace. IMPORT. adds namespaces *below* the current one for lookup without changing where definitions go.

## Current Namespace

Words are always defined in the **top namespace** on the stack.
- `NAMESPACE.` pushes a namespace to the top (becomes current)
- `IMPORT.` inserts below the current namespace (adds to lookup path only)

## Scoping Rules

1. **Root namespace is always accessible** - Built-in words like `+`, `-`, `dup` are in root
2. **Qualified names bypass the stack** - Direct O(1) access to any namespace
3. **Imports create shadowing** - Most recent import wins for unqualified names
4. **Definitions go to top of stack** - New words go in the current namespace

## Examples

### Simple Library

```forth
-- Define a library
NAMESPACE. math ;
SIGNATURE. i64 i64 -> i64 ;
: square dup * ;
: cube dup dup * * ;

-- Use it with qualified names
5 math.square .              -- prints: 25
3 math.cube .                -- prints: 27

-- Or import it
IMPORT. math ;
5 square .                   -- prints: 25
```

### Avoiding Conflicts

```forth
-- Two libraries with same word name
NAMESPACE. lib1 ;
: process "Processing in lib1" . ;

NAMESPACE. lib2 ;
: process "Processing in lib2" . ;

-- Qualified access is unambiguous
lib1.process    -- prints: Processing in lib1
lib2.process    -- prints: Processing in lib2

-- Import controls which one is used unqualified
IMPORT. lib1 ;
process         -- prints: Processing in lib1

IMPORT. lib2 ;
process         -- prints: Processing in lib2 (shadows lib1)
```

### Nested Namespaces

```forth
NAMESPACE. io.net ;
: connect "Connecting..." . ;

NAMESPACE. io.file ;
: open "Opening file..." . ;

-- Fully qualified access
io.net.connect
io.file.open

-- Import specific namespace
IMPORT. io.net ;
connect         -- works
open            -- error: not in scope
```

## Implementation Details

### Data Structures

```rust
pub struct Forth {
    pub namespaces: Vec<HashMap<String, Word>>,  // All namespaces indexed
    pub namespace_stack: Vec<usize>,             // Stack of indices
    pub namespace_names: Vec<String>,            // Parallel to namespaces
    // ...
}
```

- `namespaces` - Vector of all namespace HashMaps (indexed storage)
- `namespace_stack` - Stack of indices into `namespaces` vector
- `namespace_names` - Parallel to `namespaces` for name lookup

**Key insight:** Only indices are pushed/popped on the stack. The actual HashMaps stay in place, avoiding expensive clones.

### Lookup Algorithm

**Qualified name** (`io.net.request`):
1. Split on rightmost `.` → `("io.net", "request")`
2. Find index of "io.net" in `namespace_names`
3. Look up "request" in `namespaces[index]`

**Unqualified name** (`request`):
1. `for &ns_idx in namespace_stack.iter().rev()`
2. Look up in `namespaces[ns_idx]`
3. Return first match

### Edge Cases

- **Empty namespace name** - The root namespace has name ""
- **Single dot** - The word `.` (print) is not treated as qualified because it's length 1
- **Multiple dots** - Only the rightmost `.` is used for splitting

## Best Practices

1. **Use qualified names for clarity** - Makes dependencies explicit
2. **Import sparingly** - Too many imports create ambiguity
3. **Namespace naming** - Use hierarchical names for organization (io.net, io.file)
4. **One namespace per library** - Group related functionality
5. **Document public interface** - Make clear which words are meant to be used externally

## Future Enhancements

- **Namespace metadata** - Version, author, documentation
- **Private words** - Words not exported from namespace
- **Selective import** - Import specific words only
- **Namespace aliases** - Short names for long namespace paths
- **Package system** - Load namespaces from files/database
