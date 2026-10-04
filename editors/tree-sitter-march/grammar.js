/**
 * Tree-sitter grammar for March, surface and system tracks
 * (march7/docs/SURFACE.md). March is a sequence of whitespace-separated
 * words, so the grammar is shallow: it finds comments, strings, numbers,
 * brackets, definitions, context lines and headings.
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
      $.context,
      $.definition,
      $.system_definition,
      $._expression,
    ),

    // `# math`, `## trig`: a namespace heading.
    heading: $ => seq(
      field('marker', alias(token(prec(2, /#{1,6}/)), $.heading_marker)),
      field('name', $.word),
    ),

    // `= int str ;`: a context line. `==` nests.
    context: $ => seq(
      field('marker', alias(token(prec(2, /={1,6}/)), $.context_marker)),
      repeat($._expression),
      ';',
    ),

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
    ),

    quotation: $ => seq('[', repeat($._expression), ']'),
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
