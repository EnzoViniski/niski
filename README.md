# Medza Agents

Workspace de agentes de IA para desktop: Claude Code, Codex, Cursor e Antigravity, com terminais, chat, projetos, histórico e voz local no mesmo aplicativo.

Desenvolvido pela Medza a partir do [OMNI AGENTS](https://github.com/FelipeCamposM/OMNI-AGENTS), de Felipe Campos Macedo, sob licença MIT. Esta distribuição tem identidade, portas e dados próprios.

## Instalar

Baixe o instalador do seu sistema em [Releases](https://github.com/Medza-Primos-Organizacao/medza-agentes/releases). Como o repositório é privado, você precisa estar conectado ao GitHub com acesso à organização. Os instaladores são gerados pelo workflow **Instaladores Medza**, também disponível em Actions para execução manual. Até a primeira release ser publicada, use os artefatos desse workflow ou compile abaixo.

- macOS Apple Silicon: `.dmg` com `aarch64` no nome. Abra e arraste o app para Aplicativos.
- macOS Intel: `.dmg` com `x64` no nome.
- Windows x64: instalador `.exe`.
- Linux x64: `.deb` ou `.AppImage`.

A distribuição atual não é notarizada pela Apple nem assinada com certificado comercial no Windows. No macOS, uma eventual liberação deve ser feita manualmente em Ajustes do Sistema → Privacidade e Segurança. As atualizações são por novo instalador; o app não consulta releases do projeto original.

## Primeiro uso

1. Abra **Configurações → Agentes**. Instale a CLI oficial que quiser usar e entre com sua própria conta. O Medza indica as CLIs ausentes.
2. Adicione a pasta de um projeto, escolha o agente e a conta e abra a conversa.
3. Em **Configurações → Memória**, escreva e ative seu contexto pessoal, se desejar. O texto é aplicado nas novas aberturas/retomadas de Claude e Codex.
4. Em **Configurações → Skills**, clique **Instalar pacote Medza**. O pacote acompanha o aplicativo e pode ser instalado nas contas compatíveis, preservando versões existentes.
5. Para ditar, abra a visão **Conversa** do agente e use o microfone. Na primeira vez, o app oferece baixar o modelo de voz.

Cada pessoa começa com o próprio workspace e faz o próprio login. A aplicação pode detectar uma instalação de Claude/Codex já existente na máquina da pessoa; nenhuma conta é fornecida pelo Medza.

## Whisper incluído

A captura de microfone e a transcrição estão compiladas no aplicativo, com `cpal` e `whisper-rs`/whisper.cpp. Não é preciso instalar OpenWhispr, Python ou uma chave de API para ditar dentro do Medza.

O modelo `large-v3-turbo-q5_0` é baixado uma vez pela interface (aproximadamente 574 MB), com conferência de tamanho e SHA-256. Ele fica na pasta local de modelos, fora do repositório e dos instaladores. Depois desse download, a transcrição funciona sem internet. O áudio é processado localmente; o texto só é enviado ao provedor do agente quando você envia a mensagem. Há prévia da fala e revisão no campo antes do envio. A transcrição está configurada para português.

No macOS, autorize o microfone quando solicitado. O bundle inclui a descrição de uso e o entitlement de áudio. A inferência usa Metal no Mac e CPU nos demais sistemas.

## O que acompanha

- Painéis e abas com agentes e terminais, editor com workers locais, arquivos e Git.
- Conversa com aprovações, anexos, histórico e retomada pelas CLIs oficiais.
- Proteção do índice com snapshots e arquivo local de transcripts Claude/Codex observados pelo app.
- Memória pessoal opcional, gerenciador de skills e pacote com `frontend-design`, `webapp-testing`, `copywriting` e `seo-audit`.
- Modelos locais pelo Codex, usando Ollama ou LM Studio já instalado e um modelo compatível informado pelo usuário.
- Fila de contas, modelos de agentes, fluxos, orquestração, agendamentos e acesso remoto opcional.

Servidores de modelos, CLIs dos provedores, Docker e Tailscale são dependências opcionais e separadas. O acesso remoto vem desligado. A execução de IA segue a autenticação, os recursos e os limites da CLI escolhida.

## Privacidade e dados

O código e os instaladores não incluem contas, credenciais, conversas, arquivos de projetos, memória pessoal ou configurações de máquina. A distribuição limpa é gerada somente a partir de arquivos versionados e não publica o histórico Git do workspace interno.

Dados de uso ficam em pastas locais do sistema. O arquivamento protege conversas vistas por esta versão; não recupera conteúdo apagado antes de existir uma cópia. Faça backup externo para proteger contra perda do disco. O Medza não oferece garantia de perda zero. Logs da aplicação contêm metadados, sem áudio ou texto de conversa.

## Compilar

Requisitos: Node.js 22, Rust estável, Git, CMake e ferramentas nativas de compilação. No macOS, instale as Command Line Tools do Xcode; no Windows, Visual Studio Build Tools com C++ e WebView2; no Linux, as dependências listadas no workflow de CI.

```bash
git clone https://github.com/Medza-Primos-Organizacao/medza-agentes.git
cd medza-agentes
npm ci
npm run dev
```

Para gerar o instalador da sua máquina:

```bash
npm run build
```

Os arquivos ficam em `installers/`. Para gerar apenas o `.app` no macOS, use `npm run build -- --bundles app`. O build não exige credenciais, modelos de voz baixados ou chave de assinatura do updater.

```bash
npm run typecheck
npm test
npm run build:engine
cargo test -p omni-engine -p omni-core -p omni-protocol -- --test-threads=1
cargo test -p omni-agents --lib
```

O desenvolvimento usa `ai.medza.agents.dev`; a distribuição usa `ai.medza.agents`, com engine na porta 47421. Dados de desenvolvimento e do aplicativo instalado ficam separados.

## Manutenção e distribuição

Leia [docs/distribuicao.md](docs/distribuicao.md), [docs/privacidade.md](docs/privacidade.md) e [skills/README.md](skills/README.md). A versão fica em `VERSION`; use `npm run version:sync` ao alterá-la.

Verificação local desta preparação: build macOS Apple Silicon e testes automatizados. Os outros instaladores precisam passar pelo GitHub Actions e por uso real no respectivo sistema; um workflow configurado ainda não é uma validação executada.

## Licença e créditos

Código do aplicativo sob [MIT](LICENSE), preservando o aviso de copyright de Felipe Campos Macedo. As skills mantêm suas próprias licenças e atribuições em cada pasta. Consulte [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
