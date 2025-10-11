# Namespace Design for March2

## Core Concept

Namespaces are first-class values that integrate with the existing multi-method dispatch system.

## Type and Value Extensions

```rust
enum Type {
    // ... existing types ...
    Namespace(String),  // Namespace type with name
}

enum Value {
    // ... existing values ...
    Namespace(String),  // Namespace value with name
}
```

## Namespace Dictionary Storage

The `Forth` struct needs a namespace dictionary:

```rust
struct Forth {
    dictionary: HashMap<String, Vec<(Signature, Vec<Word>)>>,
    namespaces: HashMap<String, HashMap<String, Vec<(Signature, Vec<Word>)>>>,
    // ... existing fields ...
}
```

## Syntax

### Define namespace
```forth
<> math ;  -- Creates namespace "math"
```

### Define word in namespace
```forth
= i64 i64 -> i64 ;
: math.div swap / ;  -- Fully qualified name
```

### Use namespace dynamically
```forth
math div  -- Push namespace, call div
-- Equivalent to: math.div
```

### Import namespace (optional future feature)
```forth
< math ;  -- Makes all math.* words available as short names
```

## Lookup Algorithm

When `eval_token(token)` is called:

1. **Check if TOS is a Namespace:**
   ```rust
   if let Some((Value::Namespace(ns_name), Type::Namespace(_))) = self.stack.peek() {
       // Pop namespace
       self.stack.pop();

       // Look up "ns_name.token" in main dictionary
       let qualified_name = format!("{}.{}", ns_name, token);
       // ... proceed with normal multi-method dispatch ...
   }
   ```

2. **Check for qualified name in token:**
   ```rust
   if token.contains('.') {
       // Direct qualified lookup
       // ... proceed with normal multi-method dispatch ...
   }
   ```

3. **Normal lookup:**
   - Try dictionary as usual
   - Parse as literal if not found

## Implementation Steps

1. Add `Namespace` variants to `Type` and `Value` enums
2. Add `namespaces` field to `Forth` struct
3. Implement `<>` namespace definition syntax
4. Modify tokenizer to handle `namespace.word` as single token
5. Modify `eval_token()` to check for Namespace on TOS
6. Add `Word::NamespaceRef(String)` for pushing namespace values
7. Test basic namespace functionality

## Future Extensions

### Import (< operator)
```forth
< math ;  -- Adds all math.* to import table
div  -- Looks up as math.div via import
```

### Alias
```forth
< mymath=math ;  -- Alias namespace
mymath div
```

### Transitive imports (maybe)
```forth
-- Should < math ; also import math's imports?
-- User mentioned Elixir limits to one level
```

## Key Design Decisions

1. **Single token for qualified names:** `math.div` is one token, not three
2. **Namespace as value:** Can push/pop namespaces, enabling dynamic dispatch
3. **Compile-time and runtime:** Qualified names resolved at compile time, dynamic namespace dispatch at runtime
4. **No special syntax required:** Leverage existing multi-method system
5. **Dictionary storage:** Fully qualified names in main dictionary for simplicity

## Example Usage

```forth
<> math ;

= i64 i64 -> i64 ;
: math.div swap / ;

= i64 i64 -> i64 ;
: math.add + ;

-- Direct qualified call
10 5 math.div .  -- prints 2

-- Dynamic namespace call
10 5 math div .  -- also prints 2

-- Store namespace in state
$ mynamespace = math ;
10 5 mynamespace div .  -- also prints 2
```

## Integration with Multi-Method Dispatch

The beauty of this design is that namespaced words are just normal words with dots in their names. The multi-method dispatch system doesn't need to know about namespaces at all!

The only special handling is in `eval_token()` for the dynamic dispatch case where a Namespace is on TOS.

## Questions to Resolve

1. Should `<>` create the namespace as a value and push it? Or just register it?
2. How to reference a namespace by name? Need `Word::NamespaceRef(String)`?
3. Does every namespace word need to be fully qualified internally, or do we maintain separate dictionaries?
