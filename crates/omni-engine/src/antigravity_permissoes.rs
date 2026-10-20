//! Fork: permissões do Antigravity (`agy`) no app.
//!
//! O Enzo tinha de ir ao terminal aceitar até a criação de um arquivo. Duas peças:
//! - **Modo da sessão**: o app abre o `agy` com `--mode accept-edits` (cria e edita arquivo sem perguntar)
//!   ou, se escolhido, `--mode auto` (agy 1.3.5+: também roda comando seguro sem perguntar). Só a sessão
//!   aberta pelo app — nada global muda. Conferido no agy 1.4.2 real (2026-10-19): nesta conta o Auto
//!   ainda não está liberado ("auto mode is unavailable, falling back to default" no log, liberação
//!   aos poucos pelo servidor); quando isso aparece, o app passa a usar `accept-edits` sozinho.
//! - **O que ainda pergunta aparece na conversa**: o diálogo de permissão do agy vira o cartão
//!   Permitir/Negar do chat (`interaction::approval`).


use axum::{extract::State, http::StatusCode, routing::get, Json, Router};
use serde_json::{json, Value};
use std::{path::{Path, PathBuf}, sync::Arc};

const ARQUIVO: &str = "antigravity.json";

/// Modo com que o app abre o `agy`. `Padrao` = não passa `--mode` (o `request-review` do agy, que pede
/// tudo). Padrão do app: aceitar edições (o Auto depende da conta e não existe no Windows).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Modo { Auto, AceitarEdicoes, Padrao }

impl Modo {
    fn de(texto: &str) -> Option<Self> {
        match texto { "auto" => Some(Self::Auto), "accept-edits" => Some(Self::AceitarEdicoes), "padrao" => Some(Self::Padrao), _ => None }
    }
    fn nome(self) -> &'static str {
        match self { Self::Auto => "auto", Self::AceitarEdicoes => "accept-edits", Self::Padrao => "padrao" }
    }
    fn do_app() -> Self { Self::AceitarEdicoes }
}

fn salvo(dir: &Path) -> Value {
    std::fs::read(dir.join(ARQUIVO)).ok().and_then(|b| serde_json::from_slice::<Value>(&b).ok()).unwrap_or_else(|| json!({}))
}

/// O modo escolhido na tela.
pub(crate) fn modo(dir: &Path) -> Modo {
    salvo(dir)["modo"].as_str().and_then(Modo::de).unwrap_or_else(Modo::do_app)
}

/// A conta já mostrou que não tem o Auto (ver `vigiar_auto`).
fn auto_indisponivel(dir: &Path) -> bool { cfg!(windows) || salvo(dir)["auto_indisponivel"] == true }

/// O que vai na linha: Auto sem Auto na conta vira aceitar edições.
pub(crate) fn modo_efetivo(dir: &Path) -> Modo {
    match modo(dir) { Modo::Auto if auto_indisponivel(dir) => Modo::AceitarEdicoes, outro => outro }
}

/// Texto do log do agy quando a conta não tem o Auto.
const SEM_AUTO: &str = "auto mode is unavailable";

/// Depois de abrir com `--mode auto`, lê o log da sessão (`--log-file` próprio): se o agy caiu no modo
/// padrão, marca a conta sem Auto — as próximas sessões já abrem aceitando edições.
fn vigiar_auto(dir: PathBuf, log: PathBuf) {
    std::thread::spawn(move || {
        for _ in 0..30 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let Ok(texto) = std::fs::read_to_string(&log) else { continue };
            if texto.contains(SEM_AUTO) {
                let mut dados = salvo(&dir);
                dados["auto_indisponivel"] = json!(true);
                let _ = crate::atomic_write_json(&dir.join(ARQUIVO), &dados);
                omni_core::registro::aviso("antigravity_sem_auto", &[]);
                return;
            }
            if texto.contains("SetCycleMode called") { return; }
        }
    });
}

/// O comando do `agy` com o `--mode` escolhido no app (troca o que estiver lá). A opção vai logo
/// depois do binário: o resto da linha pode ter `--conversation` (que precisa ficar no fim) ou um
/// prompt entre aspas.
pub(crate) fn com_modo(comando: &str, modo: Modo) -> String {
    let (binario, resto) = comando.split_once(' ').unwrap_or((comando, ""));
    let mut partes: Vec<&str> = Vec::new();
    let mut tokens = resto.split(' ').peekable();
    let mut sem_modo = String::new();
    // Tira um `--mode <x>` (ou `--mode=<x>`) que já esteja na linha, sem mexer no resto.
    while let Some(token) = tokens.next() {
        if token == "--mode" { tokens.next(); continue; }
        if token.starts_with("--mode=") { continue; }
        partes.push(token);
    }
    sem_modo.push_str(&partes.join(" "));
    let sem_modo = sem_modo.trim();
    let opcao = match modo { Modo::Padrao => String::new(), outro => format!(" --mode {}", outro.nome()) };
    // Depois do `--log-file "…"` (que o app põe logo após o binário), antes do resto.
    let (log, resto) = match sem_modo.strip_prefix("--log-file \"").and_then(|r| r.find('"').map(|i| i + "--log-file \"".len() + 1)) {
        Some(fim) => (format!(" {}", &sem_modo[..fim]), sem_modo[fim..].trim()),
        None => (String::new(), sem_modo),
    };
    if resto.is_empty() { format!("{binario}{log}{opcao}") } else { format!("{binario}{log}{opcao} {resto}") }
}

