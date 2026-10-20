# Medza Agents

Workspace de agentes de IA para desktop: Claude Code, Codex, Cursor e Antigravity, com terminais, chat, projetos, histórico e voz local no mesmo aplicativo.

Desenvolvido pela Medza a partir do [OMNI AGENTS](https://github.com/FelipeCamposM/OMNI-AGENTS), de Felipe Campos Macedo, sob licença MIT. Esta distribuição tem identidade, portas e dados próprios.

Versão atual: **0.8.0** (arquivo `VERSION`). Novidades em **Configurações → Sobre** dentro do app.

## Instalar

Os instaladores ficam em [Releases](https://github.com/Medza-Primos-Organizacao/medza-agentes/releases). O repositório é privado: entre no GitHub com uma conta que tenha acesso à organização. Enquanto não houver release publicada, baixe os artefatos da última execução do workflow **Instaladores Medza** em [Actions](https://github.com/Medza-Primos-Organizacao/medza-agentes/actions) ou compile (veja [Compilar](#compilar)).

| Sistema | Arquivo |
|---|---|
| macOS Apple Silicon (M1 ou mais novo) | `.dmg` com `aarch64` no nome |
| macOS Intel | `.dmg` com `x64` no nome |
| Windows 10/11 x64 | `*-setup.exe` |
| Linux x64 | `.deb` ou `.AppImage` |

Os instaladores não são notarizados pela Apple nem assinados com certificado comercial no Windows, então o sistema avisa na primeira abertura.

### macOS

1. Abra o `.dmg` e arraste **Medza Agents** para **Aplicativos**.
2. Abra o app. Se o macOS disser que não pôde verificar o desenvolvedor, feche o aviso.
3. Vá em **Ajustes do Sistema → Privacidade e Segurança**, role até o aviso do Medza Agents e clique **Abrir Mesmo Assim**. Confirme com a senha.
4. Se aparecer "está danificado", rode no Terminal: `xattr -dr com.apple.quarantine "/Applications/Medza Agents.app"`.
5. Na primeira vez que usar o ditado, autorize o microfone.

### Windows

1. Execute o `*-setup.exe`.
2. Se aparecer "O Windows protegeu o computador", clique **Mais informações → Executar assim mesmo**.
3. Para acessar pelo celular, veja a regra de firewall em [docs/mobile-and-account-usage.md](docs/mobile-and-account-usage.md).

### Linux

- `.deb`: `sudo apt install ./Medza*.deb`.
- `.AppImage`: `chmod +x Medza*.AppImage` e execute o arquivo.

### Atualizar

Não há atualização automática nesta distribuição. Baixe o instalador da versão nova e instale por cima: contas, projetos e conversas ficam na pasta de dados e são mantidos. A versão instalada aparece em **Configurações → Sobre**.

## Primeiro uso

1. Abra **Configurações → Agentes**. Instale a CLI oficial que quiser usar (o app mostra as que faltam e abre a instalação) e entre com sua própria conta.
2. Adicione a pasta de um projeto, escolha o agente e a conta e abra a conversa.
3. Em **Configurações → Memória**, escreva e ative seu contexto pessoal, se quiser. O texto vale nas novas aberturas e retomadas de Claude e Codex.
4. Em **Configurações → Skills**, clique **Instalar pacote Medza** para copiar as skills incluídas para as contas compatíveis. Versões que você já tem são preservadas.
5. Para ditar, abra a visão **Conversa** do agente e use o microfone. Na primeira vez, o app oferece baixar o modelo de voz.

Cada pessoa começa com o próprio workspace e faz o próprio login. O app pode detectar uma instalação de Claude/Codex já existente na máquina; nenhuma conta é fornecida pelo Medza.

## Voz local (Whisper)

A captura de microfone e a transcrição estão compiladas no aplicativo, com `cpal` e `whisper-rs`/whisper.cpp. Não é preciso instalar OpenWhispr, Python ou uma chave de API para ditar dentro do Medza.

O modelo `large-v3-turbo-q5_0` é baixado uma vez pela interface (cerca de 574 MB), com conferência de tamanho e SHA-256. Ele fica na pasta local de modelos, fora do repositório e dos instaladores. Depois desse download, a transcrição funciona sem internet. O áudio é processado no computador; o texto só vai para o provedor do agente quando você envia a mensagem. Há prévia da fala e revisão no campo antes do envio. A transcrição está configurada para português.

A inferência usa Metal no Mac e CPU no Windows e no Linux.

## O que acompanha

- Painéis e abas com agentes e terminais, editor, arquivos e Git.
- Conversa com aprovações, anexos, histórico e retomada pelas CLIs oficiais.
- Proteção do índice com snapshots e arquivo local de transcripts Claude/Codex observados pelo app.
- Memória pessoal opcional, gerenciador de skills e pacote com `frontend-design`, `webapp-testing`, `copywriting` e `seo-audit` ([skills/README.md](skills/README.md)).
- Modelos locais pelo Codex, usando Ollama ou LM Studio já instalado e um modelo compatível informado por você.
- Fila de contas, modelos de agentes, fluxos, orquestração, agendamentos e acesso remoto opcional pelo celular.

Servidores de modelos, CLIs dos provedores, Docker e Tailscale são dependências opcionais e separadas. O acesso remoto vem desligado. A execução de IA segue a autenticação, os recursos e os limites da CLI escolhida.

## Privacidade e dados

O código e os instaladores não incluem contas, credenciais, conversas, arquivos de projetos, memória pessoal ou configurações de máquina. A distribuição limpa é gerada só a partir de arquivos versionados e não publica o histórico Git do workspace interno. Detalhes em [docs/privacidade.md](docs/privacidade.md).

| Sistema | Pasta de dados |
|---|---|
| Windows | `%LOCALAPPDATA%\ai.medza.agents` |
| macOS | `~/Library/Application Support/ai.medza.agents` |
| Linux | `~/.local/share/ai.medza.agents` |

O arquivamento protege conversas vistas por esta versão; não recupera conteúdo apagado antes de existir uma cópia. Faça backup externo para se proteger de perda do disco. Logs da aplicação contêm só metadados, sem áudio ou texto de conversa.

## Problemas comuns

- **CLI não encontrada:** instale pela tela **Configurações → Agentes** e reabra o app. No macOS/Linux, o app lê o PATH do shell de login; um `.zshrc` lento ou interativo pode atrapalhar.
- **Rodapé sem `ENGINE: ONLINE`:** feche o app, encerre o processo `omni-engine` (no Windows, `omni-engine.exe` no Gerenciador de Tarefas) e abra de novo.
- **Microfone sem resposta no Mac:** confira **Ajustes do Sistema → Privacidade e Segurança → Microfone**.

## Compilar

Requisitos: Node.js 22, Rust estável, Git, CMake e ferramentas nativas de compilação. No macOS, Command Line Tools do Xcode; no Windows, Visual Studio Build Tools com C++ e WebView2; no Linux, as dependências listadas em `.github/workflows/release.yml`.

```bash
git clone https://github.com/Medza-Primos-Organizacao/medza-agentes.git
cd medza-agentes
npm ci
npm run dev
```

No Windows, `setup.bat` instala Rust e as dependências e gera o instalador; `setup-dev.bat` abre em modo de desenvolvimento.

Para gerar o instalador da sua máquina:

```bash
npm run build
```

Os arquivos ficam em `installers/`. Para gerar só o `.app` no macOS, use `npm run build -- --bundles app`. O build não exige credenciais, modelo de voz baixado ou chave de assinatura do updater.

```bash
npm run typecheck
npm test
npm run build:engine
cargo test -p omni-engine -p omni-core -p omni-protocol -- --test-threads=1
cargo test -p omni-agents --lib
```

Os crates internos mantêm os nomes `omni-*` do upstream. O desenvolvimento usa `ai.medza.agents.dev`; a distribuição usa `ai.medza.agents`, com engine na porta 47421 e celular na 47422. Dados de desenvolvimento e do app instalado ficam separados.

## Manutenção e distribuição

Leia [docs/distribuicao.md](docs/distribuicao.md), [docs/multiplataforma.md](docs/multiplataforma.md) e [docs/validacao.md](docs/validacao.md). A versão fica em `VERSION`; use `npm run version:sync` ao alterá-la e escreva a entrada em `src/lib/changelog.ts`.

Para publicar uma release: `git tag v<versão> && git push origin v<versão>`. O workflow **Instaladores Medza** compila Windows, macOS (Apple Silicon e Intel) e Linux e cria um rascunho de release para revisão.

## Licença e créditos

Código do aplicativo sob [MIT](LICENSE), preservando o aviso de copyright de Felipe Campos Macedo. As skills mantêm suas próprias licenças e atribuições em cada pasta. Consulte [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
