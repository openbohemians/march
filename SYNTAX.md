# SYNTAX THOUGHTS

## Original Syntax

```
= string -- ;

? informal? ;
: hello   "Hi" print ;           -- define a runtime word, via constant thunk.
: bye-bye "Bye" Print ;
: thanks  "Thanks print" ;

? formal? ;
: hello   "Hello" print ;
: bye-bye "Good Bye" print ;
: thanks  "Thank you" print ;
```

Question: Are types names the same a namespaces? Or do they *need* to be separate?

## Thunks Required?

```
# mylib
< yourlib		  -- import
> hello       -- export

? informal? ;
  = string -> ;
  : hello   ( "Hi" print ] ;
  : bye-bye ( "Bye" print ) ;
  : thanks  ( "Thanks" print ] ;

? formal? ;
  = string -> ;
  : hello   ( "Hello" print ) ;
  : bye-bye ( "Good Bye" print ) ;
  : thanks  ( "Thank you" print ) ;

# mylib!   -- compile time code?

$ age = 0 ;
  = -> ;
  : now do-now
```

## COBOL-Like

Then it was decide that since code will end up in a database and way, and the text format is really a serialization format,
albeit importantly human readable and easy to parse, we came up with a COBOL like syntax:

```
PROGRM. mylib
VERSION. < 0 2 0 >
IMPORT. yourlib ;
EXPORT. hello ;

RUNTIME.
  CASE. informal? ;
    TYPE. string -> ;
      DEFINE. hello   "Hi" print ;
      DEFINE. bye-bye "Bye" print ;
      DEFINE. thanks  "Thanks" print ;

  CASE. formal? ;
    TYPE. string -> ;
    DEFINE. hello   "Hello" print ;
    DEFINE. bye-bye "Good Bye" print ;
    DEFINE. thanks  "Thank you" print ;

COMTIME.
  TYPEDEF. string [char] ;

  TYPE. string -> string ;
  DEFINE. now ... ;
```

A more advanced version would allow grouping.

```
PROGRAM. mylib

IMPORT. yourlib ;
EXPORT. hello ;

RUNTIME.
  CONTEXT. informal? ;
    SIGNATURE. string -> ;
    FUNCTIONS. 
      hello   ( "Hi" print ) ;
      bye-bye ( "Bye" print ) ;
      thanks  ( "Thanks" print ) ;
  CONTEXT. formal? ;
    SIGNATURE. string -> ;
    FUNCTIONS.
      hello   ( "Hello" print ) ;
      bye-bye ( "Good Bye" print ) ;
      thanks  ( "Thank you" print ) ;
COMTIME.
    SIGNATURE. ;
    DEFINE. 
      now ... ;
```

Lower case?

```
program. mylib
  import. yourlib
  export. hello

  context. informal? ;
    type. string -> ;
      DEFINE. hello [ "Hi" print ] ;
```

## Nascent Refs on Stack

This was a more advanced consideration we almost adopted.

When a word is encountered that is not yet defined, a nascent ref goes on the stack to
be used by defining words like `:` and `=` or whatever we want.

```
age : 12 ;

< string -- > !                  -- put type vector on comptime stack, make current type signature.

( informal? ) ?                  -- put *context quotation* on stack, make part of current context.
hello   : "Hi" print ;           -- define a runtime word, via constant thunk.
hye-bye : "Bye" Print ;
thanks  : "Thanks print" ;

( formal? ) ?
hello   : "Hello" print ;
bye-bye : "Good Bye" print ;
thanks  : ( "Thank you" print ) ! ;
```


