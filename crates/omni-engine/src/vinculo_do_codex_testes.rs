//! Fork (conversa do Codex): rollouts sintéticos no formato do Codex 0.164; nenhuma CLI real.

use super::*;
use omni_core::conversations::{write_index, Conversation, Segment, TitleSource};
use std::sync::{atomic::AtomicU64, Mutex};

fn rollout(pasta: &Path, id: &str, extra: serde_json::Value) {
    let mut payload = serde_json::json!({"id": id, "cwd": "/projeto", "timestamp": "2026-10-23T18:28:36Z"});
    payload.as_object_mut().unwrap().extend(extra.as_object().unwrap().clone());
    fs::write(pasta.join(format!("rollout-2026-10-23T15-28-36-{id}.jsonl")),
        format!("{}\n", serde_json::json!({"type": "session_meta", "payload": payload}))).unwrap();
}

#[test]
fn thread_do_notify_liga_a_conversa_e_ignora_subagente() {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: mobile::MobileRuntime::new(dir.path()),
        token: "t".into(), state_file: dir.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1) });
    let codex_home = dir.path().join("codex-home");
    let dia = codex_home.join("sessions/2026/10/23");
    fs::create_dir_all(&dia).unwrap();
    rollout(&dia, "01a16a05-a046", serde_json::json!({"source": "cli", "thread_source": "user"}));
    rollout(&dia, "01a16a05-abd3", serde_json::json!({"thread_source": "subagent", "parent_thread_id": "outra",
        "source": {"subagent": {"thread_spawn": {"parent_thread_id": "outra"}}}}));
    atomic_write_json(&dir.path().join("profiles.json"), &serde_json::json!({"profiles": [{"id": "codex-p", "provider": "codex",
        "name": "Codex", "config_dir": codex_home.to_string_lossy(), "builtin": false, "created_at_ms": 0}]})).unwrap();

    let sessao = spawn_terminal_inner(&state, state.next_id(), "p".into(), "Codex".into(), dir.path().to_string_lossy().into_owned(),
        None, None, 24, 80, now_ms(), SessionOrigin { provider: Some("codex".into()), profile_id: Some("codex-p".into()),
            conversation_id: Some("conv".into()), ..SessionOrigin::default() }).unwrap();
    // Reaberto minutos depois (`started_at_ms` velho): a busca por horário não acharia nada.
    write_index(dir.path(), &[Conversation { id: "conv".into(), project_id: "p".into(), cwd: "/projeto".into(), title: "x".into(),
        title_source: TitleSource::Auto, created_at_ms: 0, segments: vec![Segment { provider: "codex".into(),
            profile_id: Some("codex-p".into()), external_session_id: None, transcript_path: None,
            terminal_session_id: Some(sessao.id.clone()), started_at_ms: 1_000, ended_at_ms: None }] }]).unwrap();
    let viva = match state.sessions.lock().unwrap().get(&sessao.id) { Some(SessionEntry::Live(s)) => s.clone(), _ => panic!("sem sessão") };
    let trecho = || omni_core::conversations::read_index(dir.path())[0].segments[0].clone();

    vincular(&state, &viva, &sessao.id, "01a16a05-abd3");
    assert_eq!(trecho().external_session_id, None, "subagente não é a conversa da aba");
    vincular(&state, &viva, &sessao.id, "nao/valido");
    vincular(&state, &viva, &sessao.id, "sem-rollout");
    assert_eq!(trecho().external_session_id, None);

    vincular(&state, &viva, &sessao.id, "01a16a05-a046");
    let ligado = trecho();
    assert_eq!(ligado.external_session_id.as_deref(), Some("01a16a05-a046"));
    assert!(ligado.transcript_path.as_deref().unwrap().ends_with("rollout-2026-10-23T15-28-36-01a16a05-a046.jsonl"));
    assert_eq!(viva.meta.lock().unwrap().external_session_id.as_deref(), Some("01a16a05-a046"), "o reinício retoma este thread");

    // Outra aba (outro terminal) não mexe nesta conversa.
    vincular(&state, &viva, "terminal-de-outra-aba", "01a16a05-a046");
    assert_eq!(trecho().terminal_session_id.as_deref(), Some(sessao.id.as_str()));
    let _ = stop_session(&state, &sessao.id);
}