/// `com_modo` com o modo salvo na pasta do engine. `log`: o `--log-file` da sessão, para descobrir se a
/// conta tem o Auto.
pub(crate) fn com_modo_salvo(comando: &str, log: Option<PathBuf>) -> String {
    let Ok(dir) = crate::engine_dir() else { return comando.to_owned() };
    let modo = modo_efetivo(&dir);
    if let (Modo::Auto, Some(log)) = (modo, log) { vigiar_auto(dir, log); }
    com_modo(comando, modo)
}

/// A dica que o agy escreve no campo vazio conforme o modo (strings do binário 1.4.2): "Accept-edits
/// mode: file edits auto-approved (shift+tab to cycle)", "Auto mode: …", "Plan mode: …", "Best-of-N
/// mode: …". Não é texto digitado: sem isto o app achava que havia rascunho e não mandava mensagem nem
/// trocava o modelo.
pub(crate) fn dica_de_modo(texto: &str) -> bool {
    let texto = texto.trim();
    texto.ends_with("(shift+tab to cycle)") && texto.split_once(" mode: ").is_some_and(|(nome, _)| !nome.is_empty() && nome.len() <= 20)
}

/* ── Comandos de pouco risco liberados no settings.json do agy (pedido do Enzo) ────────────────── */

/// Liberados com `command(<prefixo>)` em `permissions.allow` do `settings.json` do agy — o agy casa pelo
/// começo do comando ("commands that start with"), e comando composto é conferido por partes. Só o que
/// lê, mostra versão, testa, compila ou checa; nada que apague, mova, baixe, publique ou mexa no git
/// além de ler (`git push`, `git checkout`, `git reset` continuam pedindo). Fora de propósito: `find`
/// (tem `-delete`/`-exec`), `sed` (tem `-i`), `xargs`, `curl`, `rm`, `mv`, `cp`, `npm install`.
pub(crate) const COMANDOS_COMUNS: &[&str] = &[
    "ls", "pwd", "cat", "head", "tail", "wc", "which", "file", "stat", "du", "df", "tree", "date", "whoami", "uname",
    "echo", "printf", "grep", "rg", "diff", "cmp", "sort", "uniq", "cut", "tr", "jq", "mkdir", "touch",
    // Fork: git branch também aceita -D/--delete; um prefixo não restringe seus argumentos.
    "git status", "git diff", "git log", "git show", "git rev-parse", "git ls-files", "git blame", "git remote -v", "git stash list",
    "node --version", "npm --version", "npm ls", "npm test", "npm run test", "npm run lint", "npm run typecheck", "npm run build",
    "npx tsc", "npx vitest", "npx eslint", "npx prettier --check",
    "cargo check", "cargo test", "cargo build", "cargo clippy", "cargo fmt --check",
    "python3 --version", "pytest", "python3 -m pytest", "go test", "go build", "go vet",
];

/// Junta os comandos em `permissions.allow` sem tirar nada do que já está lá. Devolve os que entraram.
pub(crate) fn mesclar_comandos(config: &mut Value, comandos: &[&str]) -> Vec<String> {
    if !config.is_object() { *config = json!({}); }
    let permissoes = config.as_object_mut().expect("objeto").entry("permissions").or_insert_with(|| json!({}));
    if !permissoes.is_object() { *permissoes = json!({}); }
    let lista = permissoes.as_object_mut().expect("objeto").entry("allow").or_insert_with(|| json!([]));
    if !lista.is_array() { *lista = json!([]); }
    let lista = lista.as_array_mut().expect("lista");
    let mut novos = Vec::new();
    for comando in comandos {
        let regra = format!("command({comando})");
        if !lista.iter().any(|item| item.as_str() == Some(regra.as_str())) { lista.push(json!(regra)); novos.push(regra); }
    }
    novos
}

/// Tira só as regras que o app pôs (as do usuário ficam).
pub(crate) fn tirar_comandos(config: &mut Value, regras: &[String]) -> usize {
    let Some(lista) = config.pointer_mut("/permissions/allow").and_then(Value::as_array_mut) else { return 0 };
    let antes = lista.len();
    lista.retain(|item| !item.as_str().is_some_and(|r| regras.iter().any(|x| x == r)));
    antes - lista.len()
}

