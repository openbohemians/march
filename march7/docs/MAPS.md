# Maps

Status: first slice, built 2026-10-05, the third stage of doc/design/TEXT.md
("maps keyed by any value, strings included").

## What a map is

A map is one cell: a handle to a persistent CHAMP map that the machine holds
in a region slot, as it holds arrays and strings. The map is merkle-champ's
`ChampMap` from cells to cells: its shape depends only on its contents, a
new version shares everything but the changed path, and it has a cached
SHA-256 identity.

**Strings are interned,** so equal text is always the same handle. That is
what lets a map key on strings while holding plain cells: a string key
hashes and compares as a cell, and `eq?` compares strings correctly. Making
a string looks its identity up first, and returns the existing handle if the
text exists; freeing a string removes it from the table.

## Literals

`{ … }` gathers pairs of a key and a value, as `( … )` gathers an array:

```
{ "port" 8080 "host" "example.org" }
{ 1 1.5 2 2.5 }
{ }
```

`{` marks the stack like `(`; `}` (primitive 56) pairs up the cells above the
mark, so the body may compute keys and values, loop and branch. An odd count
is a stack error.

## Words

| Word | Effect | Meaning |
|---|---|---|
| `at` | `( m k -- v )` | the value for a key; a memory error if absent |
| `put` | `( m k v -- m' )` | a new map with the key set; the old one is unchanged |
| `has?` | `( m k -- flag )` | whether a key is present |
| `remove` | `( m k -- m' )` | a new map without the key |
| `length` | `( m -- n )` | the number of entries |
| `keys`, `values` | `( m -- a )` | the keys, or the values, as an array |
| `same?` | `( a b -- flag )` | equal contents, whatever the insertion order |

- `at` and `length` are the array words; the machine looks at what the handle
  holds. `put` also replaces an array's element: `( 1 2 3 ) 1 99 put` is
  `( 1 99 3 )`.
- `put`, `has?`, `remove`, `keys` and `values` are primitives 57 to 61. In
  `seed/system.march` the word for `put` is `map-put`, since the system track
  keeps `put` for its working memory; `init` installs it as `put`.

## Types

The checker types a map by the kinds its keys and values agree on:
`231 + 5k + v`, where the key kind `k` is 0 unknown, 1 i64 or 2 string, and
the value kind `v` is 0 unknown, 1 i64, 2 f64 or 3 string. 246 is the empty
map. So:
- `{ "a" 1 }` is 242, a map from strings to i64.
- `{ "a" 1 "b" "c" }` is 241: string keys, values of mixed kinds.
- `at` gives a value its map's value type, so `{ 1 1.5 } 1 at 1.0 +` adds
  floats.
- `put` joins the kinds: putting into the empty map takes the key's and
  value's kinds, and putting a different kind makes that kind unknown.
- Where paths meet, an empty map meeting a typed map becomes the typed one,
  so a map built in a loop keeps its type. Arrays built in loops on the data
  stack now keep theirs the same way.
- `keys` and `values` give arrays of the kinds' types; an array of strings is
  type 247.

Arrays now run to rank 76 (types 3 to 230), leaving 231 to 251 for maps and
other kinds.

## Showing maps

The command line shows a map as March writes it, sorted by key:
`<1> { "host" 7 "port" 8080 }`. Keys and values show by their kinds; a
value of unknown kind shows as a raw cell.

## Not yet

- **Identities across sessions.** A map's identity hashes its cells, so a
  map with string, array or map values or keys has an identity that depends
  on handle numbers. `same?` is right within a session, where strings are
  interned; storing maps needs the kinds at `put` time, so the host can hash
  contents instead of handles.
- **Arrays and maps as keys** are compared by handle, not by contents, since
  only strings are interned.
- **Nested maps** work at run time (`m "inner" at "x" at`), but the outer
  map's value kind is unknown, so the inner map shows as a raw cell.
- **Iteration** over entries with `each`, and map literal names
  (`{ port: 8080 }`, docs/SURFACE.md).
