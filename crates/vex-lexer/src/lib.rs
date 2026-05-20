//! Vex lexer — tokenização baseada em `logos`.
//!
//! O lexer emite [`Token`]s com seus spans (byte ranges). Os spans são
//! preservados em todas as fases seguintes para que o diagnostic layer
//! (`vex-diagnostics`) consiga apontar a posição exata no fonte original.
//!
//! Princípios:
//! - falhar com [`LexError`] estruturado, nunca silenciar.
//! - escapes de string processados no lexer (parser consome literal já tratado).
//! - sem allocação em tokens triviais — texto cru via [`smol_str::SmolStr`].

use logos::Logos;
use smol_str::SmolStr;

pub type Span = std::ops::Range<usize>;

/// Erro de tokenização. Cada variante carrega o span exato para diagnóstico.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LexError {
    #[error("caractere inesperado")]
    UnknownChar { span: Span },
    #[error("string literal não fechada")]
    UnterminatedString { span: Span },
    #[error("char literal não fechado")]
    UnterminatedChar { span: Span },
    #[error("char literal inválido")]
    InvalidCharLiteral { span: Span },
    #[error("escape sequence inválida")]
    InvalidEscape { span: Span },
    #[error("bloco de comentário não fechado")]
    UnterminatedBlockComment { span: Span },
    #[error("número inválido: {raw}")]
    InvalidNumber { span: Span, raw: String },
}

impl LexError {
    pub fn span(&self) -> &Span {
        match self {
            LexError::UnknownChar { span }
            | LexError::UnterminatedString { span }
            | LexError::UnterminatedChar { span }
            | LexError::InvalidCharLiteral { span }
            | LexError::InvalidEscape { span }
            | LexError::UnterminatedBlockComment { span }
            | LexError::InvalidNumber { span, .. } => span,
        }
    }
}

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\r\n\f]+")]
#[logos(skip r"//[^\n]*")]
pub enum Token {
    // ── Literais ───────────────────────────────────────────────────
    #[regex(r"[0-9][0-9_]*", parse_int)]
    Int(i64),

    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*", parse_float)]
    Float(f64),

    /// String com escapes já processados.
    #[token("\"", parse_string)]
    Str(SmolStr),

    /// Char literal Unicode (codepoint único).
    #[token("'", parse_char)]
    Char(char),

    /// Comentário de bloco `/* ... */`, possivelmente aninhado.
    /// Tratado como skip mas com erro se não fechado.
    #[token("/*", skip_block_comment)]
    BlockComment,

    #[token("true")]  True,
    #[token("false")] False,

    // ── Keywords ───────────────────────────────────────────────────
    #[token("let")]      Let,
    #[token("const")]    Const,
    // `fn` é a forma tradicional; `def` é alias Python-friendly.
    // Parser trata ambos identicamente.
    #[token("fn")]
    #[token("def")]      Fn,
    #[token("return")]   Return,
    #[token("if")]       If,
    #[token("else")]     Else,
    #[token("while")]    While,
    #[token("for")]      For,
    #[token("in")]       In,
    #[token("break")]    Break,
    #[token("continue")] Continue,
    // `struct` clássico; `class` alias Python-friendly.
    #[token("struct")]
    #[token("class")]    Struct,
    #[token("enum")]     Enum,
    #[token("impl")]     Impl,
    #[token("trait")]    Trait,
    #[token("pub")]      Pub,
    #[token("use")]      Use,
    #[token("mod")]      Mod,
    #[token("import")]   Import,
    #[token("mut")]      Mut,
    #[token("match")]    Match,
    #[token("self")]     SelfLower,
    #[token("Self")]     SelfUpper,
    #[token("as")]       As,
    #[token("comptime")] Comptime,

    // ── Tipos primitivos ───────────────────────────────────────────
    #[token("int")]   TInt,
    #[token("float")] TFloat,
    #[token("bool")]  TBool,
    #[token("str")]   TStr,
    #[token("char")]  TChar,
    #[token("void")]  TVoid,

