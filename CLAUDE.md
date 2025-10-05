# CLAUDE.md - AI Assistant Context

## Project Overview

**March** is a stack-based programming language that combines FORTH's simplicity with modern features like strong typing, contextual dispatch, and database-backed code storage.

### Key Innovations
- **Explicit Global State**: All program state is visible and declared up front
- **Contextual Dispatch**: Functions have multiple definitions based on state/type conditions
- **Content-Addressed Functions**: Immutable, shareable functions via cryptographic hashes
- **Database-Backed Storage**: Code lives in SQLite, not text files
- **Incremental Compilation**: Immediate compilation to IR as you write

## Current Development Phase

**Phase 1: Minimal Interpreter** (Current)
- Basic stack operations (+, -, *, /, dup, swap, drop)
- Word definitions (no contexts yet)
- Simple REPL
- SQLite storage for words

## Technology Stack

- **Implementation Language**: C
- **Database**: SQLite for code/metadata storage
- **Architecture**: Stack-based with postfix notation
- **Compilation**: Incremental to IR, eventual native compilation

## Key Design Principles

1. **Explicitness over cleverness**: Make behavior visible
2. **State as a feature**: Global state is intentional, not hidden
3. **Program = Module**: Reject excessive modularization
4. **Context-driven behavior**: Same word, different meanings based on context

## Development Workflow

### Current Status
- Project is in design/planning phase
- No source code implemented yet
- Database schema outlined in DESIGN.md
- Implementation strategy defined

### When Implementation Begins
- Start with basic stack operations in C
- Implement SQLite schema for word storage
- Build simple REPL for testing
- Focus on core stack semantics before adding contexts

## Database Schema (Phase 1 Priority)

Focus on these tables first:
- `words`: Basic word definitions
- `definitions`: Word implementations
- `signatures`: Stack input/output types

Leave context-related tables for Phase 2.

## Architecture Notes

### Stack Model
```
: compute ( x -- y )
  velocity @ * acceleration @ + ;
```

Really means:
```
: compute ( State x -- State y )
```

State threading is implicit for ergonomics, explicit conceptually.

### Error Handling Philosophy
Errors don't throw - they set error state. Subsequent calls dispatch to error contexts. This keeps error handling explicit and visible.

## AI Assistant Guidelines

### Helpful Focus Areas
1. **C Implementation**: Stack data structures, memory management
2. **SQLite Integration**: Schema implementation, queries for word lookup
3. **Parser Design**: Handling postfix notation and word definitions
4. **REPL Architecture**: Interactive interpreter loop

### Project Context to Remember
- This is NOT just another FORTH - the contextual dispatch system is novel
- State management is intentionally global and explicit
- Database storage is core to the design, not an add-on
- Incremental compilation is a key feature, not just optimization

### Avoid These Assumptions
- Don't assume traditional file-based code organization
- Don't suggest hiding state or making it implicit
- Don't recommend breaking into sub-modules early
- Don't suggest exception-based error handling

## Testing Strategy

### Phase 1 Testing
- Unit tests for stack operations
- REPL-based manual testing
- SQLite schema validation
- Basic word definition/lookup tests

### Future Testing
- Context dispatch correctness
- State constraint validation
- Error context handling
- Import/export with state mapping

## Success Metrics

Phase 1 is successful when:
- Basic arithmetic works on the stack
- Words can be defined and called
- REPL provides immediate feedback
- Word definitions persist in SQLite
- Stack manipulation feels natural

Long-term success when:
- Stateful code feels natural, not error-prone
- Context dispatch clarifies rather than confuses
- Error handling is straightforward
- Functions are easily shareable
- Real-time debugging is intuitive