# ADR 0001 — Arquitetura do Compilador Vex

**Data:** 2026-05-19
**Status:** Aceito
**Autor:** Jeff Almeida

## Contexto

Vex é uma linguagem nova. Antes de escrever código precisamos fixar
decisões arquiteturais fundamentadas em pesquisa do estado da arte (2026).

## Decisões

### 1. Backend de codegen: LLVM via `inkwell` 0.9+

- LLVM continua imbatível em otimização de código (loop vectorization,
  inter-procedural). Vex quer "rápido como C++" — descartar LLVM elimina
  essa promessa.
- Cranelift descartado: production-ready apenas em WASM/JIT (wasmtime).
  AOT é experimental — caso Perry/TypeScript→Cranelift→LLVM mostra que
  Cranelift não escala para AOT genérico.
- QBE descartado: sem suporte Windows nativo, adoção niche.

### 2. Cross-compilation Linux→Windows via `llvm-mingw`

- Dev em WSL2 Ubuntu, mas Vex precisa produzir `.exe` para usuários
  finais. `llvm-mingw` (mantida por Martin Storsjö) fornece toolchain
  LLVM+Clang+LLD pré-construída.
- Target Rust: `x86_64-pc-windows-gnu` (não MSVC — evita VS Build Tools).
- UCRT (não MSVCRT) — runtime C moderno do Windows 10+.

### 3. Ownership: híbrido Vale + Austral + Mojo

- Baseline: *generational references* (Vale) — 10.84% overhead vs unsafe,
  permite mutable aliasing (sem `Rc<RefCell<>>` workaround do Rust).
- Linear types opt-in para recursos críticos (file, socket, lock) — estilo
  Austral. Borrow checker linear em ~600 linhas.
- ASAP destruction (Mojo) — drops no último uso, não fim de escopo. Tail
  recursion sem destrutores pendentes.
- Descartado: Rust borrow checker puro (curva de aprendizado defeats
  ergonomia), Hylo value-only (rígido demais para systems code).

### 4. Parser hand-written (recursive descent + Pratt)

- Ruff migrou de gerador → hand-written em v0.4 (2024) e ganhou em
  velocidade e qualidade de erros. rustc também é hand-written.
- LALRPOP descartado: gramática unambiguous é meta, não requisito.
  Hand-written dá controle total de error recovery e mensagens.
- Lexer: `logos` 0.14 (padrão do ecossistema Rust).

### 5. Diagnósticos via `miette`

- Suporte a modo de acessibilidade, runtime switching, ecossistema
  crescente (Pixi migrou em 2024).
- Alternativa `ariadne` rejeitada: apenas gráfico, sem accessibility.
- `codespan-reporting` descartado: sem manutenção desde 2021.

### 6. Pipeline em IRs separados (AST → HIR → MIR → LLVM IR)

- Inspirado em rustc/Swift. Cada IR tem propósito claro:
  - **AST** (`vex-ast`): saída direta do parser.
  - **HIR** (`vex-hir`): pós resolução de nomes, `DefId`s únicos.
  - **MIR** (`vex-mir`): pós typeck, ownership lowering (gen refs check,
    linear validation), CFG.
  - **LLVM IR** (`vex-codegen`): emissão final.
- Trade-off: mais código, mais cerimônia. Ganho: cada fase é testável,
  optimizable, e refatorável independentemente. Compiladores que tentam
  unificar IRs viram bagunça em larga escala (lição do GCC antigo).

### 7. Tooling de Dia 1

- Lição da Gleam: sucesso vem do tooling, não de features.
- `vex fmt` (Fase 7), `vex-lsp` (Fase 8) são prioridade — não opcionais.
- Formatter opinativo (zero config, estilo `gofmt`/`rustfmt`).

## Consequências

Positivas:
- Promessa de "C++-fast" sustentada por LLVM.
- Cross-Windows resolvido com toolchain estabelecida.
- Ownership com curva de aprendizado menor que Rust.
- Mensagens de erro como vantagem competitiva.

Negativas:
- Inkwell pré-v1.0 → breaking changes possíveis. Mitigação: pin de versão,
  revisão trimestral.
- LLVM compila devagar. Mitigação: incremental compilation (v1.1+),
  caching de IR.
- Generational references têm overhead em hot paths. Mitigação: static
  region analysis em fases futuras, linear types para hot paths.
- Parser hand-written é mais código que LALRPOP. Mitigação: aceito —
  controle de erros vale.