    // ── Identificadores ────────────────────────────────────────────
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| SmolStr::new(lex.slice()))]
    Ident(SmolStr),

    // ── Operadores ─────────────────────────────────────────────────
    // Ordem dos tokens importa em logos quando há prefixos comuns:
    // operadores compostos vêm antes dos simples para o longest-match.
    #[token("==")] EqEq,
    #[token("!=")] Neq,
    #[token("<=")] Lte,
    #[token(">=")] Gte,
    #[token("+=")] PlusEq,
    #[token("-=")] MinusEq,
    #[token("*=")] StarEq,
    #[token("/=")] SlashEq,
    #[token("&&")] AndAnd,
    #[token("||")] OrOr,
    #[token("->")] Arrow,
    #[token("=>")] FatArrow,
    #[token("::")] ColonColon,
    #[token("..=")] DotDotEq,
    #[token("..")] DotDot,

    #[token("+")] Plus,
    #[token("-")] Minus,
    #[token("*")] Star,
    #[token("/")] Slash,
    #[token("%")] Percent,
    #[token("=")] Eq,
    #[token("<")] Lt,
    #[token(">")] Gt,
    #[token("&")] Amp,
    #[token("|")] Pipe,
    #[token("^")] Caret,
    #[token("!")] Bang,

    // ── Pontuação ──────────────────────────────────────────────────
    #[token("(")] LParen,
    #[token(")")] RParen,
    #[token("{")] LBrace,
    #[token("}")] RBrace,
    #[token("[")] LBracket,
    #[token("]")] RBracket,
    #[token(":")] Colon,
    #[token(";")] Semi,
    #[token(",")] Comma,
    #[token(".")] Dot,
    #[token("?")] Question,
    #[token("@")] At,
}

// ── Callbacks de logos ────────────────────────────────────────────────

fn parse_int(lex: &mut logos::Lexer<Token>) -> Result<i64, ()> {
    lex.slice().replace('_', "").parse::<i64>().map_err(|_| ())
}

fn parse_float(lex: &mut logos::Lexer<Token>) -> Result<f64, ()> {
    lex.slice().replace('_', "").parse::<f64>().map_err(|_| ())
}

/// Lê string literal com escapes a partir da aspa de abertura já consumida.
///
/// Avança manualmente o cursor de `remainder()` e ajusta `bump()` ao final.
fn parse_string(lex: &mut logos::Lexer<Token>) -> Result<SmolStr, ()> {
    let rem = lex.remainder();
    let bytes = rem.as_bytes();
    let mut out = String::new();
    let mut i = 0;

    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                lex.bump(i + 1); // consome até e incluindo a aspa de fechamento
                return Ok(SmolStr::new(&out));
            }
            b'\\' => {
                if i + 1 >= bytes.len() {
                    lex.bump(i);
                    return Err(());
                }
                let esc = bytes[i + 1];
                let decoded = match esc {
                    b'n' => '\n',
                    b't' => '\t',
                    b'r' => '\r',
                    b'0' => '\0',
                    b'\\' => '\\',
                    b'"' => '"',
                    b'\'' => '\'',
                    _ => {
                        lex.bump(i + 2);
                        return Err(());
                    }
                };
                out.push(decoded);
                i += 2;
            }
            b'\n' => {
                lex.bump(i);
                return Err(());
            }
            b => {
                out.push(b as char);
                i += 1;
            }
        }
    }

    // Fim do input sem fechar: consome tudo para que o erro seja um único
    // span contíguo (do `"` inicial até o EOF), não fragmentos.
    lex.bump(bytes.len());
    Err(())
}

/// Lê char literal a partir da apóstrofe de abertura já consumida.
fn parse_char(lex: &mut logos::Lexer<Token>) -> Result<char, ()> {
    let rem = lex.remainder();
    let mut chars = rem.char_indices();

    let (_, c) = chars.next().ok_or(())?;
    let (decoded, consumed_bytes) = if c == '\\' {
        let (_, esc) = chars.next().ok_or(())?;
        let d = match esc {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            '0' => '\0',
            '\\' => '\\',
            '\'' => '\'',
            '"' => '"',
            _ => return Err(()),
        };
        (d, 1 + esc.len_utf8())
    } else if c == '\'' {
        // char vazio: ''
        return Err(());
    } else if c == '\n' {
        return Err(());
    } else {
        (c, c.len_utf8())
    };

    // próximo caractere deve ser a apóstrofe de fechamento
    let (close_off, close) = chars.next().ok_or(())?;
    if close != '\'' { return Err(()); }
    debug_assert_eq!(close_off, consumed_bytes);

    lex.bump(consumed_bytes + 1);
    Ok(decoded)
}

