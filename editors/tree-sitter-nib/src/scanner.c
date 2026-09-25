#include "tree_sitter/alloc.h"
#include "tree_sitter/parser.h"

#include <stdbool.h>
#include <stdint.h>
#include <string.h>

enum TokenType {
    NEWLINE,
    INDENT,
    DEDENT,
    ASM_BODY,
    TRY_QUESTION,
    TERNARY_QUESTION,
    ERROR_SENTINEL,
    CLOSE_PAREN,
    CLOSE_BRACKET,
    CLOSE_BRACE,
};

#define MAX_DEPTH 256

typedef struct {
    uint16_t indents[MAX_DEPTH];
    uint32_t count;
} Scanner;

static inline void advance(TSLexer *lexer) { lexer->advance(lexer, false); }
static inline void skip(TSLexer *lexer) { lexer->advance(lexer, true); }

static inline bool at_line_end(TSLexer *lexer) {
    return lexer->lookahead == '\n' || lexer->lookahead == '\r' || lexer->eof(lexer);
}

static inline uint16_t current_indent(Scanner *scanner) {
    return scanner->count ? scanner->indents[scanner->count - 1] : 0;
}

// A quoted literal from its opening quote; an f-string's braces hold source,
// where a quote opens a nested literal.
static void skip_quoted(TSLexer *lexer, bool interpolated) {
    advance(lexer);
    int32_t braces = 0;
    while (!at_line_end(lexer)) {
        int32_t c = lexer->lookahead;
        if (braces > 0) {
            if (c == '"') {
                skip_quoted(lexer, false);
                continue;
            }
            if (c == '{') braces++;
            if (c == '}') braces--;
            advance(lexer);
            continue;
        }
        if (c == '"') {
            advance(lexer);
            return;
        }
        if (c == '\\') {
            advance(lexer);
            if (!at_line_end(lexer)) advance(lexer);
            continue;
        }
        if (interpolated && c == '{') braces++;
        advance(lexer);
    }
}

static inline bool is_word(int32_t c) {
    return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9') || c == '_';
}

// The parser's rule: `?` opens a conditional when a `:` follows at its
// depth on the line and does not end the line, where it opens a block.
static bool question_is_conditional(TSLexer *lexer) {
    int32_t depth = 0;
    int32_t previous = 0;
    for (;;) {
        int32_t c = lexer->lookahead;
        if (lexer->eof(lexer)) return false;
        if (c == '\n' || c == '\r') {
            if (depth == 0) return false;
            advance(lexer);
        } else if (c == '#') {
            while (!at_line_end(lexer)) advance(lexer);
        } else if (c == '"') {
            skip_quoted(lexer, false);
        } else if (c == 'f' && !is_word(previous)) {
            advance(lexer);
            if (lexer->lookahead == '"') skip_quoted(lexer, true);
            else previous = 'f';
            continue;
        } else if (c == '\'') {
            advance(lexer);
            if (lexer->lookahead == '\\') advance(lexer);
            if (!at_line_end(lexer)) advance(lexer);
            while (!at_line_end(lexer) && lexer->lookahead != '\'') advance(lexer);
            if (lexer->lookahead == '\'') advance(lexer);
        } else if (c == '(' || c == '[' || c == '{') {
            depth++;
            advance(lexer);
        } else if (c == ')' || c == ']' || c == '}') {
            if (depth == 0) return false;
            depth--;
            advance(lexer);
        } else if (c == ',' && depth == 0) {
            return false;
        } else if (c == ':' && depth == 0) {
            advance(lexer);
            while (lexer->lookahead == ' ' || lexer->lookahead == '\t') advance(lexer);
            return !(at_line_end(lexer) || lexer->lookahead == '#');
        } else {
            advance(lexer);
        }
        previous = c;
    }
}

// An asm block's lines: those deeper than its header, as one token that
// ends with the last of them.
static bool scan_asm_body(Scanner *scanner, TSLexer *lexer) {
    while (lexer->lookahead == ' ' || lexer->lookahead == '\t') skip(lexer);
    if (lexer->lookahead == '#') {
        while (!at_line_end(lexer)) skip(lexer);
    }
    if (lexer->lookahead == '\r') skip(lexer);
    if (lexer->lookahead != '\n') return false;
    skip(lexer);
    uint16_t header = current_indent(scanner);
    bool found = false;
    for (;;) {
        uint32_t indent = 0;
        while (lexer->lookahead == ' ' || lexer->lookahead == '\t') {
            indent++;
            lexer->advance(lexer, !found);
        }
        if (lexer->eof(lexer)) break;
        bool blank = at_line_end(lexer) || lexer->lookahead == '#';
        if (!blank && indent <= header) break;
        if (!blank) found = true;
        while (!at_line_end(lexer)) lexer->advance(lexer, !found);
        if (!blank) lexer->mark_end(lexer);
        if (lexer->lookahead == '\r') lexer->advance(lexer, !found);
        if (lexer->lookahead != '\n') break;
        lexer->advance(lexer, !found);
    }
    if (!found) return false;
    lexer->result_symbol = ASM_BODY;
    return true;
}

