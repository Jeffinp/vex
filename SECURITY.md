# Política de Segurança — Vex

## Versões suportadas

Vex está em desenvolvimento ativo (v0.1). Apenas a branch `main`
recebe correções de segurança até a primeira release estável.

| Versão | Suportada |
|--------|-----------|
| main   | ✅        |
| v0.x   | ✅ (atual)|

## Reportando vulnerabilidades

**Não abra issues públicas para vulnerabilidades de segurança.**

Reporte via e-mail privado para:

📧 **jefersonreisalmeida8356@gmail.com**

Inclua:
1. Descrição da vulnerabilidade.
2. Passos para reproduzir (PoC quando possível).
3. Impacto estimado (severidade, superfície afetada).
4. Sugestão de mitigação (se houver).

Compromisso de resposta:
- Confirmação de recebimento: **até 72h**.
- Avaliação inicial: **até 7 dias**.
- Correção + disclosure coordenado: depende da severidade.

## Escopo

Cobertos por esta política:
- Bugs do compilador (`vex-*` crates) que permitam execução arbitrária,
  bypass de checks de ownership/linear types em código *válido*, ou
  geração de código LLVM IR malformado a partir de fonte *válido*.
- Vulnerabilidades no runtime (`vex-runtime`) que permitam corrupção de
  memória em programas Vex bem-formados.
- Falhas em scripts de build/CI (`tools/`, `.github/`) que comprometam
  a supply chain.

Fora de escopo:
- Bugs em código *Vex de usuário* que use `unsafe` (quando introduzido).
- Vulnerabilidades em dependências upstream (reporte ao projeto upstream).
- DoS por consumo de recursos em programas adversariais sem trust boundary.

## Hardening em desenvolvimento

- Fuzzing do parser e typeck (`tests/corpus/`) — Fase 10.
- Validação rigorosa de input no LSP (parsing de JSON-RPC).
- Sem `unsafe` Rust no compilador exceto onde inevitável (FFI inkwell).
  Cada bloco `unsafe` deve ter comentário `// SAFETY:` justificando.
- CI roda `cargo clippy -D warnings` + `cargo audit` (a adicionar).

## OWASP / supply chain

- Dependências pinadas no `Cargo.lock` (a ser commitado quando v0.1 sair).
- Toolchain `llvm-mingw` baixada de release oficial do mantenedor
  (Martin Storsjö). Checksum a ser adicionado em `tools/setup-llvm-mingw.sh`.
