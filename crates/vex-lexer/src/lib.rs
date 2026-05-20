//! Vex lexer — tokenização baseada em `logos`.
//!
//! O lexer emite [`Token`]s com seus spans (byte ranges). Os spans são
//! preservados intencionalmente em todas as fases seguintes para que o
//! diagnostic layer (`vex-diagnostics`) consiga apontar a posição exata
//! no fonte original.

use logos::Logos;
use smol_str::SmolStr;

pub type Span = std::ops::Range<usize>;

#[derive(Logos, Debug, PartialEq, Clone)]
#[logos(skip r"[ \t\r\n\f]+")]
#[logos(skip r"//[^\n]*")]
#[logos(skip r"/\*([^*]|\*[^/])*\*/")]
pub enum Token {
    // ── Literais ───────────────────────────────────────────────────
    #[regex(r"[0-9][0-9_]*", |lex| lex.slice().replace('_', "").parse::<i64>().ok())]
    Int(i64),

    #[regex(r"[0-9][0-9_]*\.[0-9][0-9_]*", |lex| lex.slice().replace('_', "").parse::<f64>().ok())]
    Float(f64),

    #[regex(r#""([^"\\]|\\.)*""#, |lex| {
        let s = lex.slice();
        SmolStr::new(&s[1..s.len()-1])
    })]
    Str(SmolStr),

    #[token("true")]  True,
    #[token("false")] False,

    // ── Keywords ───────────────────────────────────────────────────
    #[token("let")]    Let,
    #[token("const")]  Const,
    #[token("fn")]     Fn,
    #[token("return")] Return,
    #[token("if")]     If,
    #[token("else")]   Else,
    #[token("while")]  While,
    #[token("for")]    For,
    #[token("in")]     In,
    #[token("break")]  Break,
    #[token("continue")] Continue,
    #[token("struct")] Struct,
    #[token("enum")]   Enum,
    #[token("impl")]   Impl,
    #[token("trait")]  Trait,
    #[token("pub")]    Pub,
    #[token("use")]    Use,
    #[token("mod")]    Mod,
    #[token("import")] Import,
    #[token("mut")]    Mut,
    #[token("match")]  Match,
    #[token("self")]   SelfLower,
    #[token("Self")]   SelfUpper,
    #[token("as")]     As,
    #[token("comptime")] Comptime,

    // ── Tipos primitivos ───────────────────────────────────────────
    #[token("int")]    TInt,
    #[token("float")]  TFloat,
    #[token("bool")]   TBool,
    #[token("str")]    TStr,
    #[token("void")]   TVoid,

    // ── Identificadores ────────────────────────────────────────────
    #[regex(r"[a-zA-Z_][a-zA-Z0-9_]*", |lex| SmolStr::new(lex.slice()))]
    Ident(SmolStr),

    // ── Operadores ─────────────────────────────────────────────────
    #[token("+")] Plus,
    #[token("-")] Minus,
    #[token("*")] Star,
    #[token("/")] Slash,
    #[token("%")] Percent,
    #[token("=")] Eq,
    #[token("+=")] PlusEq,
    #[token("-=")] MinusEq,
    #[token("*=")] StarEq,
    #[token("/=")] SlashEq,
    #[token("==")] EqEq,
    #[token("!=")] Neq,
    #[token("<")]  Lt,
    #[token(">")]  Gt,
    #[token("<=")] Lte,
    #[token(">=")] Gte,
    #[token("&&")] AndAnd,
    #[token("||")] OrOr,
    #[token("&")]  Amp,
    #[token("|")]  Pipe,
    #[token("^")]  Caret,
    #[token("!")]  Bang,
    #[token("->")] Arrow,
    #[token("=>")] FatArrow,
    #[token("::")] ColonColon,
    #[token("..")] DotDot,
    #[token("..=")] DotDotEq,

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

/// Token emparelhado com seu [`Span`] no fonte.
#[derive(Debug, Clone, PartialEq)]
pub struct SpannedToken {
    pub token: Token,
    pub span: Span,
}

/// Tokeniza `source` em uma sequência de [`SpannedToken`].
///
/// Tokens malformados são reportados como `Err(span)` — o caller decide
/// se aborta ou tenta recuperar. O parser deve preservar essa decisão.
pub fn tokenize(source: &str) -> Vec<Result<SpannedToken, Span>> {
    Token::lexer(source)
        .spanned()
        .map(|(res, span)| match res {
            Ok(token) => Ok(SpannedToken { token, span }),
            Err(_) => Err(span),
        })
        .collect()
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
    fn lex_numbers() {
        assert_eq!(lex_ok("42 3.14 1_000 1_000.5"),
            vec![Token::Int(42), Token::Float(3.14), Token::Int(1000), Token::Float(1000.5)]);
    }

    #[test]
    fn lex_string() {
        assert_eq!(lex_ok(r#""hello world""#), vec![Token::Str("hello world".into())]);
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
    fn lex_skip_comments() {
        let toks = lex_ok("let x = 1 // ignora\nlet y = 2 /* bloco */ let z = 3");
        assert_eq!(toks.len(), 12);
    }

    #[test]
    fn lex_identifier_distinct_from_keyword() {
        let toks = lex_ok("letter let_x letx");
        assert!(matches!(toks[0], Token::Ident(_)));
        assert!(matches!(toks[1], Token::Ident(_)));
        assert!(matches!(toks[2], Token::Ident(_)));
    }
}
