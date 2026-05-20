# ADR 0008 — Arrays heap-allocated + emissão de drops

**Data:** 2026-05-20
**Status:** Aceito (MVP funcional)
**Autor:** Jeff Almeida

## Contexto

Após ADR 0007 (ownership analysis), a infraestrutura de last-use e
drop-points existia mas não havia **o que dropar**: tudo em Vex era
stack ou `.rodata`. Esta sub-fase muda isso introduzindo o primeiro
tipo realmente heap-allocated da linguagem — **arrays** — e fecha o
loop emitindo `Statement::Drop` real no MIR + codegen LLVM.

## Decisões

### 1. Layout de array: fat pointer `{ ptr, len }`

```text
struct VexArray {
    ptr: i8*,    // 8 bytes
    len: i64,    // 8 bytes
}                // total: 16 bytes, passado por valor
```

Decisão técnica:
- **Mais simples** que `{ ptr, len, capacity }` estilo Rust — push/pop
  ficam para iteração futura (precisa realloc).
- **Passagem por valor** (16 bytes) é tão barata quanto cabeçalho de
  Rust `Vec<T>` — duas movs.
- **Elemento type** não entra no layout LLVM. Codegen consulta
  `Ty::Array(inner)` diretamente para calcular `elem_size` e GEPs.

Tabela de elementos suportados:

| Element  | `elem_size_bytes` |
|----------|-------------------|
| Int      | 8                 |
| Float    | 8                 |
| Bool     | 1                 |
| Char     | 4                 |
| Str/Ref/Struct/Array | 8 (pointer-sized) |

### 2. Runtime: alloc/drop com Rust GlobalAlloc

```rust
#[no_mangle] pub extern "C" fn vex_array_alloc(n_bytes: u64) -> *mut u8;
#[no_mangle] pub unsafe extern "C" fn vex_array_drop(ptr: *mut u8, n_bytes: u64);
```

- `vex_array_alloc(0)` retorna `NonNull::dangling()` — sentinela
  para arrays vazios sem chamar o allocator (Rust `Layout::array(_, 0)`
  é proibido).
- `vex_array_drop(ptr, n_bytes)` deve receber **o mesmo `n_bytes`**
  passado a `alloc` (contrato do `GlobalAlloc`). Codegen recompõe via
  `len * elem_size`.
- Falha de alocação chama `std::process::abort()`. Unwind/panic vem na
  Fase 6+.

### 3. Codegen: `Rvalue::ArrayInit` e `Rvalue::Index`

**ArrayInit**:
1. `nbytes_const = elem_size * len` (sempre constante — sabemos no
   compile time quantos elementos o literal tem).
2. `raw_ptr = vex_array_alloc(nbytes_const)`.
3. Para cada item, GEP no `raw_ptr` + store.
4. Monta `{ raw_ptr, len }` via `insertvalue` (evita alloca).

**Index**:
1. Carrega fat pointer do local.
2. `extractvalue 0` → raw_ptr.
3. GEP elem_llty raw_ptr idx → endereço do slot.
4. `load elem_llty` → elemento.

Sem bounds check no MVP. Adicionar em Fase 6+ junto com panic
unwind handlers.

### 4. `len` builtin: extract de campo

```rust
if name == "len" && arg is StructValue {
    return extractvalue arg, 1;  // campo .len do fat pointer
}
```

Mesma estrutura serve para futuro `cap`, `is_empty`, etc.

### 5. Drop emission: `Statement::Drop` + `inject_drops`

MIR ganha novo variante:

```rust
pub enum Statement {
    Assign { local, rvalue, span },
    Store { place, value, span },
    Drop { local: LocalId, span },   // NEW
    Nop,
}
```

`vex_mir::ownership::inject_drops(f, &drop_points)` insere
`Statement::Drop { local }` nos pontos calculados — chamado pelo
driver após `analyze_ownership`.

Codegen de `Drop`:
- **Array(_)**: extrai `(ptr, len)`, calcula `len * elem_size`,
  chama `vex_array_drop`.
- **Demais owning**: no-op por ora (strings = `.rodata`, structs sem
  campos heap = sem allocator).

### 6. Posicionamento conservador no MVP: drop em Return

A análise original ASAP (drop logo após `last_use`) **quebra em
loops**: se array é usado dentro do corpo do loop, drop seria emitido
toda iteração → double-free.

Solução MVP: **drop antes de cada `Terminator::Return`** para todos os
locais owning declarados na fn (exceto parâmetros — caller é dono).

```vex
xs = [1, 2, 3]
while i < len(xs) {     // xs usado dentro do loop
    total = total + xs[i]
    i = i + 1
}
return                   // ← drop xs aqui, uma vez só
```

Trade-off: deixa de ser ASAP. Em compensação:
- ✅ Nunca double-free
- ✅ Nunca leak (todo path-to-return libera)
- ✅ Não exige dominator analysis
- ❌ Sub-ótimo em fns longas (memória vive mais que necessário)

Iteração futura (v1.x): adicionar dominator analysis para distinguir
blocos no corpo de loop dos blocos pós-loop. Volta o ASAP onde for
seguro, mantém at-return como fallback.

### 7. Parâmetros não são dropados pelo callee

Decisão consciente: hoje **toda passagem é semanticamente "por
empréstimo"** porque o move analysis ainda é conservador. Caller
mantém ownership; callee só lê. Quando move analysis ficar precisa,
adicionamos opt-in para "consumes" no callee.

## Métricas pós-implementação

- **114 testes verdes** no workspace
- `cargo clippy -D warnings` limpo
- 4 exemplos rodam: `hello`, `fib`, `ponto`, **`array`** (novo)
- `examples/array.vex` exercita: literal `[10,20,30,40,50]`, `len()`,
  index `xs[i]`, mutação em loop, soma — sem leak ou double-free

## Limitações conhecidas (pós-MVP)

- **Sem bounds check** em `xs[i]` — index fora do range = UB.
- **Sem `push`/`pop`** — arrays são tamanho-fixo no literal.
- **Sem arrays de struct** testados E2E (memcpy multi-byte funciona
  por LLVM mas faltam exemplos).
- **`for x in xs`** lowerizou para `while` + index — funciona mas
  cada iteração toma uma cópia (não-Copy elements têm custo extra).
- **Drop conservador (at-return)** em vez de ASAP — corrigido em
  iteração futura com dominator analysis.

## Próximos passos

- Bounds check opt-in (flag `--check-bounds`).
- `push`/`pop` com realloc no runtime (capacity field).
- Strings heap-allocated (cstrings dinâmicas de `input()`/`read_file()`).
- Dominator analysis → ASAP drops onde provavelmente seguro.
- `Box<T>` (heap-allocated única alocação) para tipos grandes.
