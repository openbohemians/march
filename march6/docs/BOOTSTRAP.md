# FORTH-style self-extension and context-driven compilation

This is a language goal, not a claim that `march-fast` is self-hosted today.
The original historical study and B0 gate details are preserved at
`6f17b9b:march6/BOOTSTRAP.md`; see [documentation recovery](README.md#retired-research-and-recovery).

## The principle that survives the backend change

March's FORTH inheritance is more than postfix notation. The intended system
has a small native nucleus, a dictionary of words, and a compiler/interpreter
that can itself be extended using words. Syntax is a representation of stored
code; names are dictionary bindings, not part of code identity.

Content-addressed immutable code does not require an interaction-net runtime.
Likewise, compilation and execution may share language-level semantics without
using the same implementation or interning every execution temporary.

## Why contexts and bootstrapping fit together

An explicit compiler-state value can contain the dictionary root, input cursor,
mode, symbolic stack, and declarations applying to subsequent definitions.
Reading a token then maps one state to the next. A guarded word can behave
differently in construction and execution contexts: FORTH's immediate-word
behavior becomes ordinary context-selected behavior rather than hidden global
compiler state.

The intended result is that parsing/definition words such as `:` and `;` live
above a syntax-neutral nucleus. Alternative source surfaces can construct the
same canonical code. Building the same seed plus source twice should produce
the same code/image identity. These are goals to demonstrate, not grounds for
forcing the old graph reducer's representation onto the fast runtime.

## What exists, and where

The retained CAS reference implementation demonstrated graph-defined syntax
handlers, source-defined parsing aliases, two surfaces producing the same
square quotation, and tested save/reload boundaries. Its contracts remain in
the [seed](reference/SEED.md), [reader](reference/READER.md), and
[reflection](reference/REFLECTION.md) notes. Those are useful witnesses, not
proof of a complete source bootstrap.

The conventional engine instead currently has a **host-side source compiler**,
closed quotations with explicit stack inputs, ordered guarded families,
canonical code images, and a provisional FORTH surface. It has not inherited
the reference's reflection interface, graph-defined compiler, partial-context
residualization, or snapshots of suspended evaluation. Its images save code and
dictionary roots, not a live development session. See [the guide](FAST-SPIKE.md).

## Work still needed

- Establish the missing-context/staging and sharing contracts before promising
  an incremental compiler or live image on the new engine.
- Supply the data, dictionary, token, and validated code-construction operations
  needed by compiler words; keep syntax policy above those primitives.
- Express compiler modes and definition groups through the contextual model,
  then demonstrate a parsing word defined and used in the same session.
- Test source-to-code identity across alternative surfaces and image reloads.
- Eventually write the compiler handlers in March and test a reproducible
  bootstrap fixed point. This is not the current implementation checkpoint.

Implicit lexical captures, final surface spellings, lossless formatting, effect
handling, and remote CID lookup remain separate design questions. None requires
reviving interaction-net execution or promising perfect static memory planning.
