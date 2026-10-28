//! Regressão de sessões encerradas ainda guardadas como Live, sem PTY ou ps reais.
use super::*;

#[derive(Debug)]
struct Filho(bool);
impl portable_pty::ChildKiller for Filho {
    fn kill(&mut self) -> std::io::Result<()> { Ok(()) }
    fn clone_killer(&self) -> Box<dyn portable_pty::ChildKiller + Send + Sync> { Box::new(Filho(self.0)) }
}
impl Child for Filho {
    fn try_wait(&mut self) -> std::io::Result<Option<portable_pty::ExitStatus>> {
        Ok(self.0.then(|| portable_pty::ExitStatus::with_exit_code(0)))
    }
    fn wait(&mut self) -> std::io::Result<portable_pty::ExitStatus> { Ok(portable_pty::ExitStatus::with_exit_code(0)) }
    fn process_id(&self) -> Option<u32> { Some(42) }
    #[cfg(windows)]
    fn as_raw_handle(&self) -> Option<std::os::windows::io::RawHandle> { None }
}
struct Terminal;
impl MasterPty for Terminal {
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

fn sessao(state: &Arc<EngineState>, estado: &str, pid: Option<u32>, encerrado: bool) {
    let meta = serde_json::from_value(json!({
        "id":"s", "project_id":"p", "name":"Sintética", "cwd":"/sintetico", "shell":"falso",
        "state":estado, "pid":pid, "created_at_ms":1, "last_activity_at_ms":1, "output_seq":0,
        "rows":20, "cols":100, "provider":"claude", "conversation_id":"conv-teste"
    })).unwrap();
    let viva = Arc::new(LiveSession {
        interaction: Mutex::new(interaction::Interaction::new(20, 100)),
        reserved: std::sync::atomic::AtomicBool::new(false),
        headless: std::sync::atomic::AtomicBool::new(true), meta: Mutex::new(meta),
        writer: Mutex::new(Box::new(std::io::sink())), master: Mutex::new(Box::new(Terminal)),
        child: Mutex::new(Box::new(Filho(encerrado))),
        output: Mutex::new(OutputBuffer { chunks: VecDeque::new(), bytes: 0, next_seq: 0 }),
    });
    state.sessions.lock().unwrap().insert("s".into(), SessionEntry::Live(viva));
    let dir = state.state_file.parent().unwrap();
    let mut indice = omni_core::conversations::read_index(dir);
    indice[0].segments.push(omni_core::conversations::Segment {
        provider: "claude".into(), profile_id: None, external_session_id: None,
        transcript_path: None, terminal_session_id: Some("s".into()), started_at_ms: 1, ended_at_ms: None,
    });
    omni_core::conversations::write_index(dir, &indice).unwrap();
}

#[test]
fn live_encerrada_tenta_retomada_em_vez_de_entregar_em_terminal_morto() {
    for (estado, pid, encerrado) in [("stopped", None, false), ("crashed", None, true), ("answered", Some(42), true), ("answered", None, false)] {
        let (_dir, state) = fixture();
        sessao(&state, estado, pid, encerrado);
        // Sem external_session_id: tentar retomar deve falhar explicitamente, nunca adiar o envio
        // para uma PTY morta. A retomada com id é coberta pela CLI falsa no teste de integração.
        let (_, Json(erro)) = preparar(&state, &agenda(Regra::UmaVez { em_ms: 1_000 })).unwrap_err();
        assert!(erro["error"].as_str().unwrap().contains("mensagens para retomar"), "{erro}");
        assert!(!state.sessions.lock().unwrap().contains_key("s"));
    }
}

#[test]
fn live_ativa_e_reutilizada_sem_retomar() {
    let (_dir, state) = fixture();
    sessao(&state, "answered", Some(42), false);
    assert_eq!(preparar(&state, &agenda(Regra::UmaVez { em_ms: 1_000 })).unwrap(), "conv-teste");
    assert!(state.sessions.lock().unwrap().contains_key("s"));
}
