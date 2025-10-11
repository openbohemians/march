# SYNTAX THOUGHTS

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
  DEFINE. hello [ "Hi" print ] ;
  DEFINE. bye-bye [ "Bye" print ] ;
  DEFINE. thanks [ "Thanks" print ] ;

  CONTEXT. formal? ;

  TYPE. string -> ;
  DEFINE. hello [ "Hello" print ] ;
  DEFINE. bye-bye [ "Good Bye" print ] ;
  DEEFNE. thanks [ "Thank you" print ] ;
  
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

```
program. mylib
  import. yourlib
  export. hello

  context. informal? ;
    type. string -> ;
      define. hello [ "Hi" print ] ;
```


