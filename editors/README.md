# Editor support

## tree-sitter-march

A Tree-sitter grammar for March, the surface track and the system track
(march7/docs/SURFACE.md). It is shallow on purpose: March is a sequence of
words, so the grammar finds comments, strings, numbers, brackets,
definitions, maps, `=` context lines and `#` headings.

One rule needs the external scanner in `src/scanner.c`: a `:` that starts a
line begins a system-track definition (`: name … ;`). Elsewhere, a word
followed by `:` is a name being defined (`name : … ;`). Without it, a word
ending one line, such as `immediate`, would be read as the name of the
definition on the next.

After editing `grammar.js`:

```sh
cd editors/tree-sitter-march
tree-sitter generate
tree-sitter parse ../../march7/seed/system.march --stat -q
tree-sitter query queries/highlights.scm ../../march7/docs/surface/calc.march
```

## Zed

`zed/` is a Zed extension: highlighting, bracket matching, indentation, and
an outline of headings and definitions. Code blocks marked `march` in
Markdown files, such as literate modules, are highlighted too.

Zed builds the grammar from a git commit, so:

1. Commit `editors/`.
2. Put that commit's hash in `zed/extension.toml` as `rev`.
3. In Zed, run `zed: install dev extension` and choose `editors/zed`.

After changing the grammar, commit again, update `rev`, and rebuild the
extension from Zed's extensions page.
