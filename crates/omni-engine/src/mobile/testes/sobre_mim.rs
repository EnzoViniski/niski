//! Fork (Sobre mim): API local, sugestões dos agentes e o texto que chega às CLIs.
use super::*;

async fn api(state: &Arc<EngineState>, metodo: &str, caminho: &str, corpo: Option<Value>) -> (u16, Value) {
    let EngineResponse::Api { status, body } = local(state.clone(), metodo, caminho, corpo.map(|c| c.to_string()), None, vec![]).await
        else { panic!("resposta inesperada") };
    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn sobre_mim_grava_areas_aceita_sugestao_e_entra_no_contexto_dos_agentes() {
    let (dir, state) = fixture();
    let (status, vazio) = api(&state, "GET", "/sobre-mim", None).await;
    assert_eq!(status, 200);
    assert_eq!(vazio["areas"].as_array().unwrap().len(), omni_core::sobre_mim::AREAS.len());
    assert_eq!(vazio["ajustes"], json!({"ativo":true,"sugestoes":true}));
    assert!(crate::memoria_compartilhada::texto(&state).is_none(), "nada escrito, nada injetado");

    // Uma área cheia com acentos passa do limite global de 32 KB: a rota tem o dela.
    let cheia = "ção\n".repeat(2_500);
    let (status, _) = api(&state, "POST", "/sobre-mim/areas/outros", Some(json!({"texto": cheia}))).await;
    assert_eq!(status, 200);
    let (status, _) = api(&state, "POST", "/sobre-mim/areas/outros", Some(json!({"texto": "x".repeat(10_001)}))).await;
    assert_eq!(status, 400);
    let (status, _) = api(&state, "POST", "/sobre-mim/areas/..%2Fsessions", Some(json!({"texto": "x"}))).await;
    assert_eq!(status, 400);

    let (status, estado) = api(&state, "POST", "/sobre-mim/areas/trabalho", Some(json!({"texto": "Sócio da Medza (sintético)."}))).await;
    assert_eq!(status, 200);
    assert_eq!(estado["areas"][2]["texto"], "Sócio da Medza (sintético).");

    // O agente anota uma sugestão; a tela aceita com edição e ela vai para a área certa.
    let propostas = omni_core::sobre_mim::pasta(dir.path()).join("propostas.md");
    let mut atual = std::fs::read_to_string(&propostas).unwrap();
    atual.push_str("- [estudos] Cursa Administração (Codex, 2026-10-21)\n- [outros] Fato a descartar\n");
    std::fs::write(&propostas, atual).unwrap();
    let (_, estado) = api(&state, "GET", "/sobre-mim", None).await;
    let lista = estado["propostas"].as_array().unwrap().clone();
    assert_eq!(lista.len(), 2);
    assert_eq!(lista[0]["area"], "estudos");
    assert_eq!(lista[0]["origem"], "Codex, 2026-10-21");
    let (status, estado) = api(&state, "POST", "/sobre-mim/propostas/aceitar",
        Some(json!({"linha": lista[0]["linha"], "area": "estudos", "texto": "Cursa Administração na faculdade Y"}))).await;
    assert_eq!(status, 200);
    assert_eq!(estado["areas"][3]["texto"], "- Cursa Administração na faculdade Y\n");
    let (status, _) = api(&state, "POST", "/sobre-mim/propostas/aceitar",
        Some(json!({"linha": lista[0]["linha"], "area": "estudos", "texto": "de novo"}))).await;
    assert_eq!(status, 409, "sugestão já tratada");
    let (status, estado) = api(&state, "POST", "/sobre-mim/propostas/descartar", Some(json!({"linha": lista[1]["linha"]}))).await;
    assert_eq!(status, 200);
    assert!(estado["propostas"].as_array().unwrap().is_empty());

    // Claude: o texto vai num arquivo do app, nunca na linha do shell. Codex: developer_instructions.
    let texto = crate::memoria_compartilhada::texto(&state).unwrap();
    assert!(texto.contains("Sócio da Medza") && texto.contains("propostas.md"));
    let origem = SessionOrigin { provider: Some("claude".into()), ..Default::default() };
    let comando = crate::memoria_compartilhada::preparar(&state, &origem, Some("claude".into())).unwrap();
    assert!(comando.contains("--append-system-prompt-file") && !comando.contains("Medza"));
    assert!(std::fs::read_to_string(dir.path().join("memoria-compartilhada.md")).unwrap().contains("Sócio da Medza"));
    let codex = SessionOrigin { provider: Some("codex".into()), env: vec![("CODEX_HOME".into(), dir.path().to_string_lossy().into_owned())], ..Default::default() };
    assert!(crate::memoria_compartilhada::argumentos_codex(&state, &codex)[1].contains("Sócio da Medza"));

    // Memória compartilhada e Sobre mim juntos; desligar o Sobre mim tira só ele.
    omni_core::memoria_compartilhada::salvar(dir.path(), &omni_core::memoria_compartilhada::Memoria { ativa: true, texto: "Preferência geral".into() }).unwrap();
    let juntos = crate::memoria_compartilhada::texto(&state).unwrap();
    assert!(juntos.contains("Preferência geral") && juntos.contains("Sócio da Medza"));
    let (status, _) = api(&state, "POST", "/sobre-mim/ajustes", Some(json!({"ativo": false, "sugestoes": true}))).await;
    assert_eq!(status, 200);
    let so_memoria = crate::memoria_compartilhada::texto(&state).unwrap();
    assert!(so_memoria.contains("Preferência geral") && !so_memoria.contains("Sócio da Medza"));
}
