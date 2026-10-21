//! Fork: uma sessão nativa aberta não pode ser capturada pela descoberta de uma passagem nova.
use super::conversations::*;
use serde_json::json;

#[test]
fn descoberta_nao_captura_rollout_anterior_ou_ja_ligado_a_outra_conversa() {
    let dir = tempfile::tempdir().unwrap();
    let sessoes = dir.path().join("sessions");
    std::fs::create_dir_all(&sessoes).unwrap();
    let arquivo = sessoes.join("rollout-outra.jsonl");
    std::fs::write(&arquivo,json!({"type":"session_meta","payload":{"id":"alheia","cwd":"/projeto","timestamp":"1970-01-01T00:00:10Z"}}).to_string()).unwrap();
    let perfil = crate::Profile {
        id: "conta".into(),
        provider: "codex".into(),
        name: "Teste".into(),
        config_dir: dir.path().to_string_lossy().into_owned(),
        builtin: false,
        created_at_ms: 0,
        last_used_at_ms: None,
        authenticated: false,
        email: None,
    };
    let trecho = Segment {
        provider: "codex".into(),
        profile_id: Some("conta".into()),
        external_session_id: None,
        transcript_path: None,
        terminal_session_id: Some("t".into()),
        started_at_ms: 11_000,
        ended_at_ms: None,
    };
    let mut conversa = Conversation {
        id: "nova".into(),
        project_id: "p".into(),
        cwd: "/projeto".into(),
        title: "Nova".into(),
        title_source: TitleSource::Auto,
        created_at_ms: 11_000,
        segments: vec![trecho],
    };
    assert!(
        !discover_codex_transcripts(
            std::slice::from_mut(&mut conversa),
            std::slice::from_ref(&perfil)
        ),
        "rollout criado antes do novo trecho não é dele"
    );
    let mut outra = conversa.clone();
    outra.id = "outra".into();
    outra.segments[0].external_session_id = Some("alheia".into());
    outra.segments[0].transcript_path = Some(
        arquivo
            .canonicalize()
            .unwrap()
            .to_string_lossy()
            .into_owned(),
    );
    conversa.segments[0].started_at_ms = 9_000;
    let mut conversas = vec![outra, conversa];
    assert!(
        !discover_codex_transcripts(&mut conversas, std::slice::from_ref(&perfil)),
        "rollout já atribuído não é descoberto de novo"
    );
    assert!(conversas[1].segments[0].external_session_id.is_none());
    conversas[1].segments[0].external_session_id = Some("alheia".into());
    assert!(
        discover_codex_transcripts(&mut conversas, &[perfil]),
        "retomada explícita pelo id continua funcionando"
    );
}
