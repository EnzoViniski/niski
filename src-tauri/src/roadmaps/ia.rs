//! Fork (Roadmaps): a IA da entrevista, da geração e do painel ao lado. Roda a CLI oficial sem tela
//! (`claude -p` ou `codex exec`) na conta escolhida, como o Jarvis e os Fluxos: nada de chave de API
//! própria nem de token extraído.
//!
//! - Só busca e leitura da web (Claude: `--tools WebSearch,WebFetch`; Codex: sandbox só leitura e
//!   `web_search="live"`), numa pasta temporária vazia, longe de qualquer projeto.
//! - Nada vai para o histórico de conversas (`--no-session-persistence` / `--ephemeral`).
//! - Um pedido por chave; fechar o painel cancela (`roadmaps_ia_cancelar` mata o grupo do processo).

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

/// Um plano completo com pesquisa na web pode levar alguns minutos.
const LIMITE: Duration = Duration::from_secs(600);
const FERRAMENTAS_DA_WEB: &str = "WebSearch,WebFetch";
pub const CANCELADO: &str = "Pedido cancelado.";

/// Marcas que o Claude Code põe nos processos que abre (mesma lista de `omni-engine/src/main.rs`):
/// herdadas, o Claude daqui se veria como sessão filha.
const MARCAS_DE_SESSAO_DO_CLAUDE: [&str; 11] = [
    "CLAUDECODE", "CLAUDE_CODE_CHILD_SESSION", "CLAUDE_CODE_SESSION_ID", "CLAUDE_CODE_SESSION_ATTENDED",
    "CLAUDE_CODE_ENTRYPOINT", "CLAUDE_CODE_EXECPATH", "CLAUDE_CODE_MESSAGING_SOCKET",
    "CLAUDE_CODE_MESSAGING_TOKEN", "CLAUDE_CODE_SSE_PORT", "CLAUDE_PID", "CLAUDE_EFFORT",
];
const VARIAVEIS_DO_ENGINE: [&str; 3] = ["OMNI_ENGINE_PORT", "OMNI_DATA_DIR", "OMNI_ENGINE_PATH"];

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PedidoDeIa {
    /// Quem pediu (ex.: `entrevista:<id>`, `painel:<id>`): um pedido por chave, e é por ela que cancela.
    pub chave: String,
    pub cli: String,
    pub conta: Option<String>,
    pub modelo: Option<String>,
    pub sistema: String,
    pub texto: String,
    #[serde(default)]
    pub web: bool,
}

#[derive(Default)]
struct EmAndamento { pid: Option<u32>, cancelado: bool }

fn em_andamento() -> &'static Mutex<HashMap<String, EmAndamento>> {
    static MAPA: OnceLock<Mutex<HashMap<String, EmAndamento>>> = OnceLock::new();
    MAPA.get_or_init(Default::default)
}

/// Modelo vai como argumento: só nomes simples (`opus`, `gpt-5.5`, `claude-sonnet-5-5[1m]`).
fn modelo_seguro(modelo: &str) -> bool {
    !modelo.is_empty() && modelo.len() <= 80 && !modelo.starts_with('-')
        && modelo.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '.' | '_' | ':' | '[' | ']' | '/'))
}

/// Argumentos da CLI (sem o executável) e o texto que vai pela entrada padrão.
pub fn argumentos(cli: &str, modelo: Option<&str>, web: bool, sistema: &str, texto: &str) -> Result<(Vec<String>, String), String> {
    let modelo = modelo.map(str::trim).filter(|m| !m.is_empty());
    if let Some(m) = modelo { if !modelo_seguro(m) { return Err("Modelo inválido".into()); } }
    let mut args: Vec<String> = Vec::new();
    match cli {
        "claude" => {
            args.extend(["-p", "--output-format", "json", "--no-session-persistence", "--strict-mcp-config",
                "--setting-sources", "", "--permission-mode", "dontAsk"].map(String::from));
            args.push("--tools".into());
            args.push(if web { FERRAMENTAS_DA_WEB.into() } else { String::new() });
            if web { args.push("--allowedTools".into()); args.push(FERRAMENTAS_DA_WEB.into()); }
            if let Some(m) = modelo { args.push("--model".into()); args.push(m.into()); }
            args.push("--system-prompt".into());
            args.push(sistema.into());
            Ok((args, texto.into()))
        }
        "codex" => {
            args.extend(["exec", "--json", "--ephemeral", "--skip-git-repo-check", "--sandbox", "read-only", "--color", "never", "-c"].map(String::from));
            args.push(format!("web_search=\"{}\"", if web { "live" } else { "disabled" }));
            if let Some(m) = modelo { args.push("--model".into()); args.push(m.into()); }
            args.push("-".into());
            // O Codex sem tela não tem prompt de sistema na linha de comando: vai na frente do pedido.
            Ok((args, format!("{sistema}\n\n---\n\n{texto}")))
        }
        outra => Err(format!("A CLI “{outra}” ainda não funciona nos roadmaps")),
    }
}

