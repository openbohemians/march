# Plans

## 2025-10-22 Computed Goto

1. **Stabilize current state**
   - Revert the experimental trampoline edits so the VM is back to the last-known-good direct-threaded assembly version.
   - Capture open questions from the trampoline attempt (return-stack bookkeeping, loader responsibilities) and park them in docs/IDEAS.md.

2. **Design the computed-goto interpreter**
   - Draft a C (or C-compatible) implementation of the inner interpreter that mirrors the sketch in `docs/IDEAS.md` (see “Computed GOTO”): tag table + per-word DOCOL helper using computed goto. Work-in-progress notes live in `docs/VM_COMPUTED_GOTO.md`.
   - Define how user words/quotations map to entry stubs or metadata structs and how primitives expose their labels.
   - Document the calling convention so the loader/compiler know what to emit.

3. **Prototype in isolation**
   - Build a standalone proof-of-concept VM in C using computed goto, fed by hard-coded cell streams, to validate the control flow and performance (see `experiments/vm_cgoto.c`).
   - Add minimal tests (e.g., arithmetic, nested quotations, EXIT behavior) to nail down semantics before integrating.

4. **Integrate with March runtime**
   - Decide whether the production VM will remain in assembly or move to C while GCC/Clang is required.
   - Update the loader so each CID-backed definition builds the data structures expected by the new interpreter (likely cell arrays + entry metadata instead of trampolines).
   - Ensure primitives and existing assembly glue (op_* routines) still bind correctly.

5. **Testing and migration**
   - Port the current test suite (test_loader, test_quotations, runner tests) to the new interpreter.
   - Measure interpreter throughput versus the previous design; note regressions or wins.
   - Gate the switch behind a build flag until both paths are stable.

6. **Longer-term follow-ups**
   - Plan how the computed-goto interpreter will be reimplemented once March self-hosts (e.g., JITting equivalent jump tables without relying on GCC extensions).
   - Evaluate whether future features (effects, dev-mode shadow stack) need additional hooks in the new dispatcher.






## 2025-10-16 March Redesign: Direct Threading + FORTH-style Outer Interpreter

** DO NOT ASSUME THE INCLUDED CODE SNIPPETS ARE CORRECT! ***

Core Changes
                      
## 1. New Cell Encoding (Direct Threaded)
                    
Current (Indirect/Subroutine):
XT → contains address → CALL address → primitive uses RET 
                    
New (Direct Threaded):
XT → contains address → JMP address → primitive jumps back to dispatcher

New 4-Tag Scheme:

    00  = XT  execution token         [address | 00]   Jump to word (address=0 means EXIT)
    01  = LIT immediate token         [value   | 01]   62-bit signed immediate value
    10  = LST symbol token            [value   | 10]   62-bit unsigned immediate symbol reference
    110 = LNT next N literals token   [value   | 110]  Next N tokens are literal numbers (N is 61-bit number)
    111 = EXT future extension        [value   | 11]   Extended operations TBD

Details:

- EXIT/RETURN is just XT(0). This saves a tag.
- LIT holds immediate values. So no extra cell are needed for small ints. Useful for loop counters, and true (-1) or false (0).
- If more token types are ever needed, EXT extension token can be used. Hopefully we don't need any others.

                 
## 2. Direct Threading Implementation
                      
A. Update VM (vm.asm)

*** THIS NEEDS TO BE FIXED (it uses older token table design) ***

