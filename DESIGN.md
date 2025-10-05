# Language Design Document

## Project Name: [TBD]

A stack-based programming language with explicit state management, contextual dispatch, and content-addressed functions.

---

## Core Philosophy

### State as a Feature
Most modern languages hide or minimize state. We embrace it. All state is global and visible at the program level. This isn't carelessness—it's intentional transparency. By making state explicit and centralized, we make programs easier to understand and debug in real-time.

### Program = Module
We reject excessive modularization. A program IS a module. Namespaces exist for organization, but state boundaries don't fragment across sub-modules. If you need separate concerns, write separate programs and compose them.

### Context-Driven Behavior
Functions don't just have one implementation. They have multiple definitions that exist in different contexts based on state conditions and type conditions. The same word means different things depending on the context it's called in.

---

## Language Foundations

### Stack-Based (FORTH Heritage)
- Postfix notation
- Stack manipulation as primary operation model
- Simple, direct semantics
- Words (functions) operate on the stack

### Database-Backed Code Storage
Code is not stored in text files. It lives in a structured database:
- Words are entities with properties
- Definitions are versioned and contextual
- Metadata (types, effects, contexts) live alongside code
- Query your codebase like data

### Incremental Compilation (ColorForth Style)
No build step. Functions compile to IR as you write them:
- Define a word → immediately compiled
- Edit a word → only that word recompiles
- Interpreter runs pre-compiled IR
- Fast iteration, instant feedback

---

## Key Concepts

### 1. Explicit Global State

All program state is global and declared up front:

```
State: velocity : Integer where velocity >= 0
State: mode : Symbol  
State: error : ErrorType | None
```

State variables:
- Have types
- Can have constraints (compile-time or runtime)
- Are visible throughout the program
- Can be viewed in real-time during execution

**Conceptual Model**: Every function implicitly receives and returns the entire program state. It's like pure functional programming, but the state threading is implicit for ergonomics:

```
: compute ( x -- y )
  velocity @ * acceleration @ + ;
```

Really means:

```
: compute ( State x -- State y )
  ...
```

This makes functions actually pure (same state + inputs = same outputs) while keeping syntax clean.

### 2. Contextual Dispatch

Functions can have multiple definitions based on conditions:

```
State: mode = :running

Context mode = :running
: update 
  "Running update logic" . 
  advance-simulation ;

Context mode = :paused  
: update
  "Paused, skipping" . ;
```

Calling `update` dispatches to the appropriate definition based on current state.

**Context Types**:

**Static Contexts** - Resolved at compile time:
```
Context TypeOf(x) = Integer
: process ...integer logic... ;

Context TypeOf(x) = String
: process ...string logic... ;
```

**Dynamic Contexts** - Resolved at runtime:
```
Context speed > 100
: alert "Warning: high speed!" . ;

Context speed <= 100  
: alert ( do nothing ) ;
```

**Combined Contexts**:
```
Context TypeOf(data) = Array, size > 1000
: process-batch ...optimized for large arrays... ;
```

**Ambiguity Handling**: If multiple contexts match, it's a compile-time error. Be explicit.

**Context Layers**: More specific contexts override general ones (important for error handling).

### 3. Error Handling as Context

Errors are just state. Error handling uses the context system:

```
State: error : ErrorType | None

Context error = IoError
: read-config
  "Config read failed, using defaults" .
  clear-error
  default-config ;

Context error = NetworkError
: fetch-data
  "Network error, retrying..." .
  clear-error  
  retry-with-backoff ;

Context error = None
: read-config
  ...normal operation... ;
```

**Error Layers** (from specific to general):

1. **Function-specific handlers** - Most specific, handles error for that word
2. **Application-level handlers** - General error handling across functions  
3. **Base runtime handlers** - Always present, typically abort

Errors don't throw/unwind the stack. They set error state, and subsequent calls dispatch to error contexts. Explicit, visible, manageable.

### 4. Content-Addressed Functions

Functions are immutable and shareable via cryptographic hashes:

**Metadata for Sharing**:
```json
{
  "hash": "sha256:abc123...",
  "name": "compute",
  "signature": "( x -- y )",
  "state_effects": {
    "reads": ["velocity", "acceleration"],
    "writes": [],
    "types": {
      "velocity": "Integer",
      "acceleration": "Integer"  
    },
    "constraints": {
      "velocity": ">= 0",
      "acceleration": "any"
    }
  },
  "contexts": [...],
  "body_ir": "..."
}
```

**Importing with Context-Based Mapping**:

```
import compute from "sha256:abc123..."

Context "compute import"
: statemap
  velocity -> mySpeed
  acceleration -> myAccel ;
```

The import context activates during linking, mapping imported state requirements to your program's state.

---

## Implementation Strategy

### Phase 1: Minimal Interpreter
- Basic stack operations (+, -, *, /, dup, swap, drop)
- Word definitions (no contexts yet)
- Simple REPL
- SQLite storage for words

### Phase 2: State & Contexts  
- State variable declarations
- Type system basics
- Static context resolution
- Dynamic context dispatch

### Phase 3: Database & Tooling
- Full database schema
- Web-based editor/IDE
- Real-time state viewing
- Incremental compilation to IR

### Phase 4: Advanced Features
- Content-addressed sharing
- Import/export with mappings
- Error context system
- Optimization passes

### Phase 5: Compiler
- IR to native compilation
- Whole-program optimization
- Context specialization
- Performance tuning

---

## Database Schema (Preliminary)

### Tables

**state_variables**
- name (unique)
- type
- constraints
- initial_value

**words**
- id
- name  
- namespace

**definitions**
- id
- word_id (fk)
- context_conditions
- body_source
- body_ir
- content_hash
- created_at

**state_effects**
- definition_id (fk)
- variable_name
- access_type (read/write)

**contexts**
- definition_id (fk)
- condition_expression
- is_static (boolean)

**signatures**
- definition_id (fk)
- stack_in
- stack_out

---

## Open Questions & Future Considerations

1. **Context composition**: How do imported contexts interact with local contexts?

2. **Optimization boundaries**: How much can we optimize with explicit state vs. how much do we sacrifice for visibility?

3. **Debugging**: What does a debugger look like when context determines behavior?

4. **Parallelism**: How does explicit global state interact with concurrent execution?

5. **Type system depth**: How far do we go with dependent types and refinement types?

6. **Macro system**: Do we need/want compile-time metaprogramming?

7. **FFI**: How do we call external libraries that don't understand our state model?

---

## Design Principles to Maintain

- **Explicitness over cleverness**: Make behavior visible
- **Simplicity over features**: Only add what's needed
- **Consistency**: Use the same mechanisms (contexts) for multiple concerns
- **Incrementality**: Build, test, use immediately
- **Practicality**: Real-world usability over theoretical purity

---

## Success Metrics

We'll know this is working when:
- Writing stateful code feels natural, not error-prone
- Context dispatch makes code clearer, not more confusing  
- Error handling is straightforward
- Functions are easily shareable and reusable
- Real-time debugging is powerful and intuitive
- Programs are easier to understand than in traditional languages

---

**This is a living document. Update as the design evolves through implementation.**

