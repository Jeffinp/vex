# Vex — Especificação da Linguagem (v0.1 draft)

> Documento vivo. Atualizado a cada fase do roadmap.

## Filosofia

Vex é uma linguagem de sistemas que busca **rápida como C++, segura como Rust, legível como Python**.

Três princípios não-negociáveis:

1. **Sem fluxo de controle escondido** (princípio do Zig).
   Se não parece uma chamada de função, não é. Sem operator overloading
   surpresa, sem exceptions implícitas, sem alocações invisíveis.
2. **Ergonomia primeiro**. Tipos inferidos onde possível, sintaxe enxuta,
   mensagens de erro extraordinárias (estilo Elm/Rust).
3. **Custo zero quando possível**. Ownership simplificado via *generational
   references* (estilo Vale) + *linear types* opcionais para recursos
   (estilo Austral). Sem GC.

## Estado atual (Fase 7 done + 5b infra + ergonomia Python)

- ✅ **Lexer** completo, aceita `def`/`class` como aliases de `fn`/`struct`.
- ✅ **Parser** com **script mode**: top-level stmts viram `main()` implícito.
- ✅ **Name resolution**: primeira atribuição declara automaticamente
  (auto-`let`, Python-style).
- ✅ **Type checker** bidirecional.
- ✅ **MIR (CFG)** com liveness e **ownership analysis** (last-use
  refinado por statement, drop points, use-after-move conservador).
- ✅ **Codegen LLVM** via inkwell 0.9 + LLVM 17: `vex run` produz binário.
- ✅ **Formatter** opinativo emite forma canonical Python-friendly.
- ✅ **Arrays heap-allocated** com layout `{ ptr, len }` (16B fat pointer).
  Runtime: `vex_array_alloc` / `vex_array_drop`. `len(xs)` builtin.
- ✅ **Drop emission real**: `Statement::Drop` injetado no MIR via
  ownership pass; codegen emite `vex_array_drop` antes de cada Return.
- ✅ **4 exemplos rodam:** `hello`, `fib`, `ponto`, `array` (novo).
- ⏳ Próximo: bounds check, push/pop, gen-ref tags (Vale), linear
  types (Austral), methods dentro de `class`.

## Sintaxe (informal, atualizada conforme parser evolui)

### Hello, Vex! (script mode)

```vex
println("Hello, Vex!")
```

Top-level statements são executados em ordem como corpo de um `main()`
implícito. Sem necessidade de declarar `fn main()`. Idêntico em
ergonomia ao Python.

### Declarações

```vex
// função — `def` é canonical (Python), `fn` é alias aceito
def nome(p1: T1, p2: T2) -> T3 { ... }
pub def nome(...) { ... }              // visível externamente
comptime def nome(...) { ... }         // executada em tempo de compilação

// dados — `class` é canonical (Python), `struct` é alias aceito
class Nome { field: T, ... }
pub class Nome { ... }

// impl
impl Nome {
    fn metodo(self, ...) -> ... { ... }
    pub fn estatico(...) -> Nome { ... }
}

// constante
const PI: float = 3.14159
pub const VERSION: str = "0.1"

// uso
use std::io::println
use std::math
```

### Variáveis

```vex
// Auto-declaração (Python-style): primeira atribuição declara.
// Tipo inferido, mutável por padrão.
x = 42
x = x + 1                    // assignment subsequente atualiza

// Formas explícitas continuam aceitas:
let y = 3.14                 // imutável, tipo inferido
let mut z: int = 0           // mutável, tipo anotado
let w: float = 2.5           // imutável, tipo anotado
```

Trade-off do auto-declare: typos viram declarações silenciosas
(igual Python). Análise futura de linter pode alertar.

### Controle de fluxo

```vex
if cond { ... } else { ... }
if cond { ... } else if outra { ... } else { ... }

while cond { ... }
for x in iter { ... }
break
continue

return val
return                       // sem valor (tipo void)

match val {
    0     => "zero",
    1..10 => "pequeno",       // pattern range
    _     => "outro",
}
```

### Expressões e operadores

Precedência (do mais forte ao mais fraco):

| Categoria              | Operadores                 |
|------------------------|----------------------------|
| Postfix                | `f(args)`  `obj.field`  `arr[i]` |
| Unário                 | `-x`  `!x`                 |
| Multiplicativo         | `*`  `/`  `%`              |
| Aditivo                | `+`  `-`                   |
| Comparação ordem       | `<`  `>`  `<=`  `>=`       |
| Igualdade              | `==`  `!=`                 |
| AND lógico             | `&&`                       |
| OR lógico              | `\|\|`                       |
| Atribuição             | `=`  (right-associative)   |

### Referências

```vex
&x         // borrow imutável
&mut x     // borrow mutável (linear se T: linear)
```

Tipos:

```vex
&T              // borrow imutável
&mut T          // borrow mutável
[T]             // array
fn(T1, T2) -> R // tipo de função
```

### Self em métodos

```vex
impl Ponto {
    fn distancia(self, outro: Ponto) -> float {
        self.x - outro.x   // `self` é expressão válida
    }
}
```

## Tipos primitivos

| Tipo  | LLVM IR | Notas |
|-------|---------|-------|
| int   | i64     | sempre 64-bit |
| float | double  | IEEE-754 64-bit |
| bool  | i1      | |
| str   | { ptr, len } | UTF-8, length-prefixed |
| char  | i32     | codepoint Unicode |
| void  | void    | tipo unit |

## Modelo de ownership

Em desenvolvimento. Implementação na Fase 5. Ver
`docs/design/0002-parser-pratt.md` (sintaxe) e o ADR futuro 0003
para semântica.

Resumo:
- Valores por default são **owned** (move semantics).
- Referências `&T` e `&mut T` validadas via *generational references*.
- Tipos marcados como `linear` (file, socket, lock) exigem consumo único.
- Destruição ASAP no último uso (não no final do escopo).

## Gramática

Ver `docs/grammar.ebnf` para a gramática formal completa, atualizada
com a Fase 2.

## ADRs

- `0001-architecture.md` — backend LLVM, ownership híbrido, tooling Dia 1
- `0002-parser-pratt.md` — recursive descent + Pratt parsing
- `0003-name-resolution.md` — HIR e algoritmo de duas passagens
- `0004-typeck.md` — type checker bidirecional + built-ins poly
- `0005-mir-cfg.md` — MIR como CFG; split em 5a (CFG) e 5b (ownership)
- `0006-codegen-llvm.md` — codegen via inkwell + linker subprocess
- `0007-ownership-and-python-ergonomics.md` — análise de ownership (5b
  infra) + ergonomia Python-like (script mode, auto-let, `def`/`class`)
- `0008-arrays-and-drops.md` — arrays heap-allocated `{ ptr, len }`
  + emissão real de `Statement::Drop` + `vex_array_drop` no runtime
