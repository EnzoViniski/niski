//! Regressões com shell/CLI falsos: nenhuma conta ou sessão real.
use super::*;
use crate::mobile::MobileRuntime;

fn fixture() -> (tempfile::TempDir, Arc<EngineState>) {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState {
        persist_lock: Mutex::new(()),
        mobile: MobileRuntime::new(dir.path()),
        token: "fixture".into(),
        state_file: dir.path().join("sessions.json"),
        sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1),
    });
    (dir, state)
}
fn viva(state: &EngineState, id: &str) -> Arc<LiveSession> {
    match state.sessions.lock().unwrap().get(id).unwrap() {
        SessionEntry::Live(s) => s.clone(),
        _ => panic!(),
    }
}
fn esperar(teste: impl Fn() -> bool) {
    let inicio = std::time::Instant::now();
    while !teste() {
        assert!(inicio.elapsed().as_secs() < 8, "prazo da CLI falsa");
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
}

#[cfg(unix)]
#[test]
fn parar_espera_cli_falsa_soltar_trava_sem_tocar_em_outro_terminal() {
    let (dir, state) = fixture();
    let cwd = dir.path().to_string_lossy().to_string();
    // zsh cria um grupo de primeiro plano diferente do shell (como as CLIs oficiais).
    let script = dir.path().join("cli-falsa.sh");
    fs::write(&script,"#!/bin/sh\nmkdir trava\ntrap 'rmdir trava; exit 0' TERM HUP\necho pronta\nwhile :; do sleep 0.1; done\n").unwrap();
    spawn_terminal_inner(
        &state,
        "teste".into(),
        "p".into(),
        "CLI falsa".into(),
        cwd.clone(),
        Some("/bin/zsh".into()),
        Some("/bin/sh cli-falsa.sh".into()),
        24,
        80,
        now_ms(),
        SessionOrigin::default(),
    )
    .unwrap();
    spawn_terminal_inner(
        &state,
        "outro".into(),
        "p".into(),
        "Outro".into(),
        cwd,
        Some("/bin/sh".into()),
        None,
        24,
        80,
        now_ms(),
        SessionOrigin::default(),
    )
    .unwrap();
    esperar(|| dir.path().join("trava").is_dir());
    let sessao = viva(&state, "teste");
    let outro = viva(&state, "outro");
    encerrar(&sessao).unwrap();
    assert!(
        !dir.path().join("trava").exists(),
        "CLI já soltou a trava antes do novo spawn"
    );
    assert!(sessao.child.lock().unwrap().try_wait().unwrap().is_some());
    assert!(
        outro.child.lock().unwrap().try_wait().unwrap().is_none(),
        "outra sessão segue viva"
    );
    let _ = stop_session(&state, "outro");
}

#[test]
fn trocar_muito_rapido_nao_digita_o_comando_antigo_na_pty_nova() {
    let (dir, state) = fixture();
    spawn_terminal_inner(
        &state,
        "rapida".into(),
        "p".into(),
        "Falsa".into(),
        dir.path().to_string_lossy().into_owned(),
        Some("/bin/sh".into()),
        Some("echo comando-velho > velho.txt".into()),
        24,
        80,
        now_ms(),
        SessionOrigin::default(),
    )
    .unwrap();
    restart_session_com_comando(
        &state,
        "rapida",
        Some("echo comando-novo > novo.txt".into()),
    )
    .unwrap();
    esperar(|| dir.path().join("novo.txt").exists());
    assert!(
        !dir.path().join("velho.txt").exists(),
        "thread antiga não escreve na PTY nova"
    );
    let _ = stop_session(&state, "rapida");
}
