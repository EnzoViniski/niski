# Windows, macOS e Linux

Um código só. Cada sistema compila o próprio instalador no GitHub Actions (workflow
**Instaladores Medza**, `.github/workflows/release.yml`).

## Release

1. Subir `VERSION`, rodar `npm run version:sync`, escrever a entrada em `src/lib/changelog.ts`, commitar.
2. `git tag v<versão> && git push origin v<versão>`: a tag precisa bater com `VERSION` (o workflow confere).
3. O workflow compila nas quatro combinações e cria um Release **em rascunho** com:
   - Windows x64: `*-setup.exe` (NSIS)
   - macOS Apple Silicon (`aarch64`) e Intel (`x64`): um `.dmg` para cada
   - Linux x64: `.AppImage` e `.deb`
4. Conferir o rascunho, testar os instaladores e clicar **Publish release**.

Sem tag, **Actions → Instaladores Medza → Run workflow** gera os mesmos arquivos como artefatos.
O workflow usa só o token automático do Actions: não há secrets para cadastrar.

`npm run build` local gera o instalador do sistema em que roda, em `installers/`.

`.github/workflows/ci.yml` roda os testes do engine e compila o app nas três plataformas a cada push na `main`.

## Atualização

Não há atualização automática nesta distribuição: `identidade.json` está sem chave pública do
updater. Para atualizar, baixe e instale a versão nova por cima. Os dados ficam na pasta do app e
são mantidos.

No Windows o hook do NSIS encerra o engine antes de trocar os arquivos. No macOS/Linux o app
compara o binário que o engine carregou com o arquivo em disco (`Pong.engine_exe_modified_ms`) e,
se mudou, reinicia o engine. Nos dois casos as sessões abertas caem na atualização.

## Primeira abertura no macOS (app sem Apple Developer)

O app é assinado ad-hoc (`bundle.macOS.signingIdentity: "-"`), não notarizado.

1. Abrir o `.dmg` e arrastar **Medza Agents** para **Aplicativos**.
2. Abrir o app. O macOS avisa que não pôde verificar o desenvolvedor: fechar o aviso.
3. **Ajustes do Sistema → Privacidade e Segurança** → rolar até o aviso do Medza Agents → **Abrir Mesmo Assim**.
4. Confirmar com a senha. Das próximas vezes abre direto.

Se aparecer "está danificado", rodar no Terminal:
`xattr -dr com.apple.quarantine "/Applications/Medza Agents.app"`

## Windows: aviso do SmartScreen

Sem certificado de assinatura de código o SmartScreen mostra "O Windows protegeu o computador"
(**Mais informações → Executar assim mesmo**). Só some com certificado Authenticode.

## Diferenças por sistema

| | Windows | macOS | Linux |
|---|---|---|---|
| Dados do engine | `%LOCALAPPDATA%\ai.medza.agents` | `~/Library/Application Support/ai.medza.agents` | `~/.local/share/ai.medza.agents` |
| Voz (Whisper) | CPU | Metal | CPU |
| Shell dos terminais | Git Bash, senão PowerShell | `$SHELL -l` | `$SHELL -l` |
| Login do agente | janela PowerShell | Terminal.app | x-terminal-emulator / gnome-terminal / konsole / xfce4-terminal / xterm |
| Processo do agente | ToolHelp (`claude.exe`) | `ps` | `ps` |
| Login Claude detectado por | `.credentials.json` | `oauthAccount` no `.claude.json` (token fica no Keychain) | `.credentials.json` |

App aberto pelo Finder/menu herda PATH mínimo; na inicialização o app importa o PATH do shell de
login do usuário (`engine_client::import_login_shell_path`) para achar `claude`/`codex`.

## Roteiro de teste

Marcar o que funcionou e mandar print do que não funcionou.

1. Instalar e abrir (seção acima no macOS).
2. Rodapé mostra `ENGINE: ONLINE`.
3. Configurações → Agentes: Claude/Codex aparecem como instalados; conta logada aparece conectada.
4. Adicionar um projeto (pasta qualquer). Abrir um terminal: prompt do shell aparece, `ls` funciona.
5. Abrir um agente Claude: sobe sem pedir "trust this folder".
6. Com o Claude parado, clicar ↻ no rodapé: aparece 5H/SEMANA com reset.
7. Painel de arquivos: abrir, editar e salvar um arquivo. Painel git: status e histórico aparecem.
8. Conectar conta nova (Configurações → Agentes): abre o Terminal com o login do CLI.
9. Visão Conversa: tocar no microfone, baixar o modelo de voz na primeira vez, ditar e conferir o texto no campo.
10. Configurações → Skills → **Instalar pacote Medza**: as quatro skills aparecem na conta.
11. Fechar o app e abrir de novo: terminais voltam.
12. Atalhos usam **Ctrl**, não Cmd, no macOS: anotar quais incomodam.

Riscos conhecidos a observar:
- **AppImage:** o engine continua rodando depois que o app fecha, mas a imagem montada do AppImage
  some junto com o app. Se o engine cair depois de fechar a janela, é isso.
- `.zshrc` lento (>3 s) ou que pede input: `claude` pode não ser encontrado.
