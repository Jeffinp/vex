# ADR 0006 — Codegen LLVM via inkwell

**Data:** 2026-05-20
**Status:** Aceito (Fase 6 implementada, MVP)
**Autor:** Jeff Almeida

## Contexto

Fim do pipeline: transformar MIR validado em binário nativo. Decidido
em ADR 0001 que o backend é LLVM via `inkwell` 0.9 + LLVM 17 — agora
toca **implementar**.

## Decisão

Codegen direto MIR → LLVM IR, com **arquivo objeto** intermediário
linkado por `clang`/`gcc`/`mingw-gcc` como subprocess. Runtime mínimo
em Rust (`vex-runtime`) linkado estático.

### Estrutura

```
crates/vex-codegen/src/
├── lib.rs       # API pública: compile_module, link_object, opções
├── compile.rs   # MIR → LLVM IR via Codegen state struct
└── link.rs      # invoca clang/mingw-gcc para produzir binário
runtime/src/
└── lib.rs       # externs C ABI: vex_print_*, vex_println_*, vex_sqrt, ...
```

### Mapeamento de tipos

| Vex Ty        | LLVM IR               |
| ------------- | --------------------- |
| Int           | i64                   |
| Float         | double                |
| Bool          | i1                    |
| Char          | i32                   |
| Str           | ptr (i8\*)            |
| Void          | void (em return)      |
| Struct(id)    | %vex\_<Name> (named)  |
| Array(\_)/Ref | ptr (placeholder MVP) |
| Any/Error     | ptr                   |

### Algoritmo

1. **Declara structs** (passe de duas etapas: opaque stub + body) para
   permitir referências recursivas.
2. **Declara fns** (`vex_fn_<id>`; exceto `main` que mantém o nome).
3. **Para cada fn**:
   - bloco entry com `alloca` para todos os locals
   - stores dos parâmetros recebidos
   - cria todos os blocos do CFG antecipadamente (forward jumps)
   - compila cada `BasicBlock`: statements → terminator
4. **Verifica IR** (`module.verify()`) — pega bugs do codegen cedo.
5. **Escreve `.o`** via `TargetMachine::write_to_file(FileType::Object)`.
6. **Linker** invoca `clang`/`gcc`/`mingw-gcc` com `.o + libvex_runtime.a`.

### Method dispatch

`Callee::Method { struct_id, name }` resolve em codegen via tabela
`method_lookup: (struct_id, name) → fn_id` populada a partir de
`MirModule::methods`. Chamada direta — sem vtable, sem auto-deref
complexo (typeck garante consistência).

### Built-ins

Tabela em `compile_builtin`. Dispatch por **tipo do primeiro argumento**:

| Vex              | Runtime extern      |
| ---------------- | ------------------- |
| `print(int)`     | `vex_print_int`     |
| `println(int)`   | `vex_println_int`   |
| `print(bool)`    | `vex_print_bool`    |
| `println(bool)`  | `vex_println_bool`  |
| `print(float)`   | `vex_print_float`   |
| `println(float)` | `vex_println_float` |
| `print(str)`     | `vex_print_str`     |
| `println(str)`   | `vex_println_str`   |
| `sqrt(float)`    | `vex_sqrt`          |
| `abs(float)`     | `vex_abs_f64`       |
| `abs(int)`       | `vex_abs_i64`       |

Floats são impressos com debug formatter (`{x:?}`) para preservar `.0`
em valores inteiros (5.0 → "5.0", não "5").

### Cross-compile Windows

`CodegenOptions::target_triple = Some("x86_64-pc-windows-gnu")` faz
LLVM emitir `.o` com layout Windows; linker selecionado em `link.rs`
vira `x86_64-w64-mingw32-gcc -static`. Toolchain via
`tools/setup-llvm-mingw.sh`. **Não testado E2E ainda** — fica para
hardening pós-MVP.

### Driver

Pipeline completo:

```
source → lex → parse → resolve → typeck → mir → codegen .o → link
```

`vex run` compila + executa + limpa binário.
`vex build` deixa binário no diretório.

## Limitações conscientes do MVP

- **Arrays** mapeados para ptr — `[T]` ainda não tem layout próprio.
  ArrayInit/Index retornam placeholder zero.
- **Match** stub no MIR → codegen ignora; jump table fica para
  decision-tree lowering pós-MVP.
- **Cross-Windows não testado E2E.** Fluxo deve funcionar mas não há
  CI rodando o `.exe` resultante.
- **Codegen de assignment via Store em Place complexo** suporta apenas
  field access. Index/Deref como projection retornam erro.
- **Sem auto-deref para method receivers.** `&P` chamando método de
  `P` funciona porque a sig do método receberia `&P` no recv; mas
  promoção de `P` para `&P` em call sites não está implementada.
- **Construção de struct** materializa via alloca + GEP + store + load
  — pode ser substituído por aggregate insert (`insertvalue`) em
  passada de otimização posterior.

## Próximos passos

- Arrays primeira-classe (layout `{ ptr, len }`).
- Match decision-tree lowering (algoritmo de Maranget).
- Cross-Windows CI verificando `.exe` runs.
- Otimizações LLVM ajustáveis (`-O3` vs `-Os` vs LTO).
- Fase 5b: ownership analysis injeta gen-ref checks aqui antes do codegen.
