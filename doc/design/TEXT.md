# Text

Status: design note, 2026-10-04. Nothing here is built. It records what
strings are in March, decided with Thomas before any code. It also records
a longer road toward glyphs that anyone can define, use and share, which
Unicode does not offer.

## 1. Decisions so far

- **UTF-8 throughout. No UTF-32 step.**
  - UTF-32's only advantage is finding the n-th code point in one step.
  - Text code rarely needs that, and it is not even the n-th letter a
    reader sees: "é" may be an e and a combining accent, two code points.
  - Walking, searching, slicing at positions a search found, joining,
    comparing, hashing, input and output all work as well on UTF-8 with
    byte positions. Rust and Go store strings this way.
  - Building UTF-32 first would be thrown away (Thomas: "why spend the time
    on UTF-32 if we are just going to toss it out").
- **No separate ASCII type.** Every ASCII string is valid UTF-8, byte for
  byte. ASCII text gets a fast path instead: a string records whether every
  byte is below 128. If so, the n-th character is the n-th byte, and
  character-indexing words use that.
- **Custom glyphs are a later layer,** and they ride on UTF-8 rather than
  replacing it (section 5). Nothing built for UTF-8 is thrown away when they
  come.

## 2. The complaint about Unicode

Thomas: "To this day it is not a simple thing for me to say 'I need these
special glyphs, some custom even' and have a straightforward way to define
them, use them and share them."

- **The official route takes years.** A new character goes through a
  committee.
- **The unofficial route collides.** The Private Use Area is a block of
  code points with no agreed meaning: U+E000–U+F8FF, plus planes 15 and 16.
  - You pick numbers, make a font, and hope nobody else picked the same
    numbers.
  - Icon fonts and the ConScript registry of invented scripts live there.
  - Their numbers collide, because the meaning lives outside the text.
- **Coverage of simple combinations is patchy.** For example, basic shapes
  with basic math symbols inside them are only partly encoded.

## 3. Use cases

- **Numeral systems.**
  - Thomas has studied alternative bases at length. Base 4 and base 6 suit
    human counting better than base 10, which follows from finger counting,
    "a system evolved for dexterity and survival, not counting".
  - A numeral system is a base and a glyph for each digit. Digits need
    glyphs of their own, so they are not confused with decimal digits.
  - March could read and print numbers in such a system, as it reads `1.5`
    today.
- **Notation.** Math symbols, shapes containing operators, and symbols for
  March's own words, beyond what the `\name` table (docs/SURFACE.md) can
  find in Unicode.
- **Ordinary text,** which must simply work, in every language Unicode
  covers.

## 4. Layers

### Layer 1: bytes

A string is a persistent sequence of UTF-8 bytes, valid by construction.
- **Positions are byte offsets.** A search returns positions, and slicing
  takes them.
- **Identity is over the bytes,** so equal text has equal identity and is
  stored once.

Representation, in merkle-champ (stage 1, done 2026-10-05):
- **A content-defined sequence of bytes,** `Sequence<u8>` (merkle-champ
  FORMAT.md section 10). Its leaves end where a rolling hash over the bytes
  says, so its shape depends only on the text, and inserting, deleting,
  joining or slicing rewrites a few nodes near the change; a store shares the
  rest across versions. A first design, a vector with leaves at fixed
  positions, was replaced because an insert rewrote everything after it
  (transfs's observation, 2026-10-05).
- **Leaves of about 1 KB,** since bytes are packed: a leaf of 32 bytes would
  carry as much overhead as data. The same narrow element types serve
  TENSORS.md's numeric arrays: a byte array is a sequence of u8, and a string
  is a byte array known to be valid UTF-8, with a type of its own.
- **Counts in each branch,** to add: a branch already records its
  children's lengths in bytes; it should also record how many code points
  and newlines each holds, as ropes do. Then "the n-th character" and "line
  k" take O(log n) steps, not a scan.

### Layer 2: characters

- **Code points,** for walking and counting: `each` over a string yields
  code points.
- **Grapheme clusters,** what a reader sees as one character (Unicode's
  segmentation rules, UAX #29), as library words.

Both read layer 1; neither changes how text is stored.

**Normalization is explicit.** The same visible text can be written as
different code points: é precomposed, or e plus an accent. Their bytes, and
so their identities, differ. Identity follows the bytes as given, and
normalizing to NFC is a word you call. A map that should treat both
spellings as one key normalizes its keys.

### Layer 3: glyphs by identity

A custom glyph is identified by the hash of its definition, not by a number
someone assigns:
- its outline, for example as an SVG path, with its advance width;
- a name, and a fallback spelling for tools that cannot show it, such as
  `\hexplus`;
- optional properties. A digit carries its value in its numeral system;
  other properties say whether it is a symbol or a letter, and how it
  behaves in right-to-left text.

There is no registry, so there are no collisions: the identity is the
content.

**A text that uses custom glyphs stays UTF-8.**
- It maps each glyph to a private-use code point *within that text*, and
  carries a table from those code points to glyph identities. A PDF does the
  same when it embeds a subset of a font.
- From outside, it is valid UTF-8, and ordinary tools show the private-use
  characters as unknown boxes.
- In March, the table resolves each glyph by identity. Sharing the text
  shares the glyph definitions, fetched by identity like any stored node.
- **Numbering is canonical:** private-use code points are assigned in order
  of first use. So the same text with the same glyphs always has the same
  bytes, table and identity, whoever wrote it.

### Layer 4: naming and display

- **Names.** The symbol table already makes `\times` in source the word `×`.
  It can map `\hexplus` to a custom glyph's identity in the same way.
  `march7 fmt` writes the glyph where the editor can show it, and the escape
  where it cannot.
- **Fonts.** March can generate a font for a project's glyphs, mapping each
  to its private-use code point, so editors such as Zed show them. Icon fonts
  work exactly this way, and tools such as fontTools can build one.
- **Numeral systems.** A declared system (base and digit glyphs) is used by
  the reader for literals and by the printer for output, alongside decimal.

## 5. How it fits March

- **Strings are values with a checker type.**
  - `"…"` literals are read as UTF-8 from the source, validated, and
    compiled as data.
  - The system track keeps `s"` for raw byte buffers in regions.
  - The stack display shows strings as `"…"`.
- **Maps key on strings by content,** through their identity. This is why
  strings come before maps.
- **A character is a code point,** a scalar type of its own. Its literal and
  display syntax are still to be chosen.
- **Families give string operations a place:** `length`, comparison and
  joining get string clauses.

## 6. Stages

1. **Narrow element types in merkle-champ's vector,** with leaf sizes per
   element width, for bytes and for TENSORS.md's numeric types.
2. **Strings in March:** the type, `"…"` literals, display, and words for
   length in bytes and in code points, comparison, joining, slicing by byte
   position, searching, and `each` over code points.
3. **Maps keyed by any value, strings included** (the next item after
   strings).
4. **Grapheme clusters and normalization,** as library words.
5. **Glyphs by identity, fonts and numeral systems** (layers 3 and 4). These
   are later, and Thomas called them "a down the road thing".

## 7. Open questions

1. **What `length` means for a string:** bytes, code points, or neither,
   with two plainly named words, as Swift and Rust do. My lean is two words:
   `bytes` and `chars`.
2. **Invalid UTF-8 at the edges:** reject when building a string, or replace
   bad sequences with U+FFFD. My lean: strings are always valid, decoding
   from bytes says which it does, and raw data stays a byte array.
3. **Character literals and display.** `'a` would clash with `'`, which
   quotes a word.
4. **Numeral-system literals in source:** a prefix naming the system, or a
   context heading that sets it for a section.
