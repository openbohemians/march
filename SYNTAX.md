# SYNTAX THOUGHTS

#

Are types names the same a namespaces? Or do they *need* to be separate?

```
version: 0 1 0 ;   -- constant

< string -- > ==       -- put type (vec of types) on comptime stack, make part of current type signature.
[ informal? ] ??         -- put condition block on comptime stack, make part of current context.

hello   [ "Hi" print ].   -- runtime word definition
hye-bye [ "Bye" Print ].
thanks  [ "Thanks print" ].

[ formal? ] ??
hello   : "Hello" print ;
bye-bye : "Good Bye" print ;
thanks  : "Thank you" print ;

:=: string -- ;
:?: informal? ;
: age $ 12 ;
: version < 1 0 0 > ;
: hello ( "Hello" print ) ;





```

## Bare

```
# mylib
< yourlib
> hello
? informal?
=   string ->
:     hello   [ "Hi" print ]
:     bye-bye [ "Bye" print ]
:     thanks  [ "Thanks" print ]
? formal?
=   string ->
:     hello   [ "Hello" print ]
:     bye-bye [ "Good Bye" print ]
:     thanks  [ "Thank you" print ]

# mylib!
$ age 0
=   ->
:     now do-now
```


## COBOL Like

```
PROGRM. mylib
IMPORT. yourlib
EXPORT. hello

RUNTIME.

  CONTEXT. informal? ;

  TYPE. string -> ;
  DEFINE. hello   [ "Hi" print ] ;
  DEFINE. bye-bye [ "Bye" print ] ;
  DEFINE. thanks  [ "Thanks" print ] ;

  CONTEXT. formal? ;

  TYPE. string -> ;
  DEFINE. hello   [ "Hello" print ] ;
  DEFINE. bye-bye [ "Good Bye" print ] ;
  DEEFNE. thanks  [ "Thank you" print ] ;
  
COMPTIME.

  TYPE. ;
  DEFINE. now ... ;
```

Would this approach also allow grouping?

```
PROGRAM. mylib

IMPORT. yourlib ;
EXPORT. hello ;

RUNTIME.

  CONTEXT. informal? ;

  TYPE. string -> ;
  DEFINE. 
    hello [ "Hi" print ] ;
    bye-bye [ "Bye" print ] ;
    thanks [ "Thanks" print ] ;

  CONTEXT. formal? ;

  TYPE. string -> ;
  DEFINE.
    hello [ "Hello" print ] ;
    bye-bye [ "Good Bye" print ] ;
    thanks [ "Thank you" print ] ;
  
COMPTIME.

  TYPE. ;
  DEFINE. 
    now ... ;
```

Indention is still optional, I think.

Lower case?

```
program. mylib
  import. yourlib
  export. hello

  context. informal? ;
    type. string -> ;
      define. hello [ "Hi" print ] ;
```


