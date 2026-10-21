//! Fork (energia): long-poll do snapshot com uma sessão sintética de verdade no estado do engine.
use crate::*;
use crate::mobile::MobileRuntime;
use serde_json::json;
use std::time::{Duration, Instant};

struct Pty;
impl MasterPty for Pty {
    fn resize(&self, _: PtySize) -> Result<()> { Ok(()) }
    fn get_size(&self) -> Result<PtySize> { Ok(PtySize { rows: 20, cols: 100, pixel_width: 0, pixel_height: 0 }) }
    fn try_clone_reader(&self) -> Result<Box<dyn std::io::Read + Send>> { Ok(Box::new(std::io::empty())) }
    fn take_writer(&self) -> Result<Box<dyn std::io::Write + Send>> { Ok(Box::new(std::io::sink())) }
    #[cfg(unix)]
    fn process_group_leader(&self) -> Option<i32> { None }
    #[cfg(unix)]
    fn as_raw_fd(&self) -> Option<std::os::fd::RawFd> { None }
    #[cfg(unix)]
    fn tty_name(&self) -> Option<PathBuf> { None }
}
#[derive(Debug)]
struct Filho;
impl portable_pty::ChildKiller for Filho {
    fn kill(&mut self) -> std::io::Result<()> { Ok(()) }
    fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> { Box::new(Filho) }
}
impl Child for Filho {
    fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> { Ok(None) }
    fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> { Ok(portable_pty::ExitStatus::with_exit_code(0)) }
    fn process_id(&self) -> Option<u32> { None }
    #[cfg(windows)]
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> { None }
}

fn estado_com_sessao() -> (tempfile::TempDir, Arc<EngineState>, Arc<LiveSession>) {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: MobileRuntime::new(dir.path()),
        token: "test-only".into(), state_file: dir.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1) });
    // Shell puro e sem pid: o estado não muda sozinho com o tempo (sem consulta de processos).
    let meta = serde_json::from_value(json!({
        "id":"s", "project_id":"p", "name":"Sintética", "cwd":"/sintetico", "shell":"falso",
        "state":"answered", "pid":null, "created_at_ms":1, "last_activity_at_ms":1, "output_seq":0,
        "rows":20, "cols":100
    })).unwrap();
    let viva = Arc::new(LiveSession {
        interaction: Mutex::new(interaction::Interaction::new(20, 100)),
        reserved: std::sync::atomic::AtomicBool::new(false),
        headless: std::sync::atomic::AtomicBool::new(false), meta: Mutex::new(meta),
        writer: Mutex::new(Box::new(std::io::sink())), master: Mutex::new(Box::new(Pty)),
        child: Mutex::new(Box::new(Filho)),
        output: Mutex::new(OutputBuffer { chunks: VecDeque::new(), bytes: 0, next_seq: 0 }),
    });
    state.sessions.lock().unwrap().insert("s".into(), SessionEntry::Live(viva.clone()));
    (dir, state, viva)
}

fn pedir(state: &Arc<EngineState>, since: u64, espera_ms: u64, conhecido: Option<String>) -> (EngineResponse, Duration) {
    let inicio = Instant::now();
    let resposta = handle_request(EngineRequest::Snapshot {
        token: "test-only".into(), session_id: "s".into(), since, espera_ms, estado_conhecido: conhecido,
    }, state);
    (resposta, inicio.elapsed())
}

fn estado_de(resposta: &EngineResponse) -> String {
    match resposta {
        EngineResponse::Snapshot { estado: Some(estado), .. } => estado.clone(),
        outra => panic!("resposta inesperada: {outra:?}"),
    }
}

fn escrever(sessao: &LiveSession, texto: &str) {
    let mut saida = sessao.output.lock().unwrap();
    let seq = saida.next_seq;
    saida.next_seq += 1;
    saida.bytes += texto.len();
    saida.chunks.push_back((seq, texto.into()));
    drop(saida);
    super::avisar();
}

#[test]
fn sem_estado_conhecido_responde_na_hora_e_entrega_a_impressao() {
    let (_dir, state, _) = estado_com_sessao();
    let (resposta, levou) = pedir(&state, 0, 5_000, None);
    assert!(levou < Duration::from_millis(200), "{levou:?}");
    assert_eq!(estado_de(&resposta).len(), 16);
}

#[test]
fn espera_e_acorda_assim_que_a_pty_escreve() {
    let (_dir, state, sessao) = estado_com_sessao();
    let conhecido = estado_de(&pedir(&state, 0, 0, None).0);
    let escritor = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(150));
        escrever(&sessao, "olá\r\n");
    });
    let (resposta, levou) = pedir(&state, 0, 5_000, Some(conhecido));
    escritor.join().unwrap();
    assert!(levou >= Duration::from_millis(140) && levou < Duration::from_millis(700), "{levou:?}");
    match resposta {
        EngineResponse::Snapshot { data, next_seq, .. } => { assert_eq!(data, "olá\r\n"); assert_eq!(next_seq, 1); }
        outra => panic!("{outra:?}"),
    }
}

#[test]
fn saida_ja_disponivel_nao_espera() {
    let (_dir, state, sessao) = estado_com_sessao();
    let conhecido = estado_de(&pedir(&state, 0, 0, None).0);
    escrever(&sessao, "pronto");
    let (_, levou) = pedir(&state, 0, 5_000, Some(conhecido));
    assert!(levou < Duration::from_millis(200), "{levou:?}");
}

#[test]
fn mudanca_de_estado_sem_saida_acorda_no_ritmo_do_estado() {
    let (_dir, state, sessao) = estado_com_sessao();
    let conhecido = estado_de(&pedir(&state, 0, 0, None).0);
    let trava = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        sessao.reserved.store(true, std::sync::atomic::Ordering::SeqCst);
    });
    let (resposta, levou) = pedir(&state, 0, 5_000, Some(conhecido.clone()));
    trava.join().unwrap();
    assert!(levou < Duration::from_millis(900), "{levou:?}");
    assert_ne!(estado_de(&resposta), conhecido);
}

#[test]
fn nada_acontece_volta_no_prazo() {
    let (_dir, state, _) = estado_com_sessao();
    let conhecido = estado_de(&pedir(&state, 0, 0, None).0);
    let (resposta, levou) = pedir(&state, 0, 600, Some(conhecido.clone()));
    assert!(levou >= Duration::from_millis(590) && levou < Duration::from_millis(1_200), "{levou:?}");
    assert_eq!(estado_de(&resposta), conhecido);
}

#[test]
fn sessao_fechada_durante_a_espera_responde_logo() {
    let (_dir, state, _) = estado_com_sessao();
    let conhecido = estado_de(&pedir(&state, 0, 0, None).0);
    let fechar = { let state = state.clone(); std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(100));
        state.sessions.lock().unwrap().remove("s");
    }) };
    let (_, levou) = pedir(&state, 0, 5_000, Some(conhecido));
    fechar.join().unwrap();
    assert!(levou < Duration::from_millis(900), "{levou:?}");
}