/// Grava no `settings.json` do agy (cópia do original em `settings.json.medza-backup` na primeira vez) e
/// lembra no `antigravity.json` do engine o que o app acrescentou, para poder desfazer.
pub(crate) fn liberar_comandos_comuns(config_dir: &Path, dir: &Path, liberar: bool) -> std::io::Result<usize> {
    let caminho = config_dir.join("settings.json");
    let texto = std::fs::read_to_string(&caminho).unwrap_or_else(|_| "{}".into());
    let mut config: Value = serde_json::from_str(&texto).map_err(|e| std::io::Error::other(format!("settings.json do agy inválido: {e}")))?;
    let copia = config_dir.join("settings.json.medza-backup");
    if !copia.exists() && caminho.exists() { std::fs::copy(&caminho, &copia)?; }
    let mut dados = salvo(dir);
    let anteriores: Vec<String> = dados["comandos_liberados"].as_array().into_iter().flatten().filter_map(|v| v.as_str().map(str::to_owned)).collect();
    let mudou = if liberar {
        let novos = mesclar_comandos(&mut config, COMANDOS_COMUNS);
        let todos: Vec<String> = anteriores.into_iter().chain(novos.iter().cloned()).collect();
        dados["comandos_liberados"] = json!(todos);
        novos.len()
    } else {
        let tirados = tirar_comandos(&mut config, &anteriores);
        dados["comandos_liberados"] = json!([]);
        tirados
    };
    let temporario = caminho.with_extension("json.medza-tmp");
    std::fs::write(&temporario, serde_json::to_string_pretty(&config).map_err(std::io::Error::other)?)?;
    std::fs::rename(&temporario, &caminho)?;
    crate::atomic_write_json(&dir.join(ARQUIVO), &dados).map_err(|e| std::io::Error::other(e.to_string()))?;
    omni_core::registro::info("antigravity_comandos", &[("liberar", if liberar { "sim" } else { "nao" }), ("mudou", &mudou.to_string())]);
    Ok(mudou)
}

fn comandos_liberados(dir: &Path) -> usize { salvo(dir)["comandos_liberados"].as_array().map_or(0, Vec::len) }

/// Diálogo de permissão do agy (formato conferido na tela real do 1.4.2):
///
/// ```text
/// Create file
/// ──────────
/// /…/ok.txt  +1
///   1  +  ok
///
/// Allow creation of this file?
/// > 1. Yes, allow creation
///   2. No, deny creation
///
///   ↑/↓ Navigate · tab Amend · f full diff
/// ```
///
/// O menu é de setas (`>` marca a opção): as teclas andam da marcada até a pedida e confirmam. Permitir
/// = o primeiro "Yes" **de uma vez** (nunca "always allow", que grava no settings.json); negar = o
/// primeiro "No" de uma vez. Devolve `(teclas para permitir, teclas para negar, texto do diálogo)`.
pub(crate) fn aprovacao(texto: &str) -> Option<(String, String, String)> {
    let linhas: Vec<&str> = texto.lines().map(str::trim).collect();
    let opcao = |l: &str| -> Option<(bool, String)> {
        let marcada = l.starts_with('>') || l.starts_with('❯') || l.starts_with('›');
        let resto = l.trim_start_matches(['>', '❯', '›']).trim();
        // Numerada ("1. Yes, allow creation") ou não ("Yes, I trust this folder", na confiança da pasta).
        let rotulo = match resto.split_once(". ") { Some((n, r)) if n.parse::<u8>().is_ok() => r, _ => resto };
        (rotulo.starts_with("Yes") || rotulo.starts_with("No")).then(|| (marcada, rotulo.trim().to_owned()))
    };
    let pergunta = linhas.iter().rposition(|l| l.ends_with('?') && opcao(l).is_none())?;
    // As opções começam até 4 linhas depois da pergunta (a da confiança na pasta tem um parágrafo no meio).
    let comeco = pergunta + 1 + linhas[pergunta + 1..].iter().take(5).position(|l| opcao(l).is_some())?;
    let opcoes: Vec<(bool, String)> = linhas[comeco..].iter().take(10).map_while(|l| opcao(l)).collect();
    if opcoes.len() < 2 || opcoes.iter().filter(|o| o.0).count() != 1 { return None; }
    let de_uma_vez = |o: &&(bool, String)| { let r = o.1.to_lowercase(); !r.contains("always") && !r.contains("persist") && !r.contains(" tell ") };
    let sim = opcoes.iter().position(|o| o.1.starts_with("Yes") && de_uma_vez(&o))?;
    let nao = opcoes.iter().position(|o| o.1.starts_with("No") && de_uma_vez(&o))?;
    let marcada = opcoes.iter().position(|o| o.0)?;
    let teclas = |alvo: usize| {
        let seta = if alvo >= marcada { "\x1b[B" } else { "\x1b[A" };
        format!("{}\r", seta.repeat(alvo.abs_diff(marcada)))
    };
    // Texto do cartão: do título (a linha antes do traço ──) até as opções.
    let fim = comeco + opcoes.len();
    let traco = linhas[..pergunta].iter().rposition(|l| l.chars().count() >= 10 && l.chars().all(|c| c == '─'));
    let inicio = traco.map_or(pergunta.saturating_sub(6), |t| t.saturating_sub(1)).max(pergunta.saturating_sub(16));
    let dialogo = linhas[inicio..fim.min(linhas.len())].iter().copied()
        .filter(|l| !l.is_empty() && !l.chars().all(|c| c == '─')).collect::<Vec<_>>().join("\n");
    Some((teclas(sim), teclas(nao), dialogo))
}

/* ── Rota da tela de Configurações → Agentes ───────────────────────────────────────────────── */

pub(crate) fn rotas() -> Router<Arc<crate::EngineState>> {
    Router::new().route("/antigravity/permissoes", get(ler).post(gravar))
}

fn pasta(state: &crate::EngineState) -> &Path { state.state_file.parent().unwrap() }

