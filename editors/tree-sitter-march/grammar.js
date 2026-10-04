/**
 * Tree-sitter grammar for March, surface and system tracks
 * (march7/docs/SURFACE.md). March is a sequence of whitespace-separated
 * words, so the grammar is shallow: it finds comments, strings, numbers,
 * brackets, patterns, definitions and headings.
 */

// A word is any run of characters other than whitespace, brackets and quotes.
const WORD = /[^\s\[\]\(\)\{\}"]+/;

module.exports = grammar({
  name: 'march',

  extras: $ => [/\s/, $.comment],

  // src/scanner.c: a `:` that is the first thing on its line.
  externals: $ => [$._line_colon],

  word: $ => $.word,

  rules: {
    source_file: $ => repeat($._item),

    _item: $ => choice(
      $.heading,
      $.definition,
      $.system_definition,
      $._expression,
    ),

    // `# math` opens a namespace; `## < i64 > < f64 >` opens a context, its
    // patterns being alternatives, which may continue on the next lines.
    // A `<` after a context heading always continues it.
    heading: $ => prec.right(seq(
      field('marker', alias(token(prec(2, /#{1,6}/)), $.heading_marker)),
      choice(
        field('name', $.word),
        repeat1(field('pattern', $.pattern)),
      ),
    )),

    // `name : body ;`, the surface form. A word followed by `:` is always a
    // name being defined, never a call before a system-track definition.
    definition: $ => prec(1, seq(
      field('name', $.word),
      ':',
      repeat($._expression),
      ';',
    )),

    // `: name body ;`, the system track's FORTH form, which may also define
    // the bracket words themselves.
    // A `:` that starts a line is taken as this form, so a word ending the
    // previous line (often `immediate`) is not read as a surface name.
    system_definition: $ => seq(
      alias($._line_colon, ':'),
      field('name', choice($.word, alias(choice('[', ']'), $.word))),
      repeat($._expression),
      ';',
    ),

    _expression: $ => choice(
      $.string,
      $.number,
      $.word,
      $.quotation,
      $.sequence,
      $.map,
      $.pattern,
    ),

    quotation: $ => seq('[', repeat($._expression), ']'),
    // `< i64 positive? -> i64 >`: a pattern or signature.
    pattern: $ => seq('<', repeat($._expression), '>'),
    sequence: $ => seq('(', repeat($._expression), ')'),

    // `{ name : body ; … }`: the last entry's `;` is optional.
    map: $ => seq(
      '{',
      repeat(seq($.entry, ';')),
      optional($.entry),
      '}',
    ),
    entry: $ => prec(1, seq(
      field('name', $.word),
      ':',
      repeat($._expression),
    )),

    // `--` starts a comment that runs to the end of the line.
    comment: _ => token(prec(3, /--([ \t][^\n]*)?/)),

    string: _ => token(seq('"', /[^"]*/, '"')),

    number: _ => token(prec(1, /[+-]?\d+(\.\d+)?([eE][+-]?\d+)?/)),

    word: _ => WORD,
  },
});
