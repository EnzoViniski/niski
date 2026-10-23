//! Fork: diagnóstico do chat do Codex com a CLI **real** (testes ignorados, rodar à mão). O Enzo relatou
//! que o Codex "buga quando inicia" e que não consegue trocar o modelo pela conversa.

use crate::*;
use serde_json::{json, Value};

fn tela(state: &EngineState, id: &str) -> String {
    match state.sessions.lock().unwrap().get(id) {
        Some(SessionEntry::Live(s)) => s.interaction.lock().unwrap().parser.screen().contents(),
        _ => String::new(),
    }
}

fn viva(state: &EngineState, id: &str) -> Arc<LiveSession> {
    match state.sessions.lock().unwrap().get(id) { Some(SessionEntry::Live(s)) => s.clone(), _ => panic!("sem sessão") }
}

fn imprimir(rotulo: &str, texto: &str) {
    println!("=== {rotulo}");
    for (i, linha) in texto.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) { println!("{i:02}|{linha}"); }
}

/// Abre o Codex como o app abre, mostra a tela de subida, manda "responda só: ok" pela rota do chat e
/// troca o modelo pela rota de ajustes. Conta Padrão (`~/.codex`), pasta `~/omni`.
/// `cargo test -p omni-engine chat_do_codex -- --ignored --nocapture`
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "CLI real (codex): rodar à mão"]
async fn chat_do_codex_inicia_manda_e_troca_modelo() {
    let dados = tempfile::tempdir().unwrap();
    let casa = PathBuf::from(std::env::var("HOME").unwrap());
    let pasta = casa.join("omni").to_string_lossy().into_owned();
    let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: mobile::MobileRuntime::new(dados.path()),
        token: "test-only".into(), state_file: dados.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1) });
    mobile::start(state.clone());
    atomic_write_json(&dados.path().join("profiles.json"), &json!({"profiles": [{"id": "codex-padrao", "provider": "codex",
        "name": "Padrão", "config_dir": casa.join(".codex").to_string_lossy(), "builtin": true, "created_at_ms": 0}]})).unwrap();
    mobile::publish(&state, vec![omni_protocol::PublishedProject { id: "p".into(), name: "omni".into(), path: pasta.clone() }],
        vec![omni_protocol::PublishedAgent { id: "codex".into(), label: "Codex".into(), command: "codex".into(), resume: None }], None).unwrap();
    let id = state.next_id();
    let conversa = omni_core::conversations::Conversation { id: "c-codex".into(), project_id: "p".into(), cwd: pasta.clone(),
        title: "teste codex".into(), title_source: omni_core::conversations::TitleSource::Manual, created_at_ms: now_ms(),
        segments: vec![omni_core::conversations::Segment { provider: "codex".into(), profile_id: Some("codex-padrao".into()),
            external_session_id: None, transcript_path: None, terminal_session_id: Some(id.clone()), started_at_ms: now_ms(), ended_at_ms: None }] };
    omni_core::conversations::write_index(dados.path(), &[conversa]).unwrap();
    spawn_terminal_inner(&state, id.clone(), "p".into(), "codex".into(), pasta.clone(), None,
        Some("codex -m gpt-6-luna -c model_reasoning_effort=low".into()), 40, 120, now_ms(),
        SessionOrigin { headless: true, provider: Some("codex".into()), profile_id: Some("codex-padrao".into()),
            conversation_id: Some("c-codex".into()), ..SessionOrigin::default() }).unwrap();
    let espera = |ms: u64| tokio::time::sleep(std::time::Duration::from_millis(ms));

    // Subida: tela e capacidades a cada segundo, até ficar pronta (ou 45 s).
    let inicio = std::time::Instant::now();
    let mut ultimo_motivo = String::new();
    let mut pronta = false;
    while inicio.elapsed().as_secs() < 45 {
        espera(1000).await;
        let cap = interaction::capabilities(&viva(&state, &id));
        let motivo = format!("pronta={} aprovar={} menu={:?} motivo={:?}", cap.prompt, cap.approve,
            cap.escolha.as_ref().map(|m| m.pergunta.clone()), cap.reason);
        if motivo != ultimo_motivo { println!("[{:>2}s] {motivo}", inicio.elapsed().as_secs()); ultimo_motivo = motivo; }
        if cap.prompt { pronta = true; break; }
    }
    imprimir(&format!("tela depois de {}s", inicio.elapsed().as_secs()), &tela(&state, &id));
    if !pronta { let _ = stop_session(&state, &id); panic!("o Codex não ficou pronto para mensagem"); }

    let post = |rota: String, corpo: Value| { let state = state.clone(); let id = id.clone(); async move {
        let revisao = interaction::capabilities(&viva(&state, &id)).revision;
        mobile::local(state, "POST", &rota, Some(corpo.to_string()), None,
            vec![("idempotency-key".into(), format!("t-{}", now_ms())), ("if-match".into(), revisao)]).await } };
    let EngineResponse::Api { status, body, .. } = post("/conversas/c-codex/prompt".into(), json!({"texto": "responda só: ok"})).await else { panic!() };
    println!("prompt: {status} {body}");
    let turnos = viva(&state, &id).interaction.lock().unwrap().turnos_iniciados;
    let mut comecou = false;
    for _ in 0..40 { espera(500).await; if viva(&state, &id).interaction.lock().unwrap().turnos_iniciados > turnos { comecou = true; break; } }
    println!("começou o turno: {comecou}");
    let mut de_volta = false;
    for _ in 0..120 { espera(500).await; if interaction::capabilities(&viva(&state, &id)).prompt { de_volta = true; break; } }
    imprimir("tela depois da resposta", &tela(&state, &id));
    println!("pronta de novo: {de_volta}");

    // A descoberta de rollouts (a lista faz a cada 15 s) acha o id da conversa do Codex.
    let mut achou = None;
    for _ in 0..20 {
        let mut indice = omni_core::conversations::read_index(dados.path());
        omni_core::conversations::discover_codex_transcripts(&mut indice, &omni_core::profiles(dados.path()));
        omni_core::conversations::write_index(dados.path(), &indice).unwrap();
        achou = indice[0].segments.last().and_then(|s| s.external_session_id.clone());
        if achou.is_some() { break; }
        espera(1000).await;
    }
    println!("rollout achado: {achou:?}");
    let EngineResponse::Api { body, .. } = mobile::local(state.clone(), "GET", "/conversas/c-codex/opcoes", None, None, vec![]).await else { panic!() };
    let opcoes: Value = serde_json::from_str(&body).unwrap();
    println!("opções: modelo={:?} esforço={:?} modelos={:?}", opcoes["model"], opcoes["effort"],
        opcoes["models"].as_array().map(|m| m.iter().filter_map(|x| x["id"].as_str()).collect::<Vec<_>>()));
    let EngineResponse::Api { status, body, .. } = post("/conversas/c-codex/ajustes".into(), json!({"profile_id": null, "model": "gpt-5.6-luna", "effort": "low"})).await else { panic!() };
    println!("ajustes: {status} {body}");
    let inicio = std::time::Instant::now();
    let mut ultimo_motivo = String::new();
    while inicio.elapsed().as_secs() < 40 {
        espera(1000).await;
        let cap = interaction::capabilities(&viva(&state, &id));
        let motivo = format!("pronta={} menu={:?} motivo={:?}", cap.prompt, cap.escolha.as_ref().map(|m| m.pergunta.clone()), cap.reason);
        if motivo != ultimo_motivo { println!("[{:>2}s] {motivo}", inicio.elapsed().as_secs()); ultimo_motivo = motivo; }
        if cap.prompt && inicio.elapsed().as_secs() > 4 { break; }
    }
    imprimir("tela depois da troca", &tela(&state, &id));
    let EngineResponse::Api { body, .. } = mobile::local(state.clone(), "GET", "/conversas/c-codex/timeline", None, None, vec![]).await else { panic!() };
    let timeline: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
    println!("ações: {}", timeline["actions"]);
    let _ = stop_session(&state, &id);
}
