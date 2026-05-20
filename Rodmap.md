# Vex Lang — Roadmap v2

> Linguagem de programação: rápida como C++, segura como Rust, legível como Python.
> Stack: **Rust** + **LLVM 17 (inkwell 0.9)** + **llvm-mingw** (cross-Windows) + **WASM** (futuro).
>
> Dev em **WSL2 Ubuntu**. Target binário Linux **e** Windows (`.exe`) desde o início.

Fundamentação: ver `docs/design/0001-architecture.md` (ADR baseado em
pesquisa do estado da arte 2026 — Cranelift, QBE, Vale, Hylo, Austral,
Mojo, Zig, Gleam).

---

## Pilares

1. **Performance** — LLVM, zero GC, ASAP destruction.
2. **Safety** — ownership híbrido (generational refs + linear types opcionais).
3. **Ergonomia** — sintaxe enxuta, inferência local, mensagens de erro extraordinárias.
4. **No hidden control flow** — sem alocações invisíveis, sem exceptions implícitas.
5. **Cross-platform** — Linux e Windows desde v0.1, WASM em v0.5+.

---

## Estrutura do repositório

```
vex/
├── Cargo.toml              # workspace
├── rust-toolchain.toml     # 1.85+
├── .github/workflows/      # CI Linux + cross-Windows
├── crates/
│   ├── vex-lexer/          # logos 0.14
│   ├── vex-ast/            # nós AST + spans
│   ├── vex-parser/         # recursive descent + Pratt
│   ├── vex-hir/            # HIR (pós resolução de nomes)
│   ├── vex-typeck/         # inferência + ownership
│   ├── vex-mir/            # MIR (lowering ownership → CFG)
│   ├── vex-codegen/        # inkwell → LLVM IR
│   ├── vex-driver/         # orquestra pipeline
│   ├── vex-diagnostics/    # miette wrappers
│   ├── vex-cli/            # binário `vex`
│   ├── vex-fmt/            # formatter opinativo
│   └── vex-lsp/            # language server (stub Fase 8)
├── runtime/                # runtime nativo (gen refs, intrínsecos)
├── std/                    # stdlib em .vex
├── examples/               # programas .vex de teste
├── tests/                  # ui / integration / corpus
├── docs/
│   ├── spec.md             # spec da linguagem
│   ├── grammar.ebnf        # gramática formal
│   └── design/             # ADRs
├── tools/                  # setup scripts (llvm-mingw, etc.)
└── benchmarks/             # hyperfine scripts
```

---

## Fase 0 — Setup (Dia 1)

**Status:** Concluída.

- [x] Workspace Cargo com 12 crates + runtime
- [x] `rust-toolchain.toml` pinando 1.85 + target windows-gnu
- [x] `.gitignore`, CI GitHub Actions (Linux + cross-Windows)
- [x] Script `tools/setup-llvm-mingw.sh` para baixar toolchain
- [x] Documentação inicial (spec, grammar, ADR 0001)
- [x] Examples placeholder (`hello.vex`, `fib.vex`, `ponto.vex`)

### Setup local (uma vez)

```bash
# 1. WSL2 Ubuntu — instalar LLVM 17
sudo apt-get install -y llvm-17-dev libpolly-17-dev clang-17 lld-17
echo 'export LLVM_SYS_170_PREFIX=/usr/lib/llvm-17' >> ~/.bashrc

# 2. Toolchain cross-Windows
./tools/setup-llvm-mingw.sh
rustup target add x86_64-pc-windows-gnu

# 3. Build
cargo build --workspace
cargo test --workspace
```

---

## Fase 1 — Lexer (Dia 2-3)

**Status:** ✅ Concluída.

- [x] `Token` enum com `logos` 0.14
- [x] Keywords (const, enum, trait, pub, use, mod, import, match, break,
      continue, self, Self, as, comptime, etc.)
- [x] Tipos primitivos (int, float, bool, str, char, void)
- [x] Numéricos com `_` separator (`1_000`, `1_000.5`)
- [x] String literals com escapes (`\n`, `\t`, `\r`, `\0`, `\\`, `\"`, `\'`)
- [x] Char literals com escapes (`'a'`, `'\n'`, `'á'` Unicode)
- [x] Block comments aninhados (`/* a /* b */ c */`)
- [x] Operadores compostos (longest-match: `==`, `!=`, `<=`, `>=`,
      `+=`, `-=`, `*=`, `/=`, `&&`, `||`, `->`, `=>`, `::`, `..`, `..=`)
- [x] Pontuação completa (`@`, `?`, etc.)
- [x] `LexError` estruturado com span (UnterminatedString,
      UnterminatedChar, InvalidCharLiteral, InvalidEscape,
      UnterminatedBlockComment, InvalidNumber, UnknownChar)
- [x] Spans preservados em todos os tokens
- [x] 22 testes unitários + 3 snapshot tests (`insta`) sobre
      `examples/{hello,fib,ponto}.vex`
