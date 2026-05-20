# Vex Lang

> Linguagem de programação **rápida como C++, segura como Rust, legível como Python**.

[![CI](https://github.com/jeffalmeida/vex/actions/workflows/ci.yml/badge.svg)](https://github.com/jeffalmeida/vex/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.85+-orange.svg)](rust-toolchain.toml)

**Vex** é uma linguagem de programação de sistemas em desenvolvimento ativo.
Compila para binário nativo (Linux e Windows) via LLVM, com ownership
inspirado em Vale + Austral + Mojo — sem o ônus do borrow checker do Rust.

---

## Criador

**Vex foi criada e é mantida por [Jeff Almeida](mailto:jefersonreisalmeida8356@gmail.com).**

Forks, contribuições e redistribuições são bem-vindas sob a [licença MIT](LICENSE),
**desde que o crédito ao criador original seja preservado** — em README, tela
"sobre", ou documentação do projeto derivado. Ver [`LICENSE`](LICENSE) para
o texto completo da cláusula de atribuição.

---

## Filosofia

1. **Performance** — LLVM 17 backend, zero GC, ASAP destruction.
2. **Safety** — ownership híbrido: *generational references* (Vale) +
   *linear types* opcionais (Austral) para recursos críticos.
3. **Ergonomia** — sintaxe enxuta, inferência local, mensagens de erro
   excepcionais.
4. **Sem fluxo de controle escondido** (princípio do Zig) — se não parece
   uma chamada, não é. Sem exceptions implícitas, sem alocações invisíveis.
5. **Cross-platform** — Linux e Windows desde a v0.1; WASM em v1.4+.

Fundamentação técnica completa em [`docs/design/0001-architecture.md`](docs/design/0001-architecture.md).

---

## Sintaxe (sneak peek)

```vex
struct Ponto {
    x: float,
    y: float,
}

impl Ponto {
    fn distancia(self, outro: Ponto) -> float {
        let dx: float = self.x - outro.x
        let dy: float = self.y - outro.y
        return sqrt(dx * dx + dy * dy)
    }
}

fn main() -> void {
    let a: Ponto = Ponto { x: 0.0, y: 0.0 }
    let b: Ponto = Ponto { x: 3.0, y: 4.0 }
    println(a.distancia(b))   // 5.0
}
```

---

## Status

🎉 **MVP end-to-end funcional.** `vex run examples/fib.vex` → `55`.

| Fase | Status | Descrição |
|------|--------|-----------|
| 0    | ✅     | Scaffold do workspace |
| 1    | ✅     | Lexer (logos) |
| 2    | ✅     | Parser hand-written + Pratt |
| 3    | ✅     | Name resolution + HIR |
| 4    | ✅     | Type checker bidirecional |
| 5a   | ✅     | MIR (CFG) |
| 5b   | ⏳     | Ownership analysis (gen refs, linear types) |
| 6    | ✅     | Codegen LLVM + linker |
| 7    | ⏳     | Formatter + scaffold de projeto |
| 8    | ⏳     | Stdlib formal |
| 9    | ⏳     | LSP MVP |
| 10   | ⏳     | Integration tests + benchmarks |

## Demo

```bash
$ ./target/debug/vex run examples/hello.vex
Hello, Vex!

$ ./target/debug/vex run examples/fib.vex
55

$ ./target/debug/vex run examples/ponto.vex
5.0

$ ./target/debug/vex run examples/array.vex
5
10
50
150
```

---

## Construção

### Pré-requisitos (uma vez)

```bash
# WSL2 Ubuntu (ou Linux nativo)
sudo apt-get install -y llvm-17-dev libpolly-17-dev clang-17 lld-17
echo 'export LLVM_SYS_170_PREFIX=/usr/lib/llvm-17' >> ~/.bashrc

# Rust 1.85+
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Cross-compile para Windows (opcional)
./tools/setup-llvm-mingw.sh
rustup target add x86_64-pc-windows-gnu
```

### Build & test

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

### Cross-build para Windows (gera `.exe`)

```bash
cargo build --release --target x86_64-pc-windows-gnu -p vex-cli
# binário: target/x86_64-pc-windows-gnu/release/vex.exe
```

---

## Arquitetura

Pipeline em IRs separados (AST → HIR → MIR → LLVM IR), padrão rustc/Swift:

```
source.vex
  → vex-lexer    (Tokens)
  → vex-parser   (AST)
  → name res     (HIR)
  → vex-typeck   (HIR tipada)
  → lowering     (MIR — ownership/CFG)
  → vex-codegen  (LLVM IR → .o)
  → lld          (binário ou .exe)
```

Detalhes em [`CONTRIBUTING.md`](CONTRIBUTING.md) e
[`docs/design/0001-architecture.md`](docs/design/0001-architecture.md).

---

## Segurança

Vulnerabilidades de segurança **não** devem ser reportadas em issues
públicas. Ver [`SECURITY.md`](SECURITY.md) para o processo de disclosure
coordenado.

E-mail privado: 📧 **jefersonreisalmeida8356@gmail.com**

Princípios:
- Sem `unsafe` Rust no compilador exceto onde inevitável (FFI inkwell),
  sempre com comentário `// SAFETY:` justificando.
- CI roda `clippy -D warnings`.
- Fuzzing do parser e typeck planejado (Fase 10).
- Toolchain `llvm-mingw` baixada de release oficial — checksums em roadmap.
- Validação rigorosa de input no LSP (JSON-RPC).

---

## Contribuir

Ver [`CONTRIBUTING.md`](CONTRIBUTING.md). Regras-chave:

1. Preservar atribuição ao criador (Jeff Almeida) em qualquer derivação.
2. Não avançar fase sem testes verdes na atual.
3. Decisões arquiteturais → ADR em `docs/design/`.
4. Mensagens de erro são **feature**, não detalhe.
5. `cargo fmt` + `cargo clippy -D warnings` antes de commitar.

---

## Licença

[MIT](LICENSE) com cláusula de atribuição obrigatória.

```
Copyright (c) 2026 Jeff Almeida
```

Você pode usar, copiar, modificar, mesclar, publicar, distribuir,
sublicenciar e vender cópias — **desde que** o crédito ao criador
original seja preservado de forma visível (README, "sobre", ou docs).

---

## Agradecimentos

Vex bebe de pesquisa e ideias de várias linguagens:
- **Rust** — modelo de ownership como ponto de partida.
- **Vale** — generational references.
- **Austral** — linear types simples.
- **Mojo** — ASAP destruction.
- **Zig** — "no hidden control flow", comptime.
- **Gleam** — tooling de Dia 1.
- **Ruff** — parser hand-written como decisão consciente.

---

*Vex Lang — criada por **Jeff Almeida**. v0.1 em desenvolvimento.*