Dispatch loop changes:
  
  .do_xt:
      and rcx, ~0x3           ; Extract address
      test rcx, rcx           ; Check for EXIT (address 0)
      jz .do_exit
                      
      ; Save current IP on return stack
      sub rdi, 8
      mov [rdi], rbx
                      
      ; Jump to word's code
      mov rbx, rcx            ; IP = word address
      jmp [rbx]               ; Jump to first cell of word
                      
  .do_exit:
      mov rbx, [rdi]          ; Pop return address
      add rdi, 8
      jmp .dispatch
                      
  .do_goto:
      and rcx, ~0x3           ; Extract address
      mov rbx, rcx            ; IP = target (no return save)
      jmp [rbx]
                      
  .do_lit:
      mov rax, rcx
      sar rax, 2              ; Sign-extend from 62 bits
      sub rsi, 8
      mov [rsi], rax
      jmp .dispatch
                      
  B. Update All Primitives (40 files)
                      
  Current ending:
  op_dup:
      mov rax, [rsi]
      sub rsi, 8
      mov [rsi], rax
      ret                     ; ← REMOVE 
                      
  New ending:
  op_dup:
      mov rax, [rsi]
      sub rsi, 8
      mov [rsi], rax
                      
      ; Direct threaded dispatch
      mov rcx, [rbx]          ; Fetch next cell
      add rbx, 8              ; Advance IP
      mov rax, rcx
      and rax, 0x3            ; Extract tag
                      
      ; Jump to handler (via jump table or chain)
      jmp [dispatch_table + rax*8]
                      
  Or simpler - just jump back:
      jmp vm_dispatch         ; Back to main loop
                      
                
## 3. FORTH-Style Outer Interpreter
                      
Replace Lexer/Parser/AST Model
                      
Remove:
* lexer.ml - Token-based lexer
* parser.ml - AST builder
* AST data structures
                      
