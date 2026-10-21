//! Fork: nome da conversa dado pelo próprio agente, no lugar do começo do primeiro prompt.
//!
//! - **Claude Code** grava no transcript `{"type":"ai-title","aiTitle":…}` logo depois do primeiro
//!   prompt (e regrava ao longo da conversa); o `/rename` da CLI grava `{"type":"custom-title",
//!   "customTitle":…}`, que vence. O último de cada um fica perto do fim do arquivo — a própria CLI
//!   lê só o fim (CHANGELOG 2.1.239) —, então basta ler os últimos 64 KB.
//! - **Codex** (a TUI) gera o nome da thread e acrescenta `{"id","thread_name","updated_at"}` em
//!   `<CODEX_HOME>/session_index.jsonl`; a linha mais nova do mesmo id vence.
//!
//! Sem título (CLI headless, geração que ainda não aconteceu ou falhou): `None`, e quem chama cai no
//! primeiro prompt como antes. Renome feito no app (`TitleSource::Manual`) continua vencendo tudo
//! (`conversations::display_title`). Cache por (caminho, tamanho, data): `display_title` roda a cada
//! segundo para cada sessão da lateral.
use crate::conversations::{Conversation, Segment};
use serde_json::Value;
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::SystemTime;

const FIM_LIDO: u64 = 64 * 1024;
const TITULO_MAX: usize = 60;

type Carimbo = (u64, Option<SystemTime>);
static CACHE: OnceLock<Mutex<HashMap<PathBuf, (Carimbo, CacheValor)>>> = OnceLock::new();

#[derive(Clone)]
enum CacheValor { Claude(Option<String>), Codex(HashMap<String, String>) }

fn carimbo(caminho: &Path) -> Option<Carimbo> {
    let meta = std::fs::metadata(caminho).ok()?;
    Some((meta.len(), meta.modified().ok()))
}

fn com_cache(caminho: &Path, ler: impl FnOnce() -> CacheValor) -> Option<CacheValor> {
    let atual = carimbo(caminho)?;
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Ok(mapa) = cache.lock() {
        if let Some((salvo, valor)) = mapa.get(caminho) {
            if *salvo == atual { return Some(valor.clone()); }
        }
    }
    let valor = ler();
    if let Ok(mut mapa) = cache.lock() {
        if mapa.len() > 512 { mapa.clear(); }
        mapa.insert(caminho.to_path_buf(), (atual, valor.clone()));
    }
    Some(valor)
}

fn limpar(titulo: &str) -> Option<String> {
    let linha = titulo.lines().map(str::trim).find(|l| !l.is_empty())?;
    if linha.chars().any(char::is_control) { return None; }
    Some(match linha.char_indices().nth(TITULO_MAX) {
        Some((corte, _)) => format!("{}…", linha[..corte].trim_end()),
        None => linha.to_string(),
    })
}

/// Último `custom-title` (renome na CLI) ou, sem ele, último `ai-title` do fim do transcript.
pub fn titulo_do_claude(transcript: &Path) -> Option<String> {
    let valor = com_cache(transcript, || CacheValor::Claude(ler_titulo_do_claude(transcript)))?;
    match valor { CacheValor::Claude(titulo) => titulo, CacheValor::Codex(_) => None }
}

fn ler_titulo_do_claude(transcript: &Path) -> Option<String> {
    let mut arquivo = File::open(transcript).ok()?;
    let tamanho = arquivo.metadata().ok()?.len();
    let inicio = tamanho.saturating_sub(FIM_LIDO);
    arquivo.seek(SeekFrom::Start(inicio)).ok()?;
    let mut bytes = Vec::new();
    arquivo.take(FIM_LIDO).read_to_end(&mut bytes).ok()?;
    let texto = String::from_utf8_lossy(&bytes);
    // Começou no meio do arquivo: a primeira linha está cortada.
    let linhas = texto.lines().skip(usize::from(inicio > 0));
    let (mut ai, mut custom) = (None, None);
    for linha in linhas {
        let campo = if linha.contains("\"type\":\"custom-title\"") { "customTitle" }
            else if linha.contains("\"type\":\"ai-title\"") { "aiTitle" }
            else { continue };
        let Some(titulo) = serde_json::from_str::<Value>(linha).ok().and_then(|v| v[campo].as_str().and_then(limpar)) else { continue };
        if campo == "customTitle" { custom = Some(titulo) } else { ai = Some(titulo) }
    }
    custom.or(ai)
}

/// `<CODEX_HOME>/sessions/AAAA/MM/DD/rollout-….jsonl` → `<CODEX_HOME>`.
fn codex_home(rollout: &Path) -> Option<PathBuf> {
    let home = rollout.ancestors().nth(5)?;
    (rollout.ancestors().nth(4)?.file_name()? == "sessions").then(|| home.to_path_buf())
}

/// Nome da thread no `session_index.jsonl` (a linha mais nova do id vence).
pub fn titulo_do_codex(rollout: &Path, thread_id: &str) -> Option<String> {
    let indice = codex_home(rollout)?.join("session_index.jsonl");
    let valor = com_cache(&indice, || CacheValor::Codex(ler_indice_do_codex(&indice)))?;
    match valor { CacheValor::Codex(nomes) => nomes.get(thread_id).cloned(), CacheValor::Claude(_) => None }
}

