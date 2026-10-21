//! Fork (Editor de vídeo): roda uma CLI (agente ou render) lendo a saída linha a linha, com
//! cancelamento por chave (mata o grupo do processo), limite de tempo e prioridade baixa opcional
//! (`nice`, para o render não disputar o processador com a janela).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub const CANCELADO: &str = "Cancelado.";

pub enum Linha { Saida(String), Erro(String) }

pub struct Execucao<'a> {
    pub chave: &'a str,
    pub exe: &'a Path,
    pub args: &'a [String],
    pub entrada: Option<String>,
    pub env: &'a [(String, String)],
    pub pasta: &'a Path,
    pub limite: Duration,
    pub prioridade_baixa: bool,
}

#[derive(Default)]
struct EmAndamento { pid: Option<u32>, cancelado: bool }

fn em_andamento() -> &'static Mutex<HashMap<String, EmAndamento>> {
    static MAPA: OnceLock<Mutex<HashMap<String, EmAndamento>>> = OnceLock::new();
    MAPA.get_or_init(Default::default)
}

pub fn rodando(chave: &str) -> bool { em_andamento().lock().unwrap_or_else(|e| e.into_inner()).contains_key(chave) }

pub fn cancelar(chave: &str) {
    let pid = {
        let mut mapa = em_andamento().lock().unwrap_or_else(|e| e.into_inner());
        mapa.get_mut(chave).and_then(|e| { e.cancelado = true; e.pid })
    };
    if let Some(pid) = pid { crate::roadmaps::ia::matar_grupo(pid); }
}

struct Liberar<'a>(&'a str);
impl Drop for Liberar<'_> {
    fn drop(&mut self) { em_andamento().lock().unwrap_or_else(|e| e.into_inner()).remove(self.0); }
}

pub struct Fim { pub sucesso: bool, pub ultima_linha_de_erro: String }

/// Roda e chama `ao_ler` a cada linha (stdout e stderr). Erro se cancelado, se passou do limite
/// ou se nem abriu.
pub fn rodar(e: Execucao<'_>, mut ao_ler: impl FnMut(Linha)) -> Result<Fim, String> {
    {
        let mut mapa = em_andamento().lock().unwrap_or_else(|e| e.into_inner());
        if mapa.contains_key(e.chave) { return Err("Já tem um pedido em andamento aqui; espere ele terminar.".into()); }
        mapa.insert(e.chave.to_owned(), EmAndamento::default());
    }
    let _liberar = Liberar(e.chave);
    let eh_script = e.exe.extension().is_some_and(|x| x.eq_ignore_ascii_case("cmd") || x.eq_ignore_ascii_case("bat"));
    let mut comando = if eh_script {
        let mut c = Command::new("cmd"); c.arg("/C").arg(e.exe); c
    } else if e.prioridade_baixa && cfg!(unix) && Path::new("/usr/bin/nice").is_file() {
        let mut c = Command::new("/usr/bin/nice"); c.args(["-n", "10"]).arg(e.exe); c
    } else {
        Command::new(e.exe)
    };
    for nome in crate::roadmaps::ia::VARIAVEIS_DO_ENGINE { comando.env_remove(nome); }
    if std::env::var_os("CLAUDECODE").is_some() || std::env::var_os("CLAUDE_CODE_CHILD_SESSION").is_some() {
        for nome in crate::roadmaps::ia::MARCAS_DE_SESSAO_DO_CLAUDE { comando.env_remove(nome); }
        comando.env_remove("CLAUDE_CONFIG_DIR");
    }
    crate::git_client::hidden(&mut comando).args(e.args).envs(e.env.iter().map(|(k, v)| (k, v)))
        .current_dir(e.pasta).stdin(if e.entrada.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(unix)] { use std::os::unix::process::CommandExt; comando.process_group(0); }
    let mut filho = comando.spawn().map_err(|erro| format!("Não consegui abrir a CLI: {erro}"))?;
    {
        let mut mapa = em_andamento().lock().unwrap_or_else(|e| e.into_inner());
        let cancelado = mapa.get_mut(e.chave).map(|x| { x.pid = Some(filho.id()); x.cancelado }).unwrap_or(false);
        drop(mapa);
        if cancelado { crate::roadmaps::ia::matar_grupo(filho.id()); }
    }
    if let (Some(texto), Some(mut stdin)) = (e.entrada, filho.stdin.take()) {
        // Escrever numa thread: CLI que ainda não leu não trava a espera.
        std::thread::spawn(move || { let _ = stdin.write_all(texto.as_bytes()); });
    }
    let (tx, rx) = mpsc::channel::<(bool, String)>();
    let saida = filho.stdout.take().ok_or("sem stdout")?;
    let erro = filho.stderr.take().ok_or("sem stderr")?;
    let tx2 = tx.clone();
    let leitor = std::thread::spawn(move || { for l in BufReader::new(saida).lines().map_while(Result::ok) { if tx.send((false, l)).is_err() { break; } } });
    let leitor_de_erro = std::thread::spawn(move || {
        // Render escreve progresso com `\r`: cada pedaço vira uma linha.
        let mut leitor = BufReader::new(erro);
        let mut bloco = Vec::new();
        loop {
            bloco.clear();
            match leitor.read_until(b'\n', &mut bloco) {
                Ok(0) | Err(_) => break,
                Ok(_) => for pedaco in String::from_utf8_lossy(&bloco).split('\r') {
                    let p = pedaco.trim_end_matches('\n');
                    if !p.trim().is_empty() && tx2.send((true, p.to_owned())).is_err() { return; }
                },
            }
        }
    });
    let inicio = Instant::now();
    let mut ultima = String::new();
    let status = loop {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok((de_erro, linha)) => {
                if de_erro { ultima = linha.chars().take(400).collect(); ao_ler(Linha::Erro(linha)); } else { ao_ler(Linha::Saida(linha)); }
                continue;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {}
        }
        if let Some(status) = filho.try_wait().map_err(|x| x.to_string())? {
            // Sobrou filho com a saída aberta (node do npx, Chrome do render)? Sai junto, senão a leitura não acaba.
            crate::roadmaps::ia::matar_grupo(filho.id());
            let _ = leitor.join();
            let _ = leitor_de_erro.join();
            for (de_erro, linha) in rx.try_iter() {
                if de_erro { ultima = linha.chars().take(400).collect(); ao_ler(Linha::Erro(linha)); } else { ao_ler(Linha::Saida(linha)); }
            }
            break status;
        }
        if inicio.elapsed() > e.limite {
            crate::roadmaps::ia::matar_grupo(filho.id());
            let _ = filho.kill();
            let _ = filho.wait();
            return Err("Demorou demais; tente de novo.".into());
        }
    };
    if em_andamento().lock().unwrap_or_else(|e| e.into_inner()).get(e.chave).is_some_and(|x| x.cancelado) {
        return Err(CANCELADO.into());
    }
    Ok(Fim { sucesso: status.success(), ultima_linha_de_erro: ultima })
}

/// `npx` no PATH ampliado (o app aberto pelo Finder não herda o PATH do terminal).
pub fn npx() -> Option<PathBuf> {
    omni_core::cli_path::ampliar_path();
    ["npx.cmd", "npx"].into_iter().find_map(crate::engine_client::resolve_on_path)
}

/// Ambiente do HyperFrames: telemetria e checagem de skills desligadas.
pub fn ambiente_do_hyperframes() -> Vec<(String, String)> {
    vec![("HYPERFRAMES_NO_TELEMETRY".into(), "1".into()), ("HYPERFRAMES_SKIP_SKILLS".into(), "1".into())]
}
