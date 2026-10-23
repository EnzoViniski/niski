//! Fork: rota da barra com pastas sintéticas; sem CLI, PTY ou conta real.
use super::*;

fn fixture() -> (tempfile::TempDir, Arc<EngineState>) {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: MobileRuntime::new(dir.path()),
        token:"test-only".into(),state_file:dir.path().join("sessions.json"),sessions:Mutex::new(HashMap::new()),id_sequence:AtomicU64::new(1) });
    let perfil = dir.path().join("perfil");
    std::fs::create_dir_all(perfil.join("skills/revisar")).unwrap();
    std::fs::write(perfil.join("skills/revisar/SKILL.md"), "---\nname: revisar\ndescription: Revisa o diff\n---\n").unwrap();
    atomic_write_json(&dir.path().join("conversations.json"),&json!({"version":1,"conversations":[{
        "id":"c","project_id":"p","cwd":dir.path(),"title":"Teste","created_at_ms":1,
        "segments":[{"provider":"claude","profile_id":"p","external_session_id":"ext","started_at_ms":1}]
    }]})).unwrap();
    atomic_write_json(&dir.path().join("profiles.json"),&json!({"profiles":[{"id":"p","provider":"claude","name":"Fixture","config_dir":perfil,"builtin":false,"created_at_ms":0}]})).unwrap();
    (dir,state)
}

#[test] fn lista_cli_e_skill_da_conta() {
    let (_dir,state) = fixture();
    let (provider, itens) = itens_da_conversa(&state,"c").unwrap();
    assert_eq!(provider,"claude");
    assert!(itens.iter().any(|i| i.nome == "compact" && i.tipo == "cli"));
    let skill = itens.iter().find(|i| i.nome == "revisar").unwrap();
    assert_eq!((skill.tipo, skill.descricao.as_str(), skill.insere.as_str()), ("skill","Revisa o diff","/revisar "));
}

#[tokio::test] async fn rota_recusa_comando_fora_da_lista() {
    let (_dir,state) = fixture();
    let EngineResponse::Api { status, body, .. } = local(state.clone(),"GET","/conversas/c/barra",None,None,vec![]).await else { panic!("não é Api") };
    assert_eq!(status,200);
    assert!(body.contains("\"revisar\""));
    for texto in ["/model opus", "/exit", "/compact", "olá"] {
        let corpo = json!({"texto":texto}).to_string();
        let EngineResponse::Api { status, .. } = local(state.clone(),"POST","/conversas/c/barra",Some(corpo),None,vec![]).await else { panic!("não é Api") };
        assert_eq!(status,400,"{texto}");
    }
}