Replace with Word Stream Processor:
                      
  (* New: interpreter.ml *)
                      
  type interpreter_state = {
    input: string;               (* Input buffer *)
    pos: int;                    (* Current position *)
    compiling: bool;             (* Compile mode vs interpret mode *)
    current_def: cell list;      (* Cells being compiled *)
    current_name: string option; (* Name of word being defined, option for quotation/thunks *)
    dictionary: (string, word_entry) Hashtbl.t;
  }

  (* Read next whitespace-delimited word *)
  let read_word state =
    (* Skip whitespace *)
    (* Read until next whitespace *)
    (* Return (word, new_state) or None *)
                      
  (* Main outer interpreter loop *)
  let rec outer_interpreter state =
    match read_word state with
    | None -> state  (* Done *)
    | Some (word, state') ->
        (* Look up word in dictionary *)      
        match lookup_dictionary word state'.dictionary with           
        | Some entry when entry.is_immediate ->                       
            (* IMMEDIATE word - execute even in compile mode *)       
            execute_immediate entry state'    
        | Some entry when state'.compiling -> 
            (* Compile mode - add word to definition *)               
            compile_word_reference entry state'                       
        | Some entry ->                       
            (* Interpret mode - execute word *)                       
            execute_word entry state'         
        | None ->     
            (* Not in dictionary - try parsing as number *)           
            match parse_number word with      
            | Some n when state'.compiling -> 
                compile_literal n state'      
            | Some n ->                       
                push_literal n state'         
            | None -> 
                error ("Unknown word: " ^ word)                       

Consideration: Should the word be checked to see if it is a number first? Currently, this design allows a number to be redefined as a word, e.g. `1` and it would no longer be a number. Checking if a word is a number could be as simple as checking if it starts with /[0-9]/ or /-[0-9]/.

### Parsing Words

*Parsing words* are immediate words that interceed the token stream and rearrange and or modify it.

### Immediate Words for Definition

The `:` word is a parsing word. That takes the first token from the stream and turns it into a symbol token (otherwise known as a type literal), 
then it expects a thunk/quotation. The adds the `define-word` word at the end.

    : foo [ do-foo-stuff ]
    
becomes

    <foo> [ ... ] define-word
    
where `<foo>` is just a representation that means symbol (not actual syntax).

There is a decision to be made here. Do we require the thunk/quotation to explict, or should it be implict? (Or can it do both?)

1. Explicit

    : foo [ ... ]
    
Or using Logo-style alternative (both will be supported):

    to foo do ... end

2. Implicit 

    : foo ... ;
    
Or using Logo-style word:

    to foo ... end

In the explicit case the `:` parsing word only needs to *expect* a thunk/quotation. 
In the implicit case it must create the token/quotation, stopping at `;` or `end`.

The tricky part about explicit is how does the parsing word handle the expectation of a thunk/quotation?

So the follwing original code sketch is *NOT CORRECT*:

  (* : word - enter compile mode *)
  let colon_word state =
    let (name, state') = read_word state in
    { state' with
      compiling = true;
      current_name = Some name;
      current_def = [];
    }

  (* ; word - exit compile mode, store definition *)
  let semicolon_word state =
    let name = Option.get state.current_name in
    let cells = List.rev (XT(0) :: state.current_def) in  (* Add EXIT *)
    let cid = compute_cid cells in
    store_definition name cid cells state.dictionary;
    { state with
      compiling = false;
      current_name = None;
      current_def = [];
    }
                 
  (* Register immediate words *)
  let init_dictionary () =
    let dict = Hashtbl.create 100 in
    register_immediate ":" colon_word dict;
    register_immediate ";" semicolon_word dict;
    (* ... register primitives ... *)
    dict

*What to do instead*:

In fact March should have a basic set of immediate parsing words to help write
other parsing words in March itself. Then `:` itself would be something like:

    march.ts.make-symbol    -- pulls word token from stream makes it a symbol
    march.ts.is-quote?      -- checks for a thunk/quotatation  (but how?)
    if (* quote-found *)
      march.ts.expect-quote  -- expects quote
    else
      "[" march.ts.emit          -- build quote
      ";" march.ts.emit-until
      "]" march.ts.emit
    end
    march.def-word (* or, `march.def-word" march.ts.emit` ? *)

(Note I have added some namespaces for clairty. `ts` means "token stream".)

The idea is that the parsing word essentially just re-arranges and modifies the incoming stream.

*Question*: Do we allow the parsing words to handle the internal details, like invoking `def-word`.
Or should parsing words only be allowed to modify the token stream, then processing continues as normal?


## 4. Implementation Order
                    
Phase 1: Update Cell Encoding
                    
1. Update types.ml - new tag constants
2. Update vm.asm - new dispatch for new token table XT/LIT/LST/LNT/EXT
3. Update all 40 primitives - remove RET, add dispatch jump
4. Update codegen.ml - emit new cell format 
5. Test: Ensure existing compiled code still works

Files: types.ml, vm.asm, kernel/x86-64/*.asm, codegen.ml
Time: 2-3 hours
                    
Phase 2: Direct Threading:
                    
1. Modify primitive endings to jump to dispatcher
2. Update VM dispatch loop for direct jumps 
3. Remove call/ret infrastructure
4. Benchmark performance improvement
     
Files: vm.asm, kernel/x86-64/*.asm
Time: 1-2 hours
                    
Phase 3: FORTH Outer Interpreter
                    
1. Create interpreter.ml - word stream processor
2. Implement read_word, outer_interpreter
3. Implement immediate words (:, ;)
4. Remove lexer.ml, parser.ml
5. Update main.ml to use new interpreter
                    
Files: interpreter.ml, main.ml (remove lexer.ml, parser.ml)
Time: 3-4 hours
                    
Phase 4: Testing & Integration

1. Test basic definitions
2. Test nested definitions
3. Test immediate words
4. Update test suite
5. Update documentation

Time: 1-2 hours
                    
                 
## Total Effort
                    
Estimated time: 8-12 hours of focused work
                    
Benefits:
- ✅ Faster execution (direct threading)
- ✅ More compact (62-bit LIT, EXIT=0)
- ✅ True FORTH architecture (outer/inner interpreter)
- ✅ Foundation for self-hosting (March compiler in March)
- ✅ Simpler model (word stream rather than AST)


## Considerations

Code is ultimate stored the the database -- but there it is pre-compiled.
Loading a program from the database does not require the parsing/compile stage.
The pre-compiled code needs only to be linked together (convert CIDs to XTs) then executed.

## Testing Strategy

Phase 1 tests:

: five 5 ;              -- LIT immediate
: ten 10 ;
: fifteen five ten + ;  -- Multiple XTs
                    
Phase 2 tests:

: square dup * ;     -- Stack effects
: double dup + ;
5 square .           -- Should print 25
                    
Phase 3 tests:

: immediate-test [ ... ] ;  -- Immediate words?
: nested : inner ; ;        -- Nested definitions?

Ready to implement this comprehensive redesign?
