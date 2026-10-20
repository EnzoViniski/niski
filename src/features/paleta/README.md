# Paleta de comandos

`Cmd+K` no Mac; `Ctrl+K` nas demais plataformas. A combinação também aparece nas
Configurações → Atalhos. As preferências antigas com Ctrl são adaptadas em tempo de leitura.

- `useAcoesDaPaleta(grupo, acoes)` registra callbacks do próprio componente. Registros saem
  ao desmontar; os callbacks sempre usam o render atual. Publique ações contextuais aqui,
  reaproveitando o handler do botão, sem recriar operações na paleta.
- Projetos abrangem todos os workspaces. Conversas vêm de `/painel`, incluindo o histórico
  nativo das CLIs. Sessões usam `FOCUS_SESSION`, como a lateral. Histórico sem sessão abre
  o mesmo `ConversationDetail` da web; abrir não inicia uma CLI. A aba persiste no workspace.
- Arquivos usam o índice de `QuickOpen` e os mesmos ignores do projeto ativo.
- `? texto` busca conteúdo literal sem distinguir caixa. As demais seções usam fuzzy,
  ignorando acentos. Consultas antigas são descartadas e as buscas IPC são serializadas.

## Busca no engine

`POST /historico/buscar` recebe `{ texto, limite?, tempo_ms? }`. O texto vai no corpo,
sem conteúdo em logs ou URLs. A rota usa o índice e o cache nativo já aquecido pelo engine
(e por `/painel`), sem nova varredura de contas a cada tecla. Não aceita caminhos do cliente.

O leitor de Claude/Codex tem teto de 100 resultados, 1.500 ms, 64 MiB lidos e 1 MiB por linha.
O prazo é cooperativo por bloco de 8 KiB; a resposta HTTP/IPC tem ainda um timeout de 100 ms
além do prazo. Linhas grandes são puladas, JSON inválido é ignorado e a resposta informa
`parcial` ao atingir limites. Só falas de usuário/agente entram; ferramentas e contexto
injetado ficam fora. O trecho inclui contexto e a data do evento quando gravada pela CLI.

Testes: `npx vitest run src/features/paleta src/test/keyBindings.test.ts src/test/shortcuts.test.ts`;
Rust: filtro `busca_historico` nos pacotes omni-core/omni-engine (sempre com a trava do time).

## Integrações compartilhadas com outras frentes

- Navegação e layout: `src/App.tsx`, `src/components/Sidebar.tsx`,
  `src/features/workspace/{PaneView,WorkspaceTabBar,PainelVazio}.tsx`,
  `src/types/workspace.ts`, `src/services/workspaceService.ts`.
- Ações existentes: `src/features/files/QuickOpen.tsx`,
  `src/features/git/GitBranchStatus.tsx`, `src/features/orquestracao/FaixaDoOrquestrador.tsx`,
  `src/features/terminal/UsoDeTodasAsContas.tsx`.
- Atalhos e rótulos: `src/lib/{plataforma,shortcuts}.ts`, `src/components/SettingsView.tsx`,
  `src/features/files/MarkdownPane.tsx`, `src/features/history/HistoryView.tsx`,
  `src/features/terminal/keyBindings.ts`, `src/test/keyBindings.test.ts`,
  `src/web/{WebApp,TerminalEspelho}.tsx`.
- Engine/core: `crates/omni-engine/src/mobile.rs` (declaração de módulo e rota) e
  `crates/omni-core/src/lib.rs` (exportação do módulo).
