// External scanner for March: recognizes a `:` that is the first thing on its
// line, followed by whitespace. That `:` starts a system-track definition
// (`: name body ;`), so a word ending the previous line is not read as the
// name of a surface definition (`name : body ;`).

#include "tree_sitter/parser.h"

enum TokenType { LINE_COLON };

void *tree_sitter_march_external_scanner_create(void) { return NULL; }
void tree_sitter_march_external_scanner_destroy(void *payload) {}
unsigned tree_sitter_march_external_scanner_serialize(void *payload, char *buffer) { return 0; }
void tree_sitter_march_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {}

static bool is_space(int32_t c) {
  return c == ' ' || c == '\t' || c == '\r' || c == '\n';
}

bool tree_sitter_march_external_scanner_scan(void *payload, TSLexer *lexer, const bool *valid_symbols) {
  if (!valid_symbols[LINE_COLON]) return false;
  bool line_start = lexer->get_column(lexer) == 0;
  while (is_space(lexer->lookahead)) {
    if (lexer->lookahead == '\n') line_start = true;
    lexer->advance(lexer, true);
  }
  if (!line_start || lexer->lookahead != ':') return false;
  lexer->advance(lexer, false);
  if (!lexer->eof(lexer) && !is_space(lexer->lookahead)) return false;  // `:x`, `::`
  lexer->mark_end(lexer);
  lexer->result_symbol = LINE_COLON;
  return true;
}
