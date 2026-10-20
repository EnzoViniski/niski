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
    Json(json!({"modo": modo(dir).nome(), "efetivo": modo_efetivo(dir).nome(), "auto_disponivel": !auto_indisponivel(dir)}))
}

async fn gravar(State(state): State<Arc<crate::EngineState>>, Json(corpo): Json<Value>) -> Result<Json<Value>, (StatusCode, Json<Value>)> {
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
}
