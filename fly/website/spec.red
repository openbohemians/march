h1. Fly Language Specification, v0.2

Thomas Sawyer, 2005-11-29


h2. Introduction

Fly is not a new programming language. 

It is very unique in it design. It does however borrow a great many fresh ideas from the present height of agile programming languages. Fly is a "post-agile" language, returning to a more traditional lanaguage construction yet with lessons learned. Its aim is not to gain market share against the likes of Perl, Python, Ruby, Lua, Io, VisualBasic and a litany of other similiar very high-level languages (although it will becasue it compares to them). But more importantly Fly seaks to compete with the beheamoth itself, C --a throne not even the venerable Lisp could stand against or even the reared-for-the-purpose Ada was able to undo. So it may seem a fool heary goal, but once you understand Fly, once you see how it handles, how it combines old an new into a new synthesis, I think it will become clear how it will be able to become a mainstream language.

Fly is based on the functionality of Forth, but structured as a prototype object-oriented language with a syntax structure inspired by YAML. This turns out to be an increadably powerful combination. Forth has always been known for its agility, like LISP, but it's highly terse syntax and reverse notation proved too obtuse, even more so than Lisp's hyper-parenthetics, which largely inhibited its later-day adoption. A YAML-like syntax goes a long way toward correcting this issue by breaking up the code into smaller, more visually managable chunks. Combined with object-oriented features and more natural-language labels, Fly's syntax is remarakable for its elegant terseness, as it is for its distinctiveness.


h2. Hello World

Everyone wants to see the traditional example right up front to get a quick "picture" of lanagues syntax. For Fly/Stacks that program
can be written like this:

  main:
    "Hello, World!" console.print ;


h2. System Structure

The core lib is implemented in pure assemby language and is organized by platform architecture for maximum speed and compatability. Adding support for another platform is just a matter of coding the core function set. In this early specification the achitectures are represented solely on the terms of the CPU --i386, i686, PPC, etc. In the future this will likely take on more a specific targeting structure to include a greater variety of reference platforms, including individual components like GPUs. The _ultimate_ goal is to have Fly itself provide the hardware abstraction layer in the form of small tools (i.e. functions). This is not something to be accomplished overnight though, obviously. In the mean time Fly can bind to other libraries for needed capabilities.

The next layer up, the standard lib, is built on top of the core. So this code is pure "low-level" Fly code. Altough per architecture assembly optimizations might occassionaly be made at this level.

The site/ directory stores 3rd party, very-high level libraries. While these too can have architecture specific optimizations, they are stongely discourged --they should only be required for bindings to external libs until such time as Fly versions of such libs can be made.

  /fly
    /core
      /i386
      /i686
      /ppc
    /std
      /i386
      /i686
      /ppc
      ...no-arch...
    /site
      user-defined/
        /i386
        /i686
        /ppc
        ...no-arch...

h2. Precluded Top-level Mixins

The 'core' module is merged directly into the top-level which provides all the core functionality.
Other core modules exist at the top-level:

  system
  console


h2. Stack Based

Stack is based on concepts espoused by Forth, and like from it rests on the concept of the Stack.
A stack is fundamental data structure of computer programming.

Unlike Forth, however, stacks in S+ are not one-dimensional and accessable only from one side. Rather
stacks are indexable. This is a key concept in S+. By specifying an index name anywhere with-in the
stack, it become a referencable point. The helloworld.sx program gives a good example of this.

  main:
    "Hello, World!" console.print ;


Here, 'main' is defining an indexable point in the stack. And 'console' actually references a point in
the stack, that is load as part of the standard library.


h2. Types

Fly has two kinds of data types. System types and high-level general types.
Generally a Fly programmer will use only the high-level types. These are:

  boolean
  Boolean flag is a two state data element, refered to as @true@ or @false@.

  null
  Null datatype represents an empty entry. A null object is often used as 
  a default initial value of a variable. End programmer sometimes use
  if to mean "undecided".

  number (or ratio)
  Number is also be referred to as ratio becuase Fly stores high-level 
  numbers as ratios. These numbers are capable of being postive or
  negative as well as complex.

  string
  A sequence of encoded data. By default stack encoded strings as UTF-8.

  stack
  A sequence of other objects and/or stacks.

High-level types may also be referenced by psuedo-types. These are types
that refer to a regular type in a certain limited state.

  integer    This is a ratio (number) with a denominator of 1.
  true       Boolean type in true state.
  false      Boolean type in false state.

Low-level data types correspond to raw data constructs recognized at
the hardware level.

  int+1   int2    bit
  int+2   int4    nibble
  int+3   int8    byte
  int+4   int16
  int+5   int32
  int+6   int64
  int+7   int128
  int+8   int256
  int+9   int512
    etc...

These larger types are ultimately not native to a particular 
machine, in which case they are simply treated as combinations
of a smaller type. Likewise some fo the smallest types, may have
to be a specialized masking of a larger one.

  float+3  float8
  float+4  float16
  float+5  float32
  float+6  float64
  float+7  float128
  float+8  float256
  float+9  float512
    etc...

[NOTE: Do not like the "+" but haven't figured out a better notation yet.]


h2. Keywords

Some special objects have special names. These are:

  true        A boolean flag in the true state.
  false       A boolean flag in the false state.
  infinity    A ratio with 0 denominator (numerator initializes to 1).
  undefined   A ratio with both a 0 denominator and a 0 numerator.


h2. Apply Functions Elementwise

A arbitray stack can have its elements used as parameters to a function using the elementwise word, each (#). This works by applying the elements in sequence to the element prior. For instance

  [ 1 2 3 ] each +

will add 3 2 1 for a sum of 6 and leave it on the top of the stack.


h2. Core Library (Primatives)

Fly's primatives consist of a few core sets of functionality. The most essential of these are the assembly
functions. These are defined on a per-architecture basis, providing a mid-level interface to the lover-level
machine code.

A the mid-level lies a few important core libraries, such as math, io, file, and console. Some of these
libraries tie back to bindings of traditional C code (until such time that the C library can be re-written
in Stacks).


h2. Standard Libraries

Built on top of the core libraries are a set of reusable libraries, loadable on demand. Some these will
not be ready for use for a while yet, but the inteded set (so far):

* OpenGL - 3D Grpahics Library
* OpenAL - Audio Library
* FreeType - Font Library

h2. Thrid Party Libraries

With the Core and Standard libraries in place, the sky's the limit.


h2. An Example

  fish:
    type: Rock
    length: 8 in.
    string:
      "%(type) Fish %(length)."

  fisherman:
    catch: []
    castnet:
      catch fish push
    showoff:
      catch each console.print

  main:
    3 [ fisherman.castnet ] repeat
    fisherman.showoff

_produces_

  Rock Fish 8 in.
  Rock Fish 8 in.
  Rock Fish 8 in.