async fn ler(State(state): State<Arc<crate::EngineState>>) -> Json<Value> {
    let dir = pasta(&state);
    Json(json!({"modo": modo(dir).nome(), "efetivo": modo_efetivo(dir).nome(), "auto_disponivel": !auto_indisponivel(dir),
        "comandos_liberados": comandos_liberados(dir), "comandos_comuns": COMANDOS_COMUNS}))
}

async fn gravar(State(state): State<Arc<crate::EngineState>>, Json(corpo): Json<Value>) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
    if let Some(liberar) = corpo["liberar_comuns"].as_bool() {
        let config = crate::config_do_antigravity()
            .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Pasta do Antigravity não encontrada"}))))?;
        let dir = pasta(&state).to_path_buf();
        liberar_comandos_comuns(&config, &dir, liberar)
            .map_err(|e| (StatusCode::CONFLICT, Json(json!({"error": e.to_string()}))))?;
        return Ok(Json(json!({"comandos_liberados": comandos_liberados(&dir)})));
    }
    let pedido = corpo["modo"].as_str().and_then(Modo::de)
        .ok_or_else(|| (StatusCode::BAD_REQUEST, Json(json!({"error": "Modo deve ser auto, accept-edits ou padrao"}))))?;
    if pedido == Modo::Auto && cfg!(windows) {
        return Err((StatusCode::BAD_REQUEST, Json(json!({"error": "O modo Auto do Antigravity ainda não existe no Windows"}))));
    }
    // Escolher o Auto de novo tenta de novo (a conta pode ter ganho o recurso).
    crate::atomic_write_json(&pasta(&state).join(ARQUIVO), &json!({"modo": pedido.nome()}))
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Não consegui gravar"}))))?;
    omni_core::registro::info("antigravity_modo", &[("modo", pedido.nome())]);
    let dir = pasta(&state);
    Ok(Json(json!({"modo": pedido.nome(), "efetivo": modo_efetivo(dir).nome(), "auto_disponivel": !auto_indisponivel(dir)})))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    /// A tela real do agy 1.4.2 pedindo para criar um arquivo (capturada com `captura_permissao_do_agy`).
    const CRIAR_ARQUIVO: &str = "> crie o arquivo ok.txt com o texto ok\n\n● Edited 1 file (ok.txt) (ctrl+o to expand)\n\nCreate file\n\
────────────────────────────────────────────────────────────\n\n/Users/ana/x/ok.txt  +1\n  1  +  ok\n\n\
Allow creation of this file?\n> 1. Yes, allow creation\n  2. No, deny creation\n\n  ↑/↓ Navigate · tab Amend · f full diff\n\
esc to cancel                                        Gemini 4 Argon · medium\n";

    #[test]
    fn pedido_de_criar_arquivo_vira_cartao_permitir_negar() {
        let (sim, nao, texto) = aprovacao(CRIAR_ARQUIVO).expect("diálogo do agy");
        assert_eq!(sim, "\r", "a marcada já é o Yes");
        assert_eq!(nao, "\x1b[B\r");
        assert_eq!(texto, "Create file\n/Users/ana/x/ok.txt  +1\n1  +  ok\nAllow creation of this file?\n> 1. Yes, allow creation\n2. No, deny creation");
        // O chat do app passa pelo `approval` do interaction.
        assert!(crate::interaction::approval(CRIAR_ARQUIVO, "antigravity").is_some());
    }

    #[test]
    fn nunca_escolhe_always_allow_e_anda_pelas_setas() {
        let tela = "Run command\n──────────\nnpm test\n\nRun this command?\n  1. Yes, and always allow for commands that start with 'npm' (Persist to settings.json)\n> 2. Yes, run command\n  3. No, and always deny 'npm test' in this conversation\n  4. No, deny\n";
        let (sim, nao, _) = aprovacao(tela).unwrap();
        assert_eq!(sim, "\r");
        assert_eq!(nao, "\x1b[B\x1b[B\r");
        let confianca = "Do you trust the contents of this project?\n\nAntigravity CLI requires permission to read, edit, and execute files here.\n\n> Yes, I trust this folder\n  No, exit\n\n  ↑/↓ Navigate · enter Confirm\n";
        // "Confiar na pasta?": também vai para a conversa. Negar é o "No, exit" (fecha o agy).
        let (sim, nao, texto) = aprovacao(confianca).expect("confiança na pasta");
        assert_eq!((sim.as_str(), nao.as_str()), ("\r", "\x1b[B\r"));
        assert!(texto.contains("Do you trust the contents of this project?") && texto.contains("No, exit"));
        assert!(aprovacao("Qual opção?\n> 1. Criar\n  2. Corrigir\n").is_none(), "pergunta comum do agente não é permissão");
        assert!(aprovacao("Allow?\n> 1. Yes, and always allow x\n  2. No, deny\n").is_none(), "sem Yes de uma vez, nada");
    }

    #[test]
    fn modo_entra_logo_depois_do_binario_e_troca_o_anterior() {
        assert_eq!(com_modo("agy", Modo::Auto), "agy --mode auto");
        assert_eq!(com_modo("agy --log-file \"/a b/c.log\" --conversation x", Modo::AceitarEdicoes),
            "agy --log-file \"/a b/c.log\" --mode accept-edits --conversation x");
        assert_eq!(com_modo("agy --log-file \"/a b/c.log\"", Modo::Auto), "agy --log-file \"/a b/c.log\" --mode auto");
        assert_eq!(com_modo("agy --mode auto --model g", Modo::Padrao), "agy --model g");
        assert_eq!(com_modo("agy --mode=plan -i \"oi\"", Modo::Auto), "agy --mode auto -i \"oi\"");
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(modo(dir.path()), Modo::AceitarEdicoes, "padrão do app: aceitar edições");
        fs::write(dir.path().join(ARQUIVO), r#"{"modo":"padrao"}"#).unwrap();
        assert_eq!(modo(dir.path()), Modo::Padrao);
        fs::write(dir.path().join(ARQUIVO), r#"{"modo":"auto","auto_indisponivel":true}"#).unwrap();
        assert_eq!((modo(dir.path()), modo_efetivo(dir.path())), (Modo::Auto, Modo::AceitarEdicoes), "conta sem Auto");
    }

    #[test]
    fn dica_do_modo_no_campo_nao_e_rascunho() {
        assert!(dica_de_modo(" Accept-edits mode: file edits auto-approved (shift+tab to cycle)  "));
        assert!(dica_de_modo("Plan mode: research & plan only (shift+tab to cycle)"));
        assert!(!dica_de_modo("crie o arquivo (shift+tab to cycle)"));
        assert!(!dica_de_modo("rascunho do usuário"));
        // A linha real do campo do agy em accept-edits, com o cursor depois do "> ".
        let mut tela = crate::interaction::Interaction::new(6, 100);
        tela.parser.process("\x1b[?2004h\r\n> Accept-edits mode: file edits auto-approved (shift+tab to cycle)\r\n\
? for shortcuts      accept-edits · Gemini 4 Argon · medium\x1b[2;3H".as_bytes());
        assert!(crate::interaction::ready(tela.parser.screen(), "antigravity"), "o agy ocioso em accept-edits aceita mensagem");
        tela.parser.process(b"\x1b[2;3Hoi\x1b[K");
        assert!(!crate::interaction::ready(tela.parser.screen(), "antigravity"), "texto digitado de verdade continua bloqueando");
    }

    #[test]
    fn exclusao_de_branch_nao_e_liberada_por_prefixo() {
        for comando in ["git branch -D tarefa", "git branch --delete tarefa", "git branch -m novo"] {
            assert!(!COMANDOS_COMUNS.iter().any(|prefixo| comando.starts_with(prefixo)), "{comando}");
        }
        assert!(COMANDOS_COMUNS.contains(&"git status"));
    }

    #[test]
    fn comandos_comuns_entram_sem_tirar_os_do_usuario_e_saem_so_os_do_app() {
        let mut config = json!({"permissions": {"allow": ["command(ls)", "command(zip)"]}, "trustedWorkspaces": ["/x"]});
        let novos = mesclar_comandos(&mut config, &["ls", "git status", "npm test"]);
        assert_eq!(novos, ["command(git status)", "command(npm test)"], "ls já estava");
        assert_eq!(config["permissions"]["allow"], json!(["command(ls)", "command(zip)", "command(git status)", "command(npm test)"]));
        assert_eq!(tirar_comandos(&mut config, &novos), 2);
        assert_eq!(config["permissions"]["allow"], json!(["command(ls)", "command(zip)"]));
        assert_eq!(config["trustedWorkspaces"], json!(["/x"]));
        // Nada perigoso na lista.
        for perigoso in ["rm", "mv", "cp", "find", "sed", "xargs", "curl", "git push", "git checkout", "git reset", "npm install", "sudo", "git"] {
            assert!(!COMANDOS_COMUNS.contains(&perigoso), "{perigoso}");
        }
        let dir = tempfile::tempdir().unwrap();
        let agy = dir.path().join("agy");
        fs::create_dir_all(&agy).unwrap();
        fs::write(agy.join("settings.json"), r#"{"permissions":{"allow":["command(zip)"]}}"#).unwrap();
        assert_eq!(liberar_comandos_comuns(&agy, dir.path(), true).unwrap(), COMANDOS_COMUNS.len());
        assert!(agy.join("settings.json.medza-backup").is_file(), "cópia do original");
        assert_eq!(comandos_liberados(dir.path()), COMANDOS_COMUNS.len());
        assert_eq!(liberar_comandos_comuns(&agy, dir.path(), false).unwrap(), COMANDOS_COMUNS.len());
        let depois: Value = serde_json::from_str(&fs::read_to_string(agy.join("settings.json")).unwrap()).unwrap();
        assert_eq!(depois["permissions"]["allow"], json!(["command(zip)"]));
    }

    #[test]
    fn log_do_agy_sem_auto_marca_a_conta() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(ARQUIVO), r#"{"modo":"auto"}"#).unwrap();
        let log = dir.path().join("sessao.log");
        // Linhas reais do log do agy 1.4.2.
        fs::write(&log, "I1019 overrides.go:82] --mode: overriding agent mode to auto\nI1019 manager.go:1495] SetCycleMode called: auto\n\
I1019 manager.go:1498] auto mode is unavailable, falling back to default\n").unwrap();
        vigiar_auto(dir.path().to_path_buf(), log);
        let limite = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while modo_efetivo(dir.path()) != Modo::AceitarEdicoes && std::time::Instant::now() < limite { std::thread::sleep(std::time::Duration::from_millis(100)); }
        assert_eq!(modo_efetivo(dir.path()), Modo::AceitarEdicoes);
        assert_eq!(modo(dir.path()), Modo::Auto, "a escolha do usuário fica");
    }

    /// Captura, uma vez e à mão, a tela real de um pedido de permissão do agy (modo padrão
    /// `request-review`), para calibrar o reconhecimento. Pasta de rascunho em `~`, prompt curto,
    /// Esc no diálogo (nega) e tudo encerrado no fim.
    ///
    /// `cargo test -p omni-engine captura_permissao_do_agy -- --ignored --nocapture`
    #[test]
    #[ignore = "CLI real (agy): rodar à mão"]
    fn captura_permissao_do_agy() {
        let dados = tempfile::tempdir().unwrap();
        let pasta = PathBuf::from(std::env::var("HOME").unwrap()).join(format!("medza-rascunho-agy-{}", now_ms()));
        fs::create_dir_all(&pasta).unwrap();
        let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: mobile::MobileRuntime::new(dados.path()),
            token: "test-only".into(), state_file: dados.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
            id_sequence: AtomicU64::new(1) });
        let sessao = spawn_terminal_inner(&state, state.next_id(), "p".into(), "agy".into(), pasta.to_string_lossy().into_owned(), None,
            Some("agy -i \"crie o arquivo ok.txt com o texto ok\"".into()), 40, 120, now_ms(),
            SessionOrigin { headless: true, provider: Some("antigravity".into()), ..SessionOrigin::default() }).unwrap();
        let viva = match state.sessions.lock().unwrap().get(&sessao.id) { Some(SessionEntry::Live(s)) => s.clone(), _ => panic!("sem sessão") };
        let inicio = std::time::Instant::now();
        // O "confiar na pasta?" grava em settings.json: guarda o original e devolve no fim.
        let config = PathBuf::from(std::env::var("HOME").unwrap()).join(".gemini/antigravity-cli/settings.json");
        let original = fs::read(&config).ok();
        let mut confiou = false;
        let mut capturou = false;
        while inicio.elapsed() < std::time::Duration::from_secs(120) {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let tela = viva.interaction.lock().unwrap().parser.screen().contents();
            let pedido = ["Allow ", "Accept this file edit?", "Run this command", "requires permission", "Do you want to proceed?"]
                .iter().any(|t| tela.contains(t)) && tela.contains("No,");
            if !confiou && tela.contains("Do you trust the contents of this project?") {
                confiou = true;
                let _ = write_terminal(&state, &sessao.id, "\r");
                continue;
            }
            if pedido {
                std::thread::sleep(std::time::Duration::from_millis(800));
                let tela = viva.interaction.lock().unwrap().parser.screen().contents();
                println!("=== TELA DO PEDIDO ({}s) ===", inicio.elapsed().as_secs());
                for (i, linha) in tela.lines().enumerate() { println!("{i:02}|{linha:?}"); }
                capturou = true;
                let _ = write_terminal(&state, &sessao.id, "\x1b");
                std::thread::sleep(std::time::Duration::from_secs(2));
                break;
            }
        }
        if !capturou {
            println!("=== SEM PEDIDO; TELA FINAL ===");
            for (i, linha) in viva.interaction.lock().unwrap().parser.screen().contents().lines().enumerate() { println!("{i:02}|{linha:?}"); }
        }
        let _ = stop_session(&state, &sessao.id);
        if let Some(original) = original { fs::write(&config, original).unwrap(); }
        let _ = fs::remove_dir_all(&pasta);
    }

    /// **CLI real, à mão**: com `--mode accept-edits` o agy cria o arquivo sem pedir; a única pergunta é a
    /// confiança na pasta nova, aceita pelas teclas do cartão Permitir (`interaction::approval`).
    /// `cargo test -p omni-engine modo_auto_do_agy_cria_arquivo -- --ignored --nocapture`
    #[test]
    #[ignore = "CLI real (agy): rodar à mão"]
    fn modo_auto_do_agy_cria_arquivo_sem_perguntar() {
        let dados = tempfile::tempdir().unwrap();
        let pasta = PathBuf::from(std::env::var("HOME").unwrap()).join(format!("medza-rascunho-agy-{}", now_ms()));
        fs::create_dir_all(&pasta).unwrap();
        let config = PathBuf::from(std::env::var("HOME").unwrap()).join(".gemini/antigravity-cli/settings.json");
        let original = fs::read(&config).ok();
        let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: mobile::MobileRuntime::new(dados.path()),
            token: "test-only".into(), state_file: dados.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
            id_sequence: AtomicU64::new(1) });
        let sessao = spawn_terminal_inner(&state, state.next_id(), "p".into(), "agy".into(), pasta.to_string_lossy().into_owned(), None,
            Some(com_modo("agy -i \"crie o arquivo ok.txt com o texto ok\"", Modo::AceitarEdicoes)), 40, 120, now_ms(),
            SessionOrigin { headless: true, provider: Some("antigravity".into()), ..SessionOrigin::default() }).unwrap();
        let viva = match state.sessions.lock().unwrap().get(&sessao.id) { Some(SessionEntry::Live(s)) => s.clone(), _ => panic!("sem sessão") };
        let inicio = std::time::Instant::now();
        let mut pedidos = Vec::new();
        while inicio.elapsed() < std::time::Duration::from_secs(120) && !pasta.join("ok.txt").is_file() {
            std::thread::sleep(std::time::Duration::from_millis(500));
            let tela = viva.interaction.lock().unwrap().parser.screen().contents();
            if let Some((permitir, _, texto)) = crate::interaction::approval(&tela, "antigravity") {
                let pergunta = texto.lines().find(|l| l.ends_with('?')).unwrap_or("").to_owned();
                println!("[{:>3}s] pedido na conversa: {pergunta:?} → Permitir", inicio.elapsed().as_secs());
                pedidos.push(pergunta);
                let _ = write_terminal(&state, &sessao.id, &permitir);
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
        }
        let criou = pasta.join("ok.txt").is_file();
        println!("criou ok.txt: {criou} em {}s; pedidos: {pedidos:?}", inicio.elapsed().as_secs());
        let _ = stop_session(&state, &sessao.id);
        if let Some(original) = original { fs::write(&config, original).unwrap(); }
        let _ = fs::remove_dir_all(&pasta);
        assert!(criou, "o agy não criou o arquivo");
        assert!(pedidos.iter().all(|p| p.contains("trust")), "aceitando edições, só a confiança na pasta pergunta: {pedidos:?}");
    }

    /// **CLI real, à mão, sem prompt**: a tela ociosa do agy em cada modo e se o app a reconhece como
    /// pronta para receber mensagem. Abre em `~/omni` (pasta já confiável), não manda nada e fecha.
    /// `cargo test -p omni-engine tela_ociosa_do_agy -- --ignored --nocapture`
    #[test]
    #[ignore = "CLI real (agy): rodar à mão"]
    fn tela_ociosa_do_agy() {
        for modo in [Modo::AceitarEdicoes, Modo::Padrao] {
            let dados = tempfile::tempdir().unwrap();
            let pasta = PathBuf::from(std::env::var("HOME").unwrap()).join("omni");
            let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: mobile::MobileRuntime::new(dados.path()),
                token: "test-only".into(), state_file: dados.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
                id_sequence: AtomicU64::new(1) });
            let sessao = spawn_terminal_inner(&state, state.next_id(), "p".into(), "agy".into(), pasta.to_string_lossy().into_owned(), None,
                Some(com_modo("agy", modo)), 40, 120, now_ms(),
                SessionOrigin { headless: true, provider: Some("antigravity".into()), ..SessionOrigin::default() }).unwrap();
            let viva = match state.sessions.lock().unwrap().get(&sessao.id) { Some(SessionEntry::Live(s)) => s.clone(), _ => panic!() };
            std::thread::sleep(std::time::Duration::from_secs(10));
            let tela = viva.interaction.lock().unwrap().parser.screen().contents();
            let cap = crate::interaction::capabilities(&viva);
            println!("=== {modo:?}: pronta={} motivo={:?}", cap.prompt, cap.reason);
            for (i, linha) in tela.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) { println!("{i:02}|{linha:?}"); }
            let _ = stop_session(&state, &sessao.id);
        }
    }

    /// **CLI real, à mão, um prompt curto**: o caminho do chat do app com o agy — manda "responda só: ok"
    /// pela rota `/prompt` e troca o modelo pela `/ajustes` (reabre com `--conversation`). Em `~/omni`.
    /// `cargo test -p omni-engine chat_do_agy_manda_e_troca_modelo -- --ignored --nocapture`
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    #[ignore = "CLI real (agy): rodar à mão"]
    async fn chat_do_agy_manda_e_troca_modelo() {
        let dados = tempfile::tempdir().unwrap();
        let casa = PathBuf::from(std::env::var("HOME").unwrap());
        let pasta = casa.join("omni").to_string_lossy().into_owned();
        let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: mobile::MobileRuntime::new(dados.path()),
            token: "test-only".into(), state_file: dados.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
            id_sequence: AtomicU64::new(1) });
        mobile::start(state.clone());
        crate::atomic_write_json(&dados.path().join("profiles.json"), &json!({"profiles": [{"id": "agy-padrao", "provider": "antigravity",
            "name": "Padrão", "config_dir": casa.join(".gemini/antigravity-cli").to_string_lossy(), "builtin": true, "created_at_ms": 0}]})).unwrap();
        mobile::publish(&state, vec![omni_protocol::PublishedProject { id: "p".into(), name: "omni".into(), path: pasta.clone() }],
            vec![omni_protocol::PublishedAgent { id: "antigravity".into(), label: "Antigravity".into(), command: "agy".into(), resume: None }], None).unwrap();
        let id = state.next_id();
        let segmento = |terminal: Option<String>| omni_core::conversations::Segment { provider: "antigravity".into(), profile_id: Some("agy-padrao".into()),
            external_session_id: None, transcript_path: None, terminal_session_id: terminal, started_at_ms: now_ms(), ended_at_ms: None };
        let conversa = |terminal| omni_core::conversations::Conversation { id: "c-agy".into(), project_id: "p".into(), cwd: pasta.clone(),
            title: "teste agy".into(), title_source: omni_core::conversations::TitleSource::Manual, created_at_ms: now_ms(), segments: vec![segmento(terminal)] };
        omni_core::conversations::write_index(dados.path(), &[conversa(Some(id.clone()))]).unwrap();
        spawn_terminal_inner(&state, id.clone(), "p".into(), "agy".into(), pasta.clone(), None, Some("agy".into()), 40, 120, now_ms(),
            SessionOrigin { headless: true, provider: Some("antigravity".into()), profile_id: Some("agy-padrao".into()),
                conversation_id: Some("c-agy".into()), ..SessionOrigin::default() }).unwrap();
        let viva = || match state.sessions.lock().unwrap().get(&id) { Some(SessionEntry::Live(s)) => s.clone(), _ => panic!() };
        let tela = || viva().interaction.lock().unwrap().parser.screen().contents();
        let espera = |s: u64| tokio::time::sleep(std::time::Duration::from_secs(s));
        let pronta = |limite: u64| { let viva = viva(); async move {
            for _ in 0..limite * 2 { if crate::interaction::capabilities(&viva).prompt { return true; } tokio::time::sleep(std::time::Duration::from_millis(500)).await; }
            false } };
        assert!(pronta(30).await, "não ficou pronta: {}", tela());
        let post = |rota: &str, corpo: Value| { let state = state.clone(); let rota = rota.to_owned(); let id = id.clone(); async move {
            let revisao = crate::interaction::capabilities(&match state.sessions.lock().unwrap().get(&id) { Some(SessionEntry::Live(s)) => s.clone(), _ => panic!() }).revision;
            mobile::local(state, "POST", &rota, Some(corpo.to_string()), None,
                vec![("idempotency-key".into(), format!("t-{}", now_ms())), ("if-match".into(), revisao)]).await } };
        let EngineResponse::Api { status, body } = post("/conversas/c-agy/prompt", json!({"texto": "responda só: ok"})).await else { panic!() };
        println!("prompt: {status} {body}");
        let turnos = viva().interaction.lock().unwrap().turnos_iniciados;
        let mut comecou = false;
        for _ in 0..60 { espera(1).await; if viva().interaction.lock().unwrap().turnos_iniciados > turnos { comecou = true; break; } }
        println!("o agy começou o turno: {comecou}");
        assert!(pronta(90).await, "não voltou a ficar pronta: {}", tela());
        // A descoberta (a cada 15 s na lista) acha a conversa do agy pelo log da sessão.
        let mut indice = omni_core::conversations::read_index(dados.path());
        omni_core::conversations::discover_codex_transcripts(&mut indice, &omni_core::profiles(dados.path()));
        omni_core::conversations::write_index(dados.path(), &indice).unwrap();
        println!("conversa do agy: {:?}", indice[0].segments.last().and_then(|s| s.external_session_id.clone()));
        let EngineResponse::Api { body, .. } = mobile::local(state.clone(), "GET", "/conversas/c-agy/opcoes", None, None, vec![]).await else { panic!() };
        let opcoes: Value = serde_json::from_str(&body).unwrap();
        let atual = opcoes["model"].as_str().unwrap_or("").to_owned();
        let outro = opcoes["models"].as_array().unwrap().iter().filter_map(|m| m["id"].as_str()).find(|m| *m != atual).unwrap().to_owned();
        println!("modelo atual {atual:?} → {outro:?}");
        let EngineResponse::Api { status, body } = post("/conversas/c-agy/ajustes", json!({"profile_id": null, "model": outro, "effort": "low"})).await else { panic!() };
        println!("ajustes: {status} {body}");
        espera(4).await;
        assert!(pronta(40).await, "não reabriu: {}", tela());
        let cabecalho: Vec<String> = tela().lines().filter(|l| l.contains("agy ") || l.contains("Gemini") || l.contains("Claude") || l.contains("GPT")).map(str::to_owned).collect();
        println!("depois da troca:\n{}", cabecalho.join("\n"));
        let _ = stop_session(&state, &id);
        assert_eq!(status, 202);
    }

    /// Aplica (ou desfaz) a lista no agy de verdade, com a pasta do engine do app em `MEDZA_ENGINE_DIR`
    /// para o app lembrar o que acrescentou. `MEDZA_LIBERAR=0` desfaz.
    /// `MEDZA_ENGINE_DIR=… cargo test -p omni-engine aplicar_comandos_comuns -- --ignored --nocapture`
    #[test]
    #[ignore = "mexe no settings.json do agy de verdade: rodar à mão"]
    fn aplicar_comandos_comuns_no_agy_de_verdade() {
        let dir = PathBuf::from(std::env::var("MEDZA_ENGINE_DIR").expect("MEDZA_ENGINE_DIR"));
        let liberar = std::env::var("MEDZA_LIBERAR").map_or(true, |v| v != "0");
        let mudou = liberar_comandos_comuns(&crate::config_do_antigravity().unwrap(), &dir, liberar).unwrap();
        println!("{} regras {}", mudou, if liberar { "acrescentadas" } else { "tiradas" });
    }
}
