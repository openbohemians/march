# March 2

## Important Consideration

We considered a modal where everything that goes on the stack is a symbol or number (or collection object of such).
The `SYMBOL .` looks up the functon named `SYMBOL` to execute it, or `SYMBOL @` looks up the global variable value,
etc. The downside was of course noise. So `SYMBOL.` short-hand was suggested, which is okay, but looks kind of ugly
for other words that end in punctuation, e.g. `valid?.`, when you'd rather just have `valid?`. 

I tried to work around this but it just made things too complicated. The only reasonable concession was maybe
to allow 1-character punctuation symbols (or maybe if it is all punctuation symbols) to forego the dot.
Even so, I am willing to fallback to the simple fact of using `.` for all of it, IF IT IS WORTH IT. 

So it boils down to the question: Is it worth it?  Does this simple model of symbol and number (and collections)
make things better in any substantial way (implementation, excution, capabilities in the future, etc.)?
Or are we just as well, or better off, following a more taditional FORTH approach?

## More Traditional Alternatative

The alternative is a more traditional FORTH-like approach.

### Example

Here is a pseudo-example. This example is NOT using the symbol idea.
It is an example of the ALTERNATIVE approach, which is much more akin to traditional FORTH.

This example curently doesn't have `;` endings and instead uses indentation and
first line character "sigil-ops" to determine when to break, but that might be a bad idea.

```
<> demo 7r4n5.top

-- imports
> std.io

-- exports
< age name salutes inc-age rename set-title

-- state
$ age     = 42 i64 !
$ name    = "George"
$ salutes = [ "Mr." "Ms." "Dr." ]

= --                     -- no return
: inc-age
  age 1 + -> age         -- read via auto-deref; write via ->

: rename
  "Ada" -> name

: set-title
  "Mrs." salutes append

: show
  name println
  age  println
  salutes 1 @ println   -- "Ms." (before set-title), value via auto-deref

: main
  show
  inc-age rename set-title
  show
```

## Top-level Sigil-Ops

* `<>` Program Name (and domain name for namespace).
* `<`  Library imports.
* `>`  Words to export.
* `$`  Define state.
* `=`  Define type signature.
* `?`  Define context constraint. (not shown in example)
* `:`  Define word.

## Symbol Approach

Using symbols "all the way down" would look similar to the above example, but many
words would have a `.` after them, e.g. `println.`.