/// Texto final da resposta. Claude: o JSON do `--output-format json`. Codex: a última mensagem do
/// agente no JSONL do `exec --json`.
pub fn resposta_da_saida(cli: &str, saida: &str) -> Result<String, String> {
    if cli == "claude" {
        let valor = saida.lines().rev().find_map(|l| serde_json::from_str::<serde_json::Value>(l.trim()).ok().filter(|v| v.is_object()))
            .or_else(|| serde_json::from_str::<serde_json::Value>(saida.trim()).ok())
            .ok_or("O Claude não devolveu uma resposta legível")?;
        let texto = valor["result"].as_str().unwrap_or_default().to_owned();
        if valor["is_error"] == true || valor["subtype"].as_str().is_some_and(|s| s.starts_with("error")) {
            return Err(if texto.is_empty() { "O Claude não conseguiu responder".into() } else { format!("O Claude não conseguiu responder: {texto}") });
        }
        return if texto.trim().is_empty() { Err("O Claude respondeu vazio; tente de novo".into()) } else { Ok(texto) };
    }
    let (mut texto, mut erro) = (None::<String>, None::<String>);
    for linha in saida.lines() {
        let Ok(ev) = serde_json::from_str::<serde_json::Value>(linha.trim()) else { continue };
        match ev["type"].as_str() {
            Some("item.completed") if ev["item"]["type"] == "agent_message" => texto = ev["item"]["text"].as_str().map(str::to_owned),
            Some("error") => erro = ev["message"].as_str().map(str::to_owned),
            Some("turn.failed") => erro = ev["error"]["message"].as_str().map(str::to_owned).or(erro),
            _ => {}
        }
    }
    match (texto.filter(|t| !t.trim().is_empty()), erro) {
        (Some(t), _) => Ok(t),
        (None, Some(e)) => Err(format!("O Codex não conseguiu responder: {e}")),
        (None, None) => Err("O Codex respondeu vazio; tente de novo".into()),
    }
}

fn matar_grupo(pid: u32) {
    #[cfg(unix)] { let _ = Command::new("kill").arg("-TERM").arg(format!("-{pid}")).stdout(Stdio::null()).stderr(Stdio::null()).status(); }
    #[cfg(windows)] {
        let mut c = Command::new("taskkill");
        c.args(["/F", "/T", "/PID", &pid.to_string()]);
        let _ = crate::git_client::hidden(&mut c).stdout(Stdio::null()).stderr(Stdio::null()).status();
    }
}

pub fn cancelar(chave: &str) {
    let pid = {
        let mut mapa = em_andamento().lock().unwrap_or_else(|e| e.into_inner());
        mapa.get_mut(chave).and_then(|e| { e.cancelado = true; e.pid })
    };
    if let Some(pid) = pid { matar_grupo(pid); }
}

/// Tira a chave do mapa ao sair, mesmo com erro no meio.
struct Liberar<'a>(&'a str);
impl Drop for Liberar<'_> {
    fn drop(&mut self) { em_andamento().lock().unwrap_or_else(|e| e.into_inner()).remove(self.0); }
}

