# CLAUDE.md — Projeto Vex

Orientação para qualquer agente Claude que trabalhar neste repositório.
Curto, denso, atualizado a cada fase. Mantenha < 200 linhas.

## Resumo do projeto

**Vex** é uma linguagem de sistemas em desenvolvimento. Filosofia:
*rápida como C++, segura como Rust, legível como Python*. Compila para
binário nativo via LLVM. Autor: Jeff Almeida.

## Stack

- **Rust stable 1.95+** (pinned em `rust-toolchain.toml`)
- **LLVM 17** via `inkwell` 0.9 (env `LLVM_SYS_170_PREFIX=/usr/lib/llvm-17`,
  exposto em `.cargo/config.toml`)
- Lexer: `logos` 0.14 · Parser: hand-written · Diagnostics: `miette`
- Cross-compile Windows: `llvm-mingw` + target `x86_64-pc-windows-gnu`
- Dev em WSL2 Ubuntu 24.04

## Pipeline

```
source.vex → lex → parse → resolve → typeck → mir → [codegen] → binário
              ↓        ↓        ↓         ↓       ↓         ↓
          Tokens     AST      HIR     HIR tipada  CFG    LLVM IR + .exe/.bin
```

## Estrutura

```
crates/
├── vex-lexer        # logos, LexError, spans
├── vex-ast          # AST types + spans
├── vex-parser       # recursive descent + Pratt
├── vex-hir          # HIR + name resolution (2 passagens)
├── vex-typeck       # bidirectional simple, Ty + check_module
├── vex-mir          # CFG, basic blocks, terminators
├── vex-codegen      # LLVM IR via inkwell (Fase 6)
├── vex-driver       # orquestra pipeline; renderiza erros miette
├── vex-diagnostics  # wrappers miette
├── vex-cli          # binário `vex` (check, build, run, fmt, repl, new)
├── vex-fmt          # formatter (stub, Fase 7)
└── vex-lsp          # LSP (stub, Fase 9)
runtime/              # runtime nativo: gen refs, intrínsecos C ABI
docs/
├── spec.md           # spec da linguagem (atualize a cada fase)
├── grammar.ebnf      # gramática formal
└── design/000X-*.md  # ADRs por decisão arquitetural
examples/             # programas .vex de teste
tests/                # ui/integration/corpus
tools/setup-llvm-mingw.sh  # baixa toolchain cross-Windows
```

## Status por fase

Veja `Rodmap.md` para detalhes. Fases concluídas: 0, 1, 2, 3, 4, 5a, 6, 7.
5b: análise + emissão de drops para arrays prontas (ADR 0007 + 0008).
Sub-fases pendentes: gen-refs (5b.3), linear types (5b.4), ASAP drops
via dominator analysis (5b.7). Pendentes: 8 (stdlib), 9 (LSP),
10 (bench).

MVP end-to-end **funciona Python-like**: `vex run examples/hello.vex`
imprime "Hello, Vex!". Hello é uma única linha `println("Hello, Vex!")`.

## Princípio guia: ergonomia Python + segurança Rust + velocidade C++

Vex prioriza ergonomia Python em sintaxe (script mode, `let` opcional,
`def`/`class` aliases) **sem comprometer safety** (typeck rigoroso,
ownership analysis sobre CFG). Performance vem de LLVM 17 + zero GC.

## Comandos essenciais

```bash
cargo build --workspace                            # build full
cargo test --workspace                             # tests (97+ atualmente)
cargo clippy --workspace --all-targets -- -D warnings   # lint
./target/debug/vex check examples/fib.vex          # E2E até MIR
./target/debug/vex check examples/fib.vex --emit=mir    # pretty MIR
```

## Convenções de código

1. **Sem `unsafe`** exceto onde inevitável (FFI inkwell). Sempre com
   `// SAFETY:` justificando.
2. **Comentários apenas no porquê** de decisões não-óbvias. O quê = código.
3. **Erros estruturados com span.** Cada `*Error` enum implementa
   `thiserror::Error` e tem método `span() -> &Span`.
4. **Imutabilidade default.** `let mut` só quando preciso.
5. **`cargo fmt` + `clippy -D warnings`** antes de commitar.
6. **Sem allocação em hot paths.** Use `smol_str::SmolStr` para strings
   curtas (identifiers); `indexmap::IndexMap` quando ordem importa.

