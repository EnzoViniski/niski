//! CLI de revisão falsa; nenhuma CLI instalada é chamada.
use super::*;
use axum::body::{to_bytes, Body};
use tower::ServiceExt;

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
    let perfil = dir.path().join("perfil");
    fs::create_dir_all(perfil.join("sessions")).unwrap();
    atomic_write_json(&dir.path().join("profiles.json"),&json!({"profiles":[{"id":"fake","provider":"codex","name":"Fixture","config_dir":perfil,"builtin":false,"created_at_ms":0}]})).unwrap();
    atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[{"id":"mae","project_id":"p","cwd":dir.path(),"title":"Origem","created_at_ms":1,"segments":[{"provider":"codex","profile_id":"fake","started_at_ms":1}]}]})).unwrap();
    (dir, state)
}
fn req(corpo: Value, chave: Option<&str>) -> axum::http::Request<Body> {
    let mut r = axum::http::Request::builder()
        .method("POST")
        .uri("/conversas/mae/revisao")
        .header("content-type", "application/json");
    if let Some(k) = chave {
        r = r.header("idempotency-key", k);
    }
    r.body(Body::from(corpo.to_string())).unwrap()
}
#[tokio::test]
async fn rota_recusa_cli_caminhos_base_e_falta_de_chave() {
    let (_dir, state) = fixture();
    let app = rotas().with_state(state);
    for (corpo, chave, status) in [
        (
            json!({"provider":"codex","escopo":"local"}),
            None,
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"provider":"bash","escopo":"local"}),
            Some("k"),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"provider":"codex","escopo":"branch","base":"main;exit"}),
            Some("k"),
            StatusCode::BAD_REQUEST,
        ),
        (
            json!({"provider":"codex","escopo":"local","cwd":"/outra"}),
            Some("k"),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
    ] {
        assert_eq!(
            app.clone()
                .oneshot(req(corpo, chave))
                .await
                .unwrap()
                .status(),
            status
        );
    }
}

#[cfg(unix)]
#[tokio::test]
async fn codex_falso_roda_review_em_conversa_separada_vincula_e_nao_duplica() {
    let (dir, state) = fixture();
    let perfil = dir.path().join("perfil");
    let script = dir.path().join("codex-falso.sh");
    fs::write(
        &script,
        r#"#!/bin/sh
printf '%s\n' "$@" > "$(dirname "$0")/args"
pwd > "$(dirname "$0")/pasta"
cp "$(dirname "$0")/fixture.jsonl" "$CODEX_HOME/sessions/rollout-falso.jsonl"
printf 'Revisão falsa concluída\n'
"#,
    )
    .unwrap();
    // Transcript nativo sintético que a mesma CLI falsa publica na conta sintética.
    let data = std::process::Command::new("date")
        .args(["-u", "+%Y-%m-%dT%H:%M:%SZ"])
        .output()
        .unwrap();
    let timestamp = String::from_utf8_lossy(&data.stdout).trim().to_owned();
    let linhas = [
        json!({"type":"session_meta","payload":{"id":"review-falso","cwd":dir.path(),"timestamp":timestamp}}),
        json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Achado sintético de revisão"}]}}),
    ];
    fs::write(
        dir.path().join("fixture.jsonl"),
        linhas.iter().map(|v| format!("{v}\n")).collect::<String>(),
    )
    .unwrap();
    publish(
        &state,
        vec![PublishedProject {
            id: "p".into(),
            name: "Projeto".into(),
            path: dir.path().to_string_lossy().into(),
        }],
        vec![PublishedAgent {
            id: "codex".into(),
            label: "Codex".into(),
            command: format!("/bin/sh '{}'", script.display()),
            resume: None,
        }],
        None,
    )
    .unwrap();
    let app = rotas().with_state(state.clone());
    let pedido = json!({"provider":"codex","escopo":"local"});
    let res = app
        .clone()
        .oneshot(req(pedido.clone(), Some("mesmo-clique")))
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::CREATED);
    let valor: Value =
        serde_json::from_slice(&to_bytes(res.into_body(), usize::MAX).await.unwrap()).unwrap();
    let id = valor["id"].as_str().unwrap();
    assert_ne!(id, "mae");
    let repetida = app
        .clone()
        .oneshot(req(pedido, Some("mesmo-clique")))
        .await
        .unwrap();
    let repetida: Value =
        serde_json::from_slice(&to_bytes(repetida.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(valor, repetida);
    assert_eq!(
        app.oneshot(req(
            json!({"provider":"claude","escopo":"local"}),
            Some("mesmo-clique")
        ))
        .await
        .unwrap()
        .status(),
        StatusCode::CONFLICT
    );
    // Fork: com a suíte inteira em paralelo a CLI falsa demora mais que 2 s; espera os dois arquivos.
    for _ in 0..500 {
        if perfil.join("sessions/rollout-falso.jsonl").exists() && dir.path().join("pasta").exists() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert_eq!(
        fs::read_to_string(dir.path().join("args")).unwrap(),
        "review\n--uncommitted\n"
    );
    assert_eq!(
        Path::new(fs::read_to_string(dir.path().join("pasta")).unwrap().trim())
            .canonicalize()
            .unwrap(),
        dir.path().canonicalize().unwrap()
    );
    let registro = regra::ler(dir.path());
    assert!(registro.agente_de("mae", id).is_some());
    assert!(!registro.ativo("mae"));
    let mut indice = omni_core::conversations::read_index(dir.path());
    assert_eq!(indice.len(), 2);
    assert_eq!(indice[0].title, "Origem");
    assert!(omni_core::conversations::discover_codex_transcripts(
        &mut indice,
        &omni_core::profiles(dir.path())
    ));
    let filha = indice.iter().find(|c| c.id == id).unwrap();
    assert_eq!(filha.cwd, dir.path().to_string_lossy());
    assert_eq!(
        filha.segments[0].external_session_id.as_deref(),
        Some("review-falso")
    );
    let _ = close_session(&state, valor["session_id"].as_str().unwrap());
}