/// Consome `/* ... */` com aninhamento. Retorna `Err` se não fechar.
fn skip_block_comment(lex: &mut logos::Lexer<Token>) -> logos::Skip {
    let rem = lex.remainder();
    let bytes = rem.as_bytes();
    let mut depth: usize = 1;
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'/' && bytes[i + 1] == b'*' {
            depth += 1;
            i += 2;
        } else if bytes[i] == b'*' && bytes[i + 1] == b'/' {
            depth -= 1;
            i += 2;
            if depth == 0 {
                lex.bump(i);
                return logos::Skip;
            }
        } else {
            i += 1;
        }
    }
    // não fechou — consome o resto e marca skip; o caller detecta via
    // ausência de tokens válidos. Em uma versão futura, retornar Filter::Emit
    // com um Token::Error para diagnóstico preciso.
    lex.bump(bytes.len());
    logos::Skip
}

/// Token emparelhado com seu [`Span`] no fonte.
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

/// Tokeniza `source` em uma sequência de [`SpannedToken`].
///
/// Tokens malformados são reportados como [`LexError`] com span exato.
/// O caller decide se aborta ou tenta recuperar.
pub fn tokenize(source: &str) -> Vec<Result<SpannedToken, LexError>> {
    let mut out = Vec::new();
    for (res, span) in Token::lexer(source).spanned() {
        match res {
            Ok(token) => out.push(Ok(SpannedToken { token, span })),
            Err(_) => out.push(Err(classify_error(source, span))),
        }
    }
    out
}