bool tree_sitter_nib_external_scanner_scan(void *payload, TSLexer *lexer, const bool *valid_symbols) {
    Scanner *scanner = (Scanner *)payload;
    bool error_recovery = valid_symbols[ERROR_SENTINEL];

    if (valid_symbols[ASM_BODY] && !error_recovery) {
        return scan_asm_body(scanner, lexer);
    }

    lexer->mark_end(lexer);

    bool found_end_of_line = false;
    uint32_t indent = 0;
    int32_t first_comment_indent = -1;
    for (;;) {
        if (lexer->lookahead == '\n') {
            found_end_of_line = true;
            indent = 0;
            skip(lexer);
        } else if (lexer->lookahead == ' ') {
            indent++;
            skip(lexer);
        } else if (lexer->lookahead == '\r' || lexer->lookahead == '\f') {
            indent = 0;
            skip(lexer);
        } else if (lexer->lookahead == '\t') {
            indent += 8;
            skip(lexer);
        } else if (lexer->lookahead == '#') {
            // A comment after code on the line decides nothing.
            if (!found_end_of_line) return false;
            if (first_comment_indent == -1) first_comment_indent = (int32_t)indent;
            while (lexer->lookahead && lexer->lookahead != '\n') skip(lexer);
            skip(lexer);
            indent = 0;
        } else if (lexer->eof(lexer)) {
            indent = 0;
            found_end_of_line = true;
            break;
        } else {
            break;
        }
    }

    if (!found_end_of_line) {
        if (lexer->lookahead == '?' && (valid_symbols[TRY_QUESTION] || valid_symbols[TERNARY_QUESTION])) {
            advance(lexer);
            lexer->mark_end(lexer);
            bool conditional = question_is_conditional(lexer);
            if (conditional && !valid_symbols[TERNARY_QUESTION]) conditional = false;
            if (!conditional && !valid_symbols[TRY_QUESTION]) conditional = true;
            lexer->result_symbol = conditional ? TERNARY_QUESTION : TRY_QUESTION;
            return true;
        }
        return false;
    }

    uint16_t current = current_indent(scanner);
    if (valid_symbols[INDENT] && indent > current && scanner->count < MAX_DEPTH) {
        scanner->indents[scanner->count++] = (uint16_t)indent;
        lexer->result_symbol = INDENT;
        return true;
    }
    // Comments indented as the block is stay in it: dedent after them.
    if (valid_symbols[DEDENT] && indent < current && first_comment_indent < (int32_t)current && scanner->count > 0) {
        scanner->count--;
        lexer->result_symbol = DEDENT;
        return true;
    }
    // A bracket still open when a line starts no deeper than its block is
    // one being typed: end the line there, so the error stays on it.
    bool within_brackets = valid_symbols[CLOSE_PAREN] || valid_symbols[CLOSE_BRACKET] || valid_symbols[CLOSE_BRACE];
    bool closer = lexer->lookahead == ')' || lexer->lookahead == ']' || lexer->lookahead == '}';
    if (within_brackets && !error_recovery && !valid_symbols[NEWLINE] && indent <= current && !closer) {
        lexer->result_symbol = NEWLINE;
        return true;
    }
    if (valid_symbols[NEWLINE] && !error_recovery) {
        lexer->result_symbol = NEWLINE;
        return true;
    }
    return false;
}

void *tree_sitter_nib_external_scanner_create(void) {
    Scanner *scanner = (Scanner *)ts_calloc(1, sizeof(Scanner));
    return scanner;
}

void tree_sitter_nib_external_scanner_destroy(void *payload) { ts_free(payload); }

unsigned tree_sitter_nib_external_scanner_serialize(void *payload, char *buffer) {
    Scanner *scanner = (Scanner *)payload;
    uint32_t count = scanner->count;
    if (count * sizeof(uint16_t) > TREE_SITTER_SERIALIZATION_BUFFER_SIZE) {
        count = TREE_SITTER_SERIALIZATION_BUFFER_SIZE / sizeof(uint16_t);
    }
    memcpy(buffer, scanner->indents, count * sizeof(uint16_t));
    return count * sizeof(uint16_t);
}

void tree_sitter_nib_external_scanner_deserialize(void *payload, const char *buffer, unsigned length) {
    Scanner *scanner = (Scanner *)payload;
    scanner->count = length / sizeof(uint16_t);
    if (scanner->count > MAX_DEPTH) scanner->count = MAX_DEPTH;
    if (length) memcpy(scanner->indents, buffer, scanner->count * sizeof(uint16_t));
}
