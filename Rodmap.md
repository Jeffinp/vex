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

**Status:** ✅ Concluída.

Parser **hand-written** (recursive descent + Pratt para expressões),
estruturado em módulos: `cursor`, `ty`, `expr`, `stmt`, `item`, `error`.
Decisão técnica fundamentada em `docs/design/0002-parser-pratt.md`.

- [x] `Cursor` com peek/peek_n/bump/expect/eat sobre stream de tokens
- [x] `ParseError` estruturado (Unexpected, UnexpectedEof, InvalidExpr,
      InvalidType, InvalidPattern, Lex)
- [x] Items: fn (com `pub`/`comptime`/parâmetros/retorno), struct,
      impl, const, use (paths multi-segment)
- [x] Statements: let (`mut`/anotação opcional), return (com/sem
      valor), if/else/else-if-chain, while, for, break, continue,
      expression statement
- [x] Expressões via Pratt (BP table com 9 níveis + postfix)
- [x] Operadores: `+ - * / %`, `== !=`, `< > <= >=`, `&& ||`,
      unários `- !`, atribuição `=` (right-assoc)
- [x] Postfix: chamada `f(args)`, field access `obj.field`,
      method call `obj.method(args)`, index `arr[i]`
- [x] Atoms: literais (int/float/str/bool/char), ident, `self`,
      parênteses, array literal, struct literal, refs (`&` `&mut`),
      match, blocos
- [x] Patterns: int literal, bool, str, ident, wildcard (`_`),
      ranges (`1..10`, `1..=10`)
- [x] Heurística de desambiguação para struct literal (apenas se
      identificador começa com maiúscula)
- [x] 23 testes unitários + 3 snapshot tests (insta) sobre
      `examples/{hello,fib,ponto}.vex`
- [x] `cargo clippy -D warnings` verde

**Entregável:** `cargo test -p vex-parser` verde (26/26).

---

## Fase 3 — Resolução de nomes + HIR (Dia 9-10)

**Status:** ✅ Concluída.

Crate `vex-hir` ganha dois módulos: `hir` (tipos) e `resolve`
(algoritmo). Decisão arquitetural em `docs/design/0003-name-resolution.md`.

- [x] `DefId(u32)` opaco com tabela `defs: Vec<Def>`
- [x] `DefKind`: Fn / Struct / Const / Local / Param / SelfParam
- [x] HIR completo: items, stmts, exprs, types, patterns
- [x] Algoritmo de duas passagens (collect items → resolve bodies)
- [x] Forward references (a chama b antes de b ser declarada)
- [x] Pilha de escopos lexicais (`Vec<IndexMap<SmolStr, DefId>>`)
- [x] Shadowing dentro de blocos
- [x] Built-ins reconhecidos (print, println, len, sqrt, etc.) como
      placeholder até a Fase 8 (stdlib)
- [x] Acumula erros (não aborta no primeiro) para `vex check` reportar
      tudo de uma vez
- [x] 7 variantes de `ResolveError` com span: Unknown, Duplicate,
      UnknownType, UnknownStruct, SelfOutsideMethod, ImplOnUnknownType,
      InvalidAssignTarget
- [x] Integrado no `vex-driver`: pipeline lex → parse → resolve
- [x] Hints contextuais em cada erro renderizado via `miette`
- [x] 14 testes unitários cobrindo: forward refs, shadowing, duplicate
      detection, unknown vars/types/structs, self outside method,
      invalid assign target, impl resolution, builtin recognition

**Entregável:** `vex check examples/*.vex` reporta "parsing + resolução OK";
programas com nomes não-declarados mostram diagnóstico colorido com hint.

---

## Fase 4 — Type checker (Dia 11-15)

**Status:** ✅ Concluída.

Estratégia: **bidirecional simples + inferência local** (não HM completo).
Fundamentação em `docs/design/0004-typeck.md`.

- [x] Crate `vex-typeck` com 3 módulos: `ty`, `env`, `check`
- [x] `Ty` enum + `Ty::Any` (built-ins poly) + `Ty::Error` (propagação)
- [x] `Env` pré-computa fn sigs, struct fields, methods (chave
      `(struct_id, name)`)
- [x] Inferência bottom-up para expressões livres
- [x] Top-down quando há tipo esperado (anotações, retornos, args)
- [x] Validação de:
      - operadores binários por tipo (`+`/`-`/`*`/`/`/`%` numéricos,
        `==`/`!=` igualdade estrutural, `<`/`>`/`<=`/`>=` ordem numérica,
        `&&`/`||` bool)
      - operadores unários (`-` numérico, `!` bool)
      - retorno vs assinatura
      - aridade + tipos de argumentos em chamadas
      - acesso a campos (existência + tipo)
      - struct literals (campos faltando/extras + tipos)
      - method dispatch via `(struct_id, name)`
      - condições de `if`/`while` (bool)
      - índice de array (int) + indexação só de arrays
      - elementos homogêneos em array literals
      - atribuição (lhs e rhs compatíveis)
- [x] Tabela de built-ins (`print`, `println`, `len`, `sqrt`, etc.)
- [x] 15 variantes de `TypeError` com span + hints contextuais
- [x] Integrado no driver: pipeline lex → parse → resolve → typeck
- [x] 22 testes unitários cobrindo todas as variantes de erro +
      exemplos válidos

**Entregável:** `vex check examples/fib.vex` → "lex + parse + resolve +
typeck OK"; programas com erros de tipo reportam mensagens coloridas
com hints.

---

## Fase 5a — MIR (CFG) (Dia 16-19)

**Status:** ✅ Concluída.

Crate `vex-mir` ganha implementação completa de lowering HIR → MIR.
Decisão arquitetural em `docs/design/0005-mir-cfg.md`.

- [x] Tipos: `LocalId`, `BlockId`, `MirFn`, `BasicBlock`, `Statement`,
      `Rvalue`, `Operand`, `Place`, `Projection`, `Callee`, `Terminator`
- [x] Lowering recursivo HIR → MIR via `FnLowerer`
- [x] Operandos atômicos (sem cálculo embutido); `Rvalue` cobre BinaryOp,
      UnaryOp, Call, Field, Index, Ref, StructInit, ArrayInit
- [x] Control flow lowered para CFG: if (then/else/join), while
      (head/body/exit com back edge), for (desugar `i = 0; while i < len`)
- [x] `break`/`continue` via pilha `loop_targets`
- [x] Pretty printer (`pretty_print_module`)
- [x] CLI: `vex check <arq> --emit=mir` imprime o CFG textualmente
- [x] Driver integrado: pipeline lex → parse → resolve → typeck → mir
- [x] 10 testes unitários cobrindo empty, simple fn, let+return,
      if branches, while back edge, fib, ponto+impl, pretty print,
      call rvalue, struct init

**Entregável:** MIR pronto para alimentar codegen na Fase 6.

## Fase 5b — Ownership analysis (a fazer)

**Status:** ⏳ Pendente. Split feito porque depende do MIR.

- [ ] Last-use analysis sobre CFG (ASAP destruction)
- [ ] Generational reference checks insertion
- [ ] Linear type validation (resources marcados)
- [ ] Move/use-after-move detection

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
