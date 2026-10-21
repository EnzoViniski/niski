# Skills do HyperFrames embutidas no editor de vídeo

Cópia parcial das skills oficiais do HyperFrames, para o agente do editor de vídeo (Claude ou Codex)
saber escrever e editar composições sem depender de instalação global.

- Origem: https://github.com/heygen-com/hyperframes, tag `v0.8.183`
  (commit `7e2817619f340f9302eeaef185e73be39a19858d`, 21/10/2026).
- Licença: Apache-2.0, texto completo em `LICENSE-HYPERFRAMES`. Copyright HeyGen.
- Mesma versão fixada no app (`VERSAO_DO_HYPERFRAMES` em `src-tauri/src/videos/projetos.rs`).

## O que entrou

| Pasta | Conteúdo |
|---|---|
| `hyperframes-core/` | inteira (contrato da composição, atributos `data-*`, faixas, mídia) |
| `hyperframes-animation/` | sem `examples/`, `blueprints/`, `blueprints-index.md` e `scripts/` |
| `hyperframes-audio/` | inteira, sem os arquivos de teste |
| `hyperframes-cli/` | `SKILL.md` e as referências de lint, preview/render, doctor e init (sem cloud, lambda e cloudrun) |

Nada foi alterado nos arquivos copiados. As instruções do editor (não usar render em nuvem, não
publicar, telemetria desligada, versão fixada) ficam no `CLAUDE.md`/`AGENTS.md` de cada projeto,
gerados em `src/features/video/modelos.ts`.

Ao criar ou abrir um projeto, o app copia esta pasta para `<projeto>/.claude/skills/` (clone do
APFS quando dá, sem ocupar espaço) e grava `.versao-medza` com a versão.
