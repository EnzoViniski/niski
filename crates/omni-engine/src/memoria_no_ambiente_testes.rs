//! Fork (harness, A3): o Codex recebe a memória pelo ambiente da PTY, expandida pelo shell de verdade.
//! CLI falsa (`sh`) que grava os argumentos que recebeu; nenhuma CLI real.

use super::*;
use std::sync::{atomic::AtomicU64, Mutex};

#[test]
fn codex_falso_recebe_a_instrucao_inteira_sem_o_shell_reavaliar() {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: mobile::MobileRuntime::new(dir.path()),
        token: "sintetico".into(), state_file: dir.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1) });
    let texto = "Enzo \"aspas\" 'simples' $(touch PERIGO) `touch CRASE` $HOME \\ fim\nsegunda linha";
    memoria::salvar(dir.path(), &memoria::Memoria { ativa: true, texto: texto.into() }).unwrap();
    let codex_home = dir.path().join("codex-home");
    fs::create_dir_all(&codex_home).unwrap();
    let projeto = dir.path().join("projeto");
    fs::create_dir_all(&projeto).unwrap();
    let saida = dir.path().join("argumentos.txt");
    let script = dir.path().join("codex-falso");
    fs::write(&script, format!("#!/bin/sh\nfor a in \"$@\"; do printf '%s\\n<<fim>>\\n' \"$a\"; done > '{}'\nsleep 5\n", saida.display())).unwrap();
    let origem = SessionOrigin { provider: Some("codex".into()), env: vec![("CODEX_HOME".into(), codex_home.to_string_lossy().into_owned())], ..Default::default() };
    let terminal = format!("terminal-teste-{}", uuid::Uuid::new_v4());
    spawn_terminal_inner(&state, terminal.clone(), "p".into(), "Codex".into(), projeto.to_string_lossy().into_owned(),
        Some("/bin/sh".into()), Some(format!("/bin/sh '{}' resume abc", script.display())), 30, 100, now_ms(), origem).unwrap();
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(15);
    while !fs::read_to_string(&saida).is_ok_and(|s| s.matches("<<fim>>").count() >= 4) && std::time::Instant::now() < limite {
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    let recebido = fs::read_to_string(&saida).expect("a CLI falsa rodou");
    let argumentos: Vec<&str> = recebido.split("\n<<fim>>\n").filter(|a| !a.is_empty()).collect();
    assert_eq!(&argumentos[..3], ["resume", "abc", "-c"], "{recebido}");
    // O valor é o mesmo que `omni_core::memoria_compartilhada::instrucao_codex` monta (string JSON, que é TOML válido).
    let instrucao: String = serde_json::from_str(argumentos[3].strip_prefix("developer_instructions=").unwrap()).unwrap();
    assert!(instrucao.contains(texto), "texto chegou igual: {instrucao}");
    assert!(!projeto.join("PERIGO").exists() && !projeto.join("CRASE").exists(), "o shell não executou nada do texto");
    // O comando persistido continua o original (é ele que a retomada lê).
    let persistido = fs::read_to_string(dir.path().join("sessions.json")).unwrap();
    assert!(!persistido.contains("Enzo") && !persistido.contains(VARIAVEL_DO_CODEX));
    let _ = close_session(&state, &terminal);
}