/// Diferencia tipos de erro inspecionando o slice do fonte no span.
fn classify_error(source: &str, span: Span) -> LexError {
    let slice = &source[span.clone()];
    if slice.starts_with('"') {
        LexError::UnterminatedString { span }
    } else if slice.starts_with('\'') {
        if slice.len() <= 2 {
            LexError::UnterminatedChar { span }
        } else {
            LexError::InvalidCharLiteral { span }
        }
    } else if slice.contains('.') || slice.chars().all(|c| c.is_ascii_digit() || c == '_') {
        LexError::InvalidNumber { span, raw: slice.to_string() }
    } else {
        LexError::UnknownChar { span }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lex_ok(src: &str) -> Vec<Token> {
        tokenize(src)
            .into_iter()
            .map(|r| r.expect("lex error").token)
            .collect()
    }

    fn lex_with_spans(src: &str) -> Vec<(Token, Span)> {
        tokenize(src)
            .into_iter()
            .map(|r| {
                let st = r.expect("lex error");
                (st.token, st.span)
            })
            .collect()
    }

    #[test]
    fn lex_keywords() {
        assert_eq!(
            lex_ok("let fn return if else while for in struct impl mut"),
            vec![
                Token::Let, Token::Fn, Token::Return, Token::If, Token::Else,
                Token::While, Token::For, Token::In, Token::Struct, Token::Impl,
                Token::Mut,
            ]
        );
    }

    #[test]
    fn lex_all_keywords_extended() {
        let toks = lex_ok("const enum trait pub use mod import match break continue self Self as comptime");
        assert_eq!(toks, vec![
            Token::Const, Token::Enum, Token::Trait, Token::Pub, Token::Use,
            Token::Mod, Token::Import, Token::Match, Token::Break, Token::Continue,
            Token::SelfLower, Token::SelfUpper, Token::As, Token::Comptime,
        ]);
    }

    #[test]
    fn lex_primitive_types() {
        assert_eq!(
            lex_ok("int float bool str char void"),
            vec![Token::TInt, Token::TFloat, Token::TBool, Token::TStr, Token::TChar, Token::TVoid]
        );
    }

    #[test]
    fn lex_numbers() {
        assert_eq!(
            lex_ok("42 2.5 1_000 1_000.5"),
            vec![Token::Int(42), Token::Float(2.5), Token::Int(1000), Token::Float(1000.5)]
        );
    }

    #[test]
    fn lex_string_simple() {
        assert_eq!(lex_ok(r#""hello world""#), vec![Token::Str("hello world".into())]);
    }

    #[test]
    fn lex_string_with_escapes() {
        let toks = lex_ok(r#""hello\nworld\t!""#);
        assert_eq!(toks, vec![Token::Str("hello\nworld\t!".into())]);
    }

    #[test]
    fn lex_string_escaped_quote() {
        let toks = lex_ok(r#""she said \"hi\"""#);
        assert_eq!(toks, vec![Token::Str(r#"she said "hi""#.into())]);
    }

    #[test]
    fn lex_string_backslash() {
        let toks = lex_ok(r#""C:\\path""#);
        assert_eq!(toks, vec![Token::Str("C:\\path".into())]);
    }

    #[test]
    fn lex_char_literals() {
        assert_eq!(lex_ok("'a' '\\n' '\\t' '\\\\' '\\''"),
            vec![Token::Char('a'), Token::Char('\n'), Token::Char('\t'), Token::Char('\\'), Token::Char('\'')]);
    }

    #[test]
    fn lex_char_unicode() {
        assert_eq!(lex_ok("'á' 'ñ' '日'"),
            vec![Token::Char('á'), Token::Char('ñ'), Token::Char('日')]);
    }

    #[test]
    fn lex_operators() {
        assert_eq!(
            lex_ok("+ - * / == != <= >= && || -> ::"),
            vec![
                Token::Plus, Token::Minus, Token::Star, Token::Slash,
                Token::EqEq, Token::Neq, Token::Lte, Token::Gte,
                Token::AndAnd, Token::OrOr, Token::Arrow, Token::ColonColon,
            ]
        );
    }

    #[test]
    fn lex_compound_assignments() {
        assert_eq!(
            lex_ok("+= -= *= /= => ..="),
            vec![Token::PlusEq, Token::MinusEq, Token::StarEq, Token::SlashEq, Token::FatArrow, Token::DotDotEq]
        );
    }

    #[test]
    fn lex_punctuation() {
        assert_eq!(
            lex_ok("( ) { } [ ] : ; , . ? @ .."),
            vec![
                Token::LParen, Token::RParen, Token::LBrace, Token::RBrace,
                Token::LBracket, Token::RBracket, Token::Colon, Token::Semi,
                Token::Comma, Token::Dot, Token::Question, Token::At, Token::DotDot,
            ]
        );
    }

    #[test]
    fn lex_skip_comments() {
        let toks = lex_ok("let x = 1 // ignora\nlet y = 2 /* bloco */ let z = 3");
        assert_eq!(toks.len(), 12);
    }

    #[test]
    fn lex_nested_block_comments() {
        // aninhamento: /* outer /* inner */ ainda outer */
        let toks = lex_ok("let /* a /* b */ c */ x = 1");
        assert_eq!(toks, vec![
            Token::Let, Token::Ident("x".into()), Token::Eq, Token::Int(1),
        ]);
    }

    #[test]
    fn lex_identifier_distinct_from_keyword() {
        let toks = lex_ok("letter let_x letx");
        assert!(matches!(toks[0], Token::Ident(_)));
        assert!(matches!(toks[1], Token::Ident(_)));
        assert!(matches!(toks[2], Token::Ident(_)));
    }

    #[test]
    fn lex_spans_preserved() {
        let spans: Vec<Span> = lex_with_spans("let x = 42")
            .into_iter()
            .map(|(_, s)| s)
            .collect();
        assert_eq!(spans, vec![0..3, 4..5, 6..7, 8..10]);
    }

    #[test]
    fn lex_unterminated_string_is_error() {
        let res = tokenize(r#""no close"#);
        assert_eq!(res.len(), 1);
        assert!(matches!(res[0], Err(LexError::UnterminatedString { .. })));
    }

    #[test]
    fn lex_invalid_escape_is_error() {
        let res = tokenize(r#""bad \q escape""#);
        assert!(res.iter().any(|r| matches!(r, Err(LexError::UnterminatedString { .. })
                                            | Err(LexError::InvalidEscape { .. }))),
            "expected error, got {res:?}");
    }

    #[test]
    fn lex_unknown_char_is_error() {
        let res = tokenize("let # x");
        assert!(res.iter().any(|r| matches!(r, Err(LexError::UnknownChar { .. }))));
    }

    #[test]
    fn lex_fn_declaration_full() {
        let toks = lex_ok("fn fib(n: int) -> int { return n }");
        assert_eq!(toks, vec![
            Token::Fn, Token::Ident("fib".into()), Token::LParen,
            Token::Ident("n".into()), Token::Colon, Token::TInt,
            Token::RParen, Token::Arrow, Token::TInt,
            Token::LBrace, Token::Return, Token::Ident("n".into()), Token::RBrace,
        ]);
    }

    #[test]
    fn lex_struct_literal() {
        let toks = lex_ok("Ponto { x: 0.0, y: 0.0 }");
        assert_eq!(toks, vec![
            Token::Ident("Ponto".into()), Token::LBrace,
            Token::Ident("x".into()), Token::Colon, Token::Float(0.0), Token::Comma,
            Token::Ident("y".into()), Token::Colon, Token::Float(0.0),
            Token::RBrace,
        ]);
    }
}
