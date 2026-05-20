# Contribuindo com Vex

Vex está em construção ativa. Contribuições são bem-vindas, mas sigam
os princípios abaixo.

## Como está construído

Vex é um compilador modular escrito em **Rust 1.85+**, usando **LLVM 17**
via `inkwell` para geração de código. Detalhes arquiteturais completos
em [`docs/design/0001-architecture.md`](docs/design/0001-architecture.md).

### Pipeline

```
source.vex
  → vex-lexer   (Tokens via logos)
  → vex-parser  (AST via recursive descent + Pratt)
  → vex-hir     (HIR pós resolução de nomes)
  → vex-typeck  (HIR tipada, inferência local)
  → vex-mir    (MIR com ownership lowered)
  → vex-codegen (LLVM IR via inkwell)
  → lld         (binário nativo ou .exe)
```

### Crates

| Crate              | Responsabilidade |
|--------------------|------------------|
| `vex-lexer`        | Tokenização (logos) |
| `vex-ast`          | Tipos da AST + spans |
| `vex-parser`       | Parsing hand-written |
| `vex-hir`          | High-level IR |
| `vex-typeck`       | Type check + inferência |
| `vex-mir`          | Mid-level IR + ownership |
| `vex-codegen`      | Emissão LLVM IR |
| `vex-driver`       | Orquestra o pipeline |
| `vex-diagnostics`  | Erros formatados (miette) |
| `vex-cli`          | Binário `vex` |
| `vex-fmt`          | Formatter opinativo |
| `vex-lsp`          | Language server |
| `vex-runtime`      | Runtime nativo (gen refs) |

## Ambiente de desenvolvimento

**Requisitos:**
- WSL2 Ubuntu (ou Linux nativo)
- Rust 1.85+ via `rustup`
- LLVM 17 (`sudo apt-get install llvm-17-dev`)
- Para cross-Windows: `./tools/setup-llvm-mingw.sh`

**Build & test:**
```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all -- --check
```

## Regras de contribuição

1. **Atribuição obrigatória.** Vex é licenciada sob MIT **com requisito
   explícito de crédito** ao criador original (Jeff Almeida). Ver
   [`LICENSE`](LICENSE). Forks e derivações devem preservar este crédito.
2. **Sem avançar fase sem testes verdes.** Cada fase do roadmap tem
   entregável claro. Não abra PR mudando duas fases.
3. **Decisões não-óbvias → ADR.** Crie `docs/design/000X-titulo.md`
   antes de implementar mudanças arquiteturais.
4. **Mensagens de erro são feature.** Cada erro deve ter span + hint.
5. **Sem `unsafe` Rust** sem justificativa em comentário `// SAFETY:`.
6. **`cargo fmt` + `cargo clippy -D warnings` antes de commitar.**

## Pull requests

- Branch a partir de `main`.
- Commits pequenos, mensagens no formato Conventional Commits.
- PR description deve explicar **por que**, não só **o que**.
- Linkar issue/ADR relevante.

## Estilo

- Identificadores semânticos. Sem abreviações que não sejam padrão da
  área (`tok`, `ast`, `ir` são OK; `prsTk` não).
- Comentários apenas no **porquê** de decisões não-óbvias. O **o quê**
  é o código.
- Funções pequenas, responsabilidade única. Prefira composição.
- Imutabilidade por default.

## Reporte de segurança

Ver [`SECURITY.md`](SECURITY.md). **Não** abra issue pública para vulnerabilidades.