fn ler_indice_do_codex(indice: &Path) -> HashMap<String, String> {
    let Ok(arquivo) = File::open(indice) else { return HashMap::new() };
    let mut nomes = HashMap::new();
    for linha in BufReader::new(arquivo).lines().map_while(Result::ok) {
        let Ok(valor) = serde_json::from_str::<Value>(&linha) else { continue };
        let (Some(id), Some(nome)) = (valor["id"].as_str(), valor["thread_name"].as_str().and_then(limpar)) else { continue };
        nomes.insert(id.to_owned(), nome);
    }
    nomes
}

fn do_segmento(segmento: &Segment) -> Option<String> {
    let transcript = Path::new(segmento.transcript_path.as_deref()?);
    match segmento.provider.as_str() {
        "claude" => titulo_do_claude(transcript),
        "codex" => titulo_do_codex(transcript, segmento.external_session_id.as_deref()?),
        _ => None,
    }
}

/// Título do agente para a conversa: o do primeiro segmento que tiver um (trocar de conta não muda o
/// nome que já apareceu).
pub fn titulo_do_agente(conversa: &Conversation) -> Option<String> {
    conversa.segments.iter().find_map(do_segmento)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::conversations::{display_title, TitleSource};
    use std::fs;

    fn conversa(segmento: Segment) -> Conversation {
        Conversation { id: "c".into(), project_id: "p".into(), cwd: ".".into(), title: "Conversa".into(),
            title_source: TitleSource::Auto, created_at_ms: 0, segments: vec![segmento] }
    }
    fn segmento(provider: &str, transcript: &Path, externo: Option<&str>) -> Segment {
        Segment { provider: provider.into(), profile_id: None, external_session_id: externo.map(str::to_owned),
            transcript_path: Some(transcript.to_string_lossy().into_owned()), terminal_session_id: None, started_at_ms: 0, ended_at_ms: None }
    }
    const PROMPT: &str = r#"{"type":"user","message":{"role":"user","content":"conserta o login do celular que cai quando troca de rede"}}"#;

    #[test]
    fn claude_usa_o_ultimo_ai_title_e_o_renome_da_cli_vence() {
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join("s.jsonl");
        fs::write(&t, format!("{PROMPT}\n{{\"type\":\"ai-title\",\"aiTitle\":\"Velho\",\"sessionId\":\"s\"}}\n{{\"type\":\"ai-title\",\"aiTitle\":\"Login no celular\",\"sessionId\":\"s\"}}\n")).unwrap();
        let c = conversa(segmento("claude", &t, Some("s")));
        assert_eq!(display_title(&c), "Login no celular");

        fs::write(&t, format!("{PROMPT}\n{{\"type\":\"ai-title\",\"aiTitle\":\"Login no celular\"}}\n{{\"type\":\"custom-title\",\"customTitle\":\"Meu nome\"}}\n{{\"type\":\"ai-title\",\"aiTitle\":\"Outro\"}}\n")).unwrap();
        assert_eq!(display_title(&c), "Meu nome");
    }

    #[test]
    fn sem_titulo_do_agente_fica_o_primeiro_prompt_e_o_manual_vence_tudo() {
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join("s.jsonl");
        fs::write(&t, format!("{PROMPT}\n")).unwrap();
        let mut c = conversa(segmento("claude", &t, Some("s")));
        assert!(display_title(&c).starts_with("conserta o login"));

        fs::write(&t, format!("{PROMPT}\n{{\"type\":\"ai-title\",\"aiTitle\":\"Login no celular\"}}\n")).unwrap();
        c.title = "Escolhido por mim".into();
        c.title_source = TitleSource::Manual;
        assert_eq!(display_title(&c), "Escolhido por mim");
    }

    #[test]
    fn claude_acha_o_titulo_no_fim_de_um_transcript_grande() {
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join("s.jsonl");
        let enchimento = format!("{}\n", r#"{"type":"assistant","message":{"role":"assistant","content":"x"}}"#).repeat(4_000);
        fs::write(&t, format!("{PROMPT}\n{enchimento}{{\"type\":\"ai-title\",\"aiTitle\":\"Título no fim\"}}\n")).unwrap();
        assert!(fs::metadata(&t).unwrap().len() > 200_000);
        assert_eq!(titulo_do_claude(&t).as_deref(), Some("Título no fim"));
    }

    #[test]
    fn codex_usa_o_nome_mais_novo_da_thread() {
        let dir = tempfile::tempdir().unwrap();
        let pasta = dir.path().join("sessions/2026/10/21");
        fs::create_dir_all(&pasta).unwrap();
        let rollout = pasta.join("rollout-x.jsonl");
        fs::write(&rollout, r#"{"type":"session_meta","payload":{"id":"t1"}}"#).unwrap();
        fs::write(dir.path().join("session_index.jsonl"),
            "{\"id\":\"t1\",\"thread_name\":\"Primeiro\",\"updated_at\":\"a\"}\n{\"id\":\"t2\",\"thread_name\":\"Outra\"}\n{\"id\":\"t1\",\"thread_name\":\"Revisar app-server\",\"updated_at\":\"b\"}\n").unwrap();
        assert_eq!(titulo_do_codex(&rollout, "t1").as_deref(), Some("Revisar app-server"));
        assert_eq!(titulo_do_codex(&rollout, "t3"), None);
        let c = conversa(segmento("codex", &rollout, Some("t1")));
        assert_eq!(titulo_do_agente(&c).as_deref(), Some("Revisar app-server"));
    }

    #[test]
    fn titulo_comprido_ou_estranho_e_cortado_ou_ignorado() {
        assert_eq!(limpar(&"a".repeat(80)).unwrap().chars().count(), TITULO_MAX + 1);
        assert_eq!(limpar("  \n  Bom título \n"), Some("Bom título".into()));
        assert_eq!(limpar("   "), None);
    }
}