- [x] `cargo clippy -D warnings` verde

**Entregável:** `cargo test -p vex-lexer` verde (25/25).

---

## Fase 2 — Parser (Dia 4-8)

**Mudança vs roadmap original:** parser **hand-written** (recursive
descent + Pratt para expressões), não gerado.

- [ ] Estrutura de erros com recovery (`Result` + sync points)
- [ ] Recursive descent para items (fn, struct, impl)
- [ ] Pratt parser para expressões (precedência via binding power)
- [ ] Statements (let, return, if, while, for, match)
- [ ] Patterns (literal, ident, wildcard, range)
- [ ] Integração `miette` — erros com span + hint

**Entregável:** Parser produz AST correta para todos os `examples/`.

---

## Fase 3 — Resolução de nomes + HIR (Dia 9-10)

**Novo no roadmap v2.**

- [ ] Atribuir `DefId` único a cada item
- [ ] Resolver identificadores → `DefId`
- [ ] Construir tabela de símbolos por escopo
- [ ] AST → HIR lowering

---

## Fase 4 — Type checker (Dia 11-15)

- [ ] Tipos primitivos + checagem de compatibilidade
- [ ] Inferência local Hindley-Milner para `let` sem anotação
- [ ] Validação de retornos vs assinatura
- [ ] Validação de chamadas (aridade + tipos)
- [ ] Validação de campos de struct
- [ ] Erros tipados com `miette` + hints

**Entregável:** Programas inválidos rejeitados com mensagens úteis.

---

## Fase 5 — MIR + ownership baseline (Dia 16-20)

**Novo no roadmap v2.**

- [ ] HIR → MIR lowering (CFG)
- [ ] Generational references: tag de 8 bytes por alocação
- [ ] Gen check inserido em derefs
- [ ] ASAP destruction: análise de último uso
- [ ] Validação básica (sem region analysis ainda — v1.1+)

---

## Fase 6 — Codegen LLVM (Dia 21-26)

- [ ] Setup `inkwell` 0.9 + LLVM 17
- [ ] Mapeamento de tipos Vex → LLVM IR
- [ ] Codegen de fn, call, return
- [ ] Codegen de control flow (if, while, for)
- [ ] Codegen de structs + acesso a campos
- [ ] Linkar runtime (`vex-runtime` staticlib)
- [ ] **Cross-compile** Linux→Windows via `--target x86_64-pc-windows-gnu`

**Entregável:** `vex build hello.vex --target windows` gera `hello.exe`.

---

## Fase 7 — CLI + formatter (Dia 27-30)

- [ ] `vex run` / `build` / `check` / `fmt` / `repl` / `new`
- [ ] Formatter opinativo (zero config) — `vex-fmt`
- [ ] Scaffold de projeto (`vex new`)

**Entregável:** Toolchain end-to-end usável.

---

## Fase 8 — Stdlib mínima (Dia 31-35)

```vex
// I/O
print, println, input, read_file, write_file
// Conversões
to_int, to_float, to_str
// Arrays
len, push, pop, map, filter
// Strings
split, join, trim, contains
```

**Entregável:** Exemplos usando stdlib compilam e rodam em Linux **e** Windows.

---

## Fase 9 — LSP MVP (Dia 36-40)

**Lição da Gleam — tooling de Dia 1 é diferencial.**

- [ ] Diagnostics inline
- [ ] Hover (mostra tipo)
- [ ] Go-to-definition
- [ ] Autocomplete básico

---

## Fase 10 — Testes integração + benchmarks (Dia 41-45)

- [ ] Snapshot tests (`insta`) para parser e typeck
- [ ] UI tests (programas inválidos → erro esperado)
- [ ] Benchmarks `hyperfine`: vex vs Rust vs Python vs Zig
- [ ] Fuzz corpus

---

## Fases futuras (v1.x+)

| Feature | Inspiração | Fase |
|---------|-----------|------|
| Region analysis (otimiza gen checks) | Vale | v1.1 |
| Generics monomorfizados | Rust | v1.2 |
| Pattern matching exaustivo | Rust/Roc | v1.2 |
| Comptime explícito | Zig | v1.3 |
| WASM target | wasm32-unknown-unknown | v1.4 |
| Async runtime opt-in | — | v2.0 |
| Package manager (`vex add`) | Cargo | v2.0 |

---

## Princípios de execução

1. **Não avançar fase sem testes verdes** na atual.
2. **`cargo test --workspace` antes de cada commit.**
3. **Documentar decisões não-óbvias em `docs/design/000X-*.md`** (ADR).
4. **Mensagens de erro são feature, não detalhe** — cada `TypeError`
   deve ter span + hint.
5. **Performance é claim — meça.** Benchmarks compõem o CI a partir da Fase 10.

---

*Vex Lang — criada por Jeff Almeida. v0.1 em desenvolvimento.*
