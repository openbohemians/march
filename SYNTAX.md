# SYNTAX THOUGHTS

## Encased

```
[#] NAME
[<] mylib
[>] hello

[?] informal? ;

[=] string -> ;
[:] hello [ "Hi" print ] ;
[:] bye-bye [ "Bye" print ] ;
[:] thanks [ "Thanks" print ] ;

[?] formal? ;

[=] string -> ;
[:] hello [ "Hello" print ] ;
[:] bye-bye [ "Good Bye" print ] ;
[:] thanks [ "Thank you" print ] ;
```

## One-sided

```
#] NAME
<] mylib
>] hello

?] informal? ;

=] string -> ;
:] hello [ "Hi" print ] ;
:] bye-bye [ "Bye" print ] ;
:] thanks [ "Thanks" print ] ;

?] formal? ;

=] string -> ;
:] hello [ "Hello" print ] ;
:] bye-bye [ "Good Bye" print ] ;
:] thanks [ "Thank you" print ] ;
```

## Bare

```
# NAME
< mylib
> hello

? informal? ;

= string -> ;
: hello [ "Hi" print ] ;
: bye-bye [ "Bye" print ] ;
: thanks [ "Thanks" print ] ;

? formal? ;

= string -> ;
: hello [ "Hello" print ] ;
: bye-bye [ "Good Bye" print ] ;
: thanks [ "Thank you" print ] ;

= -> ;
!: now ... ;
```

## COBOL Like

```
PROGRM. NAME
IMPORT. mylib
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
PROGRM. NAME
IMPORT. mylib
EXPORT. hello

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