/// Roda `exe args`, manda `entrada` e devolve o stdout inteiro. Separado de [`perguntar`] para os
/// testes usarem uma CLI falsa.
pub fn rodar(chave: &str, exe: &Path, args: &[String], entrada: &str, env: &[(String, String)], pasta: &Path, limite: Duration) -> Result<String, String> {
    {
        let mut mapa = em_andamento().lock().unwrap_or_else(|e| e.into_inner());
        if mapa.contains_key(chave) { return Err("Já tem um pedido em andamento aqui; espere ele terminar".into()); }
        mapa.insert(chave.to_owned(), EmAndamento::default());
    }
    let _liberar = Liberar(chave);
    let eh_script = exe.extension().is_some_and(|e| e.eq_ignore_ascii_case("cmd") || e.eq_ignore_ascii_case("bat"));
    let mut comando = if eh_script { let mut c = Command::new("cmd"); c.arg("/C").arg(exe); c } else { Command::new(exe) };
    for nome in VARIAVEIS_DO_ENGINE { comando.env_remove(nome); }
    if std::env::var_os("CLAUDECODE").is_some() || std::env::var_os("CLAUDE_CODE_CHILD_SESSION").is_some() {
        for nome in MARCAS_DE_SESSAO_DO_CLAUDE { comando.env_remove(nome); }
        comando.env_remove("CLAUDE_CONFIG_DIR");
    }
    crate::git_client::hidden(&mut comando).args(args).envs(env.iter().map(|(k, v)| (k, v)))
        .current_dir(pasta).stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)] { use std::os::unix::process::CommandExt; comando.process_group(0); }
    let mut filho = comando.spawn().map_err(|e| format!("Não consegui abrir a CLI: {e}"))?;
    {
        let mut mapa = em_andamento().lock().unwrap_or_else(|e| e.into_inner());
        let cancelado = mapa.get_mut(chave).map(|e| { e.pid = Some(filho.id()); e.cancelado }).unwrap_or(false);
        drop(mapa);
        if cancelado { matar_grupo(filho.id()); }
    }
    let mut stdin = filho.stdin.take().ok_or("sem stdin")?;
    let entrada = entrada.to_owned();
    // Pedido grande + CLI que ainda não leu: escrever numa thread não trava a espera.
    let escritor = std::thread::spawn(move || { let _ = stdin.write_all(entrada.as_bytes()); });
    let mut stdout = filho.stdout.take().ok_or("sem stdout")?;
    let leitor = std::thread::spawn(move || { let mut s = String::new(); let _ = stdout.read_to_string(&mut s); s });
    let mut stderr = filho.stderr.take().ok_or("sem stderr")?;
    let leitor_de_erro = std::thread::spawn(move || {
        let mut s = String::new(); let _ = stderr.read_to_string(&mut s);
        s.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or_default().chars().take(400).collect::<String>()
    });

    let inicio = Instant::now();
    let status = loop {
        if let Some(status) = filho.try_wait().map_err(|e| e.to_string())? { break status; }
        if inicio.elapsed() > limite {
            matar_grupo(filho.id());
            let _ = filho.kill();
            let _ = filho.wait();
            return Err("A IA demorou demais para responder; tente de novo".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    };
    let _ = escritor.join();
    let saida = leitor.join().unwrap_or_default();
    let erro = leitor_de_erro.join().unwrap_or_default();
    if em_andamento().lock().unwrap_or_else(|e| e.into_inner()).get(chave).is_some_and(|e| e.cancelado) {
        return Err(CANCELADO.into());
    }
    if !status.success() && saida.trim().is_empty() {
        return Err(format!("A CLI saiu com erro: {}", if erro.is_empty() { "sem detalhe" } else { &erro }));
    }
    Ok(saida)
}

fn executavel(cli: &str) -> Option<PathBuf> {
    match cli {
        "claude" => crate::agentes::executavel_do_claude(),
        "codex" => {
            omni_core::cli_path::ampliar_path();
            ["codex.exe", "codex.cmd", "codex"].into_iter().find_map(crate::engine_client::resolve_on_path)
        }
        _ => None,
    }
}

pub fn perguntar(pedido: &PedidoDeIa) -> Result<String, String> {
    if pedido.chave.trim().is_empty() || pedido.chave.len() > 120 { return Err("Pedido inválido".into()); }
    let (args, entrada) = argumentos(&pedido.cli, pedido.modelo.as_deref(), pedido.web, &pedido.sistema, &pedido.texto)?;
    let nome = if pedido.cli == "claude" { "Claude Code" } else { "Codex" };
    let exe = executavel(&pedido.cli).ok_or_else(|| format!("O {nome} não foi encontrado neste PC"))?;
    let perfis = crate::profiles::list_profiles();
    let perfil = perfis.iter().filter(|p| p.provider == pedido.cli).find(|p| pedido.conta.as_deref().is_some_and(|c| c == p.id))
        .or_else(|| perfis.iter().find(|p| p.provider == pedido.cli && p.authenticated))
        .ok_or_else(|| format!("Nenhuma conta do {nome} com login"))?;
    let pasta = std::env::temp_dir().join("medza-roadmaps-ia");
    std::fs::create_dir_all(&pasta).map_err(|e| e.to_string())?;
    let inicio = Instant::now();
    let resultado = rodar(&pedido.chave, &exe, &args, &entrada, &omni_core::env_for(perfil), &pasta, LIMITE)
        .and_then(|saida| resposta_da_saida(&pedido.cli, &saida));
    // Só metadados: CLI, se deu certo e quanto tempo levou. Nunca o texto.
    omni_core::registro::info("roadmap_ia", &[
        ("cli", &pedido.cli), ("web", if pedido.web { "sim" } else { "nao" }),
        ("ok", if resultado.is_ok() { "sim" } else { "nao" }), ("ms", &inicio.elapsed().as_millis().to_string()),
    ]);
    resultado
}

#[tauri::command(async)]
pub fn roadmaps_ia(pedido: PedidoDeIa) -> Result<String, String> { perguntar(&pedido) }

#[tauri::command(async)]
pub fn roadmaps_ia_cancelar(chave: String) { cancelar(&chave); }

#[cfg(test)]
#[path = "ia_testes.rs"]
mod testes;
