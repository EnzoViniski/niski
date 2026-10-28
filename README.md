# Niski

Workspace desktop para trabalhar com agentes de IA: Claude Code, Codex, Cursor e Antigravity lado a lado, com terminais, chat, projetos, histórico, voz local e acesso pelo celular no mesmo aplicativo.

Versão atual: **0.34.0** (arquivo `VERSION`). As novidades de cada versão aparecem em **Configurações → Sobre** e em [`src/lib/changelog.ts`](src/lib/changelog.ts).

## O que faz

- **Agentes em painéis e abas**: cada agente roda pela CLI oficial do provedor, com a conta da própria pessoa. Vários workspaces, projetos na lateral, editor, arquivos e Git (com grafo de commits).
- **Conversa**: visão de chat sobre a sessão da CLI, com aprovações, anexos, lista de tarefas, mini-diff das edições, plano, troca de modelo e de agente no meio da conversa e retomada com `--resume`.
- **Histórico protegido**: índice com snapshots e arquivo local dos transcripts de Claude e Codex.
- **Voz local**: ditado com Whisper compilado no app (whisper.cpp), sem chave de API e sem enviar áudio para fora. Assistente por voz (Jarvis) com orbe de partículas.
- **Orquestração**: fluxos visuais, orquestrador que delega para outros agentes, revisão de código por agente, agendamentos, worktrees isoladas e limite de sessões.
- **Rotina**: tarefas do dia, semana, integração com Google Agenda e agente da rotina também pelo Telegram.
- **Estudo e criação**: roadmaps de estudo, cadernos com IA ao lado e editor de vídeo.
- **Celular**: acesso remoto opcional pela rede privada do Tailscale, com pareamento por código e versão web própria.
- **Contas e uso**: fila de contas, consumo de 5 horas e semanal por conta, modelos de agentes e skills.
- **Atualização automática** assinada, com backup do estado e retomada das abas.

## Arquitetura

| Parte | Tecnologia | Onde |
|---|---|---|
| Interface desktop e web do celular | React 18, TypeScript, Vite, Tailwind | `src/` |
| App desktop | Tauri 2 (Rust) | `src-tauri/` |
| Engine (sessões, PTY, API do celular) | Rust, processo persistente | `crates/omni-engine` |
| Núcleo e protocolo | Rust | `crates/omni-core`, `crates/omni-protocol` |

O engine roda separado da janela: fechar ou atualizar o app não derruba as sessões dos agentes. As decisões de arquitetura estão em [`docs/adr/`](docs/adr/).

## Instalar

Os instaladores ficam nas [releases](https://github.com/Medza-Primos-Organizacao/medza-agentes-releases/releases).

| Sistema | Arquivo |
|---|---|
| macOS Apple Silicon | `.dmg` com `aarch64` no nome |
| macOS Intel | `.dmg` com `x64` no nome |
| Windows 10/11 x64 | `*-setup.exe` |
| Linux x64 | `.deb` ou `.AppImage` |

Os instaladores não são notarizados pela Apple nem assinados com certificado comercial no Windows, então o sistema avisa na primeira abertura. Passo a passo por sistema em [`docs/multiplataforma.md`](docs/multiplataforma.md).

## Primeiro uso

1. Abra **Configurações → Agentes**, instale a CLI oficial que quiser usar e entre com a sua conta.
2. Adicione a pasta de um projeto, escolha o agente e a conta e abra a conversa.
3. Em **Configurações → Skills**, instale o pacote de skills incluído, se quiser.
4. Para ditar, use o microfone na visão **Conversa**. Na primeira vez o app oferece baixar o modelo de voz (cerca de 574 MB, conferido por SHA-256).

Nenhuma conta é fornecida pelo app: cada pessoa usa o próprio login em cada CLI.

## Privacidade e segurança

- O código e os instaladores não incluem contas, credenciais, conversas, arquivos de projetos ou configurações de máquina. Detalhes em [`docs/privacidade.md`](docs/privacidade.md).
- O app só dirige as CLIs oficiais; não lê nem guarda as credenciais delas.
- O acesso pelo celular vem desligado. Ligado, a API privilegiada continua só em `127.0.0.1` e o celular entra pela rede privada do Tailscale, com código do aparelho.
- Logs guardam só metadados, sem áudio nem texto de conversa.

| Sistema | Pasta de dados |
|---|---|
| Windows | `%LOCALAPPDATA%\ai.medza.agents` |
| macOS | `~/Library/Application Support/ai.medza.agents` |
| Linux | `~/.local/share/ai.medza.agents` |

## Compilar

Requisitos: Node.js 22, Rust estável, Git, CMake e as ferramentas nativas de compilação (Command Line Tools do Xcode no macOS; Visual Studio Build Tools com C++ e WebView2 no Windows; no Linux, as dependências listadas em `.github/workflows/ci.yml`).

```bash
git clone https://github.com/EnzoViniski/niski.git
cd niski
npm ci
npm run dev
```

No Windows, `setup.bat` instala Rust e as dependências e gera o instalador; `setup-dev.bat` abre em modo de desenvolvimento.

Instalador da sua máquina (sai em `installers/`; não exige credenciais nem chave de assinatura):

```bash
npm run build
```

Testes:

```bash
npm run typecheck
npm test
npm run build:engine
cargo test -p omni-engine -p omni-core -p omni-protocol -- --test-threads=1
cargo test -p omni-agents --lib
```

Os testes usam CLIs falsas e dados sintéticos; nenhum deles chama Claude, Codex ou outra conta real.

## Licença

[MIT](LICENSE). As skills incluídas mantêm as próprias licenças em cada pasta; veja [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