## Convenções de testes

- **Unitários inline** (`mod tests` em cada `src/lib.rs` ou módulo).
- **Snapshot tests** com `insta` para AST e tokens (`tests/snapshot.rs`).
- **Integração** com fontes reais de `examples/` (cobrem fib, hello, ponto).
- **Nunca silenciar erro** sem teste correspondente.

## Convenções de commit (PT-BR)

Formato: `tipo(escopo): assunto resumido em pt-br`. Sem assinatura
"Co-Authored-By Claude". Corpo descreve **o quê + porquê**, não
narrativa. Exemplos:

```
feat(parser): implementar Fase 2 com recursive descent + Pratt
fix(lexer): tratar escape \r em strings literais
docs(spec): refletir Fase 5a no resumo de estado
```

## Convenções de ADR

Cada decisão arquitetural não-óbvia vira ADR em `docs/design/000X-*.md`:
- **Status:** Proposto / Aceito / Substituído
- **Contexto:** o problema
- **Decisão:** escolha + por quê
- **Trade-offs aceitos**

ADRs existentes: 0001 (architecture), 0002 (parser), 0003 (resolve),
0004 (typeck), 0005 (mir).

## Erros e diagnósticos

- Todo erro carrega span (byte range no fonte).
- `vex-driver` renderiza via `miette` com `NamedSource` + label + hint
  contextual (ver `*_hint` em `vex-driver/src/lib.rs`).
- Cada nova categoria de erro deve ter hint útil.

## Ownership (estado atual)

Não implementado ainda. Plano em `0001-architecture.md`:
- *Generational references* (Vale-style) — Fase 5b
- *Linear types* opcionais (Austral-style) — Fase 5b
- *ASAP destruction* (Mojo-style) — Fase 5b

Por enquanto programas compilam sem checks de ownership.

## Built-ins reconhecidos (até stdlib formal — Fase 8)

`print`, `println`, `input`, `read_file`, `write_file`, `to_int`,
`to_float`, `to_str`, `len`, `push`, `pop`, `sqrt`, `abs`, `min`, `max`.

Assinaturas em `crates/vex-typeck/src/ty.rs::builtin_signature`.

## Licença

MIT **com cláusula de atribuição obrigatória** (ver `LICENSE`). Não é
o dual MIT-OR-Apache padrão do ecossistema Rust. Workspace usa
`license-file = "LICENSE"` (não SPDX).

## Antes de começar uma nova fase

1. Ler ADR da fase anterior em `docs/design/`.
2. Atualizar `Rodmap.md` marcando fase atual como em curso.
3. Implementar com testes desde o início (não no fim).
4. Atualizar `docs/spec.md` se a linguagem ganhou sintaxe nova.
5. Criar ADR novo se a decisão for não-óbvia.
6. `cargo test --workspace` + `cargo clippy -D warnings` antes do commit.
7. Commit PT-BR sem assinatura. Mensagem descreve **o quê + porquê**.

## O que NÃO fazer

- **Não pré-otimizar.** MVP funcional > código elegante incompleto.
- **Não silenciar warnings.** Resolva ou suprima explicitamente com
  `#[allow(...)]` + comentário do porquê.
- **Não introduzir dependência nova sem motivo claro.** Cada crate
  externo é peso de manutenção.
- **Não duplicar AST/HIR/MIR types.** Cada IR tem propósito claro;
  se precisar de coisa nova, encaixe na IR certa.
- **Não rebatizar conceitos.** "DefId" e "LocalId" são fixos. Não
  invente sinônimos.

## Performance

Vex promete *C++-fast*. Premissas:
- LLVM cuida da maior parte (otimizações: loop vectorization, inlining).
- Codegen do MIR deve ser direto (sem indireções desnecessárias).
- Runtime mantém footprint mínimo (idealmente zero-cost para programas
  que não usam gen refs).

Quando estiver em dúvida sobre uma otimização, **benchmark primeiro**
(hyperfine), depois decida.

## Onde achar

- **Decisões arquiteturais:** `docs/design/*.md`
- **Próximas fases:** `Rodmap.md`
- **Spec da linguagem:** `docs/spec.md`
- **Gramática formal:** `docs/grammar.ebnf`
- **Como o usuário roda:** `README.md`
- **Política de segurança:** `SECURITY.md`
- **Como contribuir:** `CONTRIBUTING.md`
