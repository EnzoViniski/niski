//! Fork (3B): rotas da fila de contas pelo protocolo local, cota pela statusline gravada (fixture) e
//! a queda de conta só sem tela. Sem CLI de verdade: perfis em pasta temporária, uso sintético.
use super::*;

fn fixture() -> (tempfile::TempDir, Arc<EngineState>) {
    let dir = tempfile::tempdir().unwrap();
    let perfil = |id: &str, provider: &str, nome: &str| json!({"id": id, "provider": provider, "name": nome,
        "config_dir": dir.path().join("contas").join(id), "builtin": false, "created_at_ms": 0});
    atomic_write_json(&dir.path().join("profiles.json"), &json!({"profiles": [
        perfil("a", "claude", "Pessoal"), perfil("b", "claude", "Empresa"), perfil("x", "codex", "Codex"),
    ]})).unwrap();
    let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: MobileRuntime::new(dir.path()),
        token: "test-only".into(), state_file: dir.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1) });
    (dir, state)
}

/// Uso como a statusline do Claude grava (`uso-statusline.json`): percentual da janela de 5 h.
fn uso_pela_statusline(dir: &Path, contas: &[(&str, f64)]) {
    let agora = now_ms();
    let mapa: serde_json::Map<String, Value> = contas.iter().map(|(id, pct)| ((*id).to_owned(), json!({
        "profile_id": id, "provider": "claude", "source": "claude_statusline", "status": "available", "observed_at_ms": agora - 120_000,
        "primary": {"used_percent": pct, "window_minutes": 300, "resets_at": (agora / 1000) + 3600, "reset_label": null},
        "secondary": null, "reason": null}))).collect();
    atomic_write_json(&dir.join("uso-statusline.json"), &Value::Object(mapa)).unwrap();
}

async fn chamar(state: &Arc<EngineState>, metodo: &str, caminho: &str, corpo: Option<Value>) -> (u16, Value) {
    let EngineResponse::Api { status, body } = local(state.clone(), metodo, caminho, corpo.map(|c| c.to_string()), None, vec![]).await
        else { panic!("não é Api") };
    (status, serde_json::from_str(&body).unwrap_or(Value::Null))
}

#[tokio::test]
async fn rotas_leem_gravam_e_validam_o_routing_json() {
    let (dir, state) = fixture();
    let (status, corpo) = chamar(&state, "GET", "/roteamento", None).await;
    assert_eq!(status, 200);
    assert_eq!(corpo["politica"]["modo"], "fila");
    assert_eq!(corpo["contas"].as_array().unwrap().len(), 3);
    // Sem leitura de uso: desconhecida, com a fonte que tentou (cache do Claude ausente = sem fonte).
    assert_eq!(corpo["contas"][0]["cota"]["conhecida"], false);
    assert_eq!(corpo["contas"][2]["cota"]["fonte"], "codex_rollout");
    // "Automática" funciona sem fila configurada: vale a ordem do cadastro.
    assert_eq!(corpo["escolhas"]["claude"]["perfil_id"], "a");
    assert!(corpo["escolhas"]["claude"]["explicacao"][0].as_str().unwrap().contains("a fila não tem contas de claude"));

    let politica = json!({"modo": "auto", "etapas": ["b", "a"], "contas": {"b": {"tipo": "api", "orcamento_usd": 20.0, "gasto_usd": 5.0,
        "max_concorrencia": 2, "confidencial_ok": true}}, "projetos_confidenciais": ["/empresa"], "usar_em_fluxos": true});
    let (status, corpo) = chamar(&state, "POST", "/roteamento", Some(politica)).await;
    assert_eq!(status, 200, "{corpo}");
    assert_eq!(corpo["politica"]["etapas"], json!(["b", "a"]));
    assert_eq!(corpo["contas"][1]["ajustes"]["tipo"], "api");
    assert_eq!(corpo["contas"][1]["na_fila"], true);
    let gravado = Politica::carregar(&std::fs::read_to_string(dir.path().join(ARQUIVO)).unwrap()).unwrap();
    assert_eq!((gravado.modo, gravado.usar_em_fluxos), (regra::Modo::Auto, true));

    for invalida in [json!({"modo": "aleatorio"}), json!({"contas": {"a": {"orcamento_usd": -1}}}), json!({"etapas": [" "]}),
        json!({"contas": {"a": {"max_concorrencia": 0}}})] {
        let (status, corpo) = chamar(&state, "POST", "/roteamento", Some(invalida.clone())).await;
        assert_eq!(status, 400, "{invalida} → {corpo}");
    }
    // O inválido não sobrescreveu o bom.
    assert_eq!(ler_politica(dir.path()).etapas, vec!["b".to_owned(), "a".to_owned()]);
}

#[tokio::test]
async fn escolher_explica_cota_fonte_idade_e_confidencial() {
    let (dir, state) = fixture();
    uso_pela_statusline(dir.path(), &[("a", 30.0), ("b", 100.0)]);
    atomic_write_json(&dir.path().join(ARQUIVO), &json!({"modo": "fila", "etapas": ["b", "a"],
        "contas": {"a": {"confidencial_ok": true}}, "projetos_confidenciais": ["/empresa"]})).unwrap();

    let (status, corpo) = chamar(&state, "POST", "/roteamento/escolher", Some(json!({"provider": "claude", "projeto": "/livre"}))).await;
    assert_eq!(status, 200, "{corpo}");
    assert_eq!((corpo["perfil_id"].as_str(), corpo["nome"].as_str()), (Some("a"), Some("Pessoal")));
    let linhas: Vec<&str> = corpo["explicacao"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert!(linhas.iter().any(|l| l.contains("Empresa (b): pulada — limite estourado (5h)")), "{linhas:?}");
    assert!(linhas.iter().any(|l| l.contains("5h 30%") && l.contains("fonte claude_statusline, há 2 min")), "{linhas:?}");

    // Projeto dentro da pasta confidencial: só a conta liberada; sem nenhuma, ninguém.
    atomic_write_json(&dir.path().join(ARQUIVO), &json!({"etapas": ["b", "a"], "projetos_confidenciais": ["/empresa"]})).unwrap();
    let (_, corpo) = chamar(&state, "POST", "/roteamento/escolher", Some(json!({"provider": "claude", "projeto": "/empresa/app"}))).await;
    assert_eq!(corpo["perfil_id"], Value::Null, "{corpo}");
    assert!(corpo["explicacao"][0].as_str().unwrap().contains("projeto confidencial"));

    let (status, _) = chamar(&state, "POST", "/roteamento/escolher", Some(json!({"provider": " "}))).await;
    assert_eq!(status, 400);
}

#[tokio::test]
async fn apos_falha_so_troca_sem_tela_e_avisa_no_interativo() {
    let (_dir, state) = fixture();
    let (_, corpo) = chamar(&state, "POST", "/roteamento/apos-falha", Some(json!({"provider": "claude", "falharam": ["a"], "headless": true}))).await;
    assert_eq!(corpo["perfil_id"], "b", "{corpo}");
    let (_, corpo) = chamar(&state, "POST", "/roteamento/apos-falha", Some(json!({"provider": "claude", "falharam": ["a"]}))).await;
    assert_eq!((corpo["perfil_id"].clone(), corpo["sugestao"].clone(), corpo["nome_da_sugestao"].clone()), (Value::Null, json!("b"), json!("Empresa")));
    assert!(corpo["explicacao"][0].as_str().unwrap().contains("sessão interativa"), "{corpo}");
    let (status, _) = chamar(&state, "POST", "/roteamento/apos-falha", Some(json!({"provider": "claude", "falharam": []}))).await;
    assert_eq!(status, 400);
}

#[test]
fn queda_sem_tela_tenta_a_proxima_so_em_falha_de_conta() {
    let (dir, state) = fixture();
    atomic_write_json(&dir.path().join(ARQUIVO), &json!({"etapas": ["a", "b"]})).unwrap();
    let mut tentadas = Vec::new();
    let resultado = com_queda_sem_tela(&state, "claude", "auto", "/p", None, false, |perfil| {
        let id = perfil.map(|p| p.id).unwrap_or_default();
        tentadas.push(id.clone());
        if id == "a" { Err(anyhow!("Claude AI usage limit reached")) } else { Ok(format!("feito em {id}")) }
    });
    assert_eq!(resultado.unwrap(), "feito em b");
    assert_eq!(tentadas, ["a", "b"]);

    // Erro da tarefa não troca de conta.
    let mut tentadas = 0;
    let resultado = com_queda_sem_tela(&state, "claude", "auto", "/p", None, false, |_| { tentadas += 1; Err(anyhow!("Prompt is too long")) });
    assert!(resultado.is_err());
    assert_eq!(tentadas, 1);

    // Todas no limite: para quando a fila acaba, com a explicação.
    let erro = com_queda_sem_tela(&state, "claude", "auto", "/p", None, false, |_| Err(anyhow!("429 rate limit"))).unwrap_err().to_string();
    assert!(erro.contains("Fila de contas sem conta para claude") && erro.contains("falhou nesta execução"), "{erro}");

    // Conta fixa no modelo de agente (ou vazia sem "usar nos fluxos"): a escolha de sempre, sem fila.
    let mut recebido = Vec::new();
    let _ = com_queda_sem_tela(&state, "claude", "b", "/p", None, false, |perfil| { recebido.push(perfil.is_none()); Ok(String::new()) });
    let _ = com_queda_sem_tela(&state, "claude", "", "/p", None, false, |perfil| { recebido.push(perfil.is_none()); Ok(String::new()) });
    assert_eq!(recebido, [true, true]);
    // Com "usar nos fluxos", agente sem conta também vai pela fila.
    atomic_write_json(&dir.path().join(ARQUIVO), &json!({"etapas": ["b"], "usar_em_fluxos": true})).unwrap();
    let usada = com_queda_sem_tela(&state, "claude", "", "/p", None, false, |perfil| Ok(perfil.map(|p| p.id).unwrap_or_default())).unwrap();
    assert_eq!(usada, "b");

    // Retomar fica na conta em que a sessão nasceu, mesmo que a fila prefira outra.
    let usada = com_queda_sem_tela(&state, "claude", "auto", "/p", Some("a".into()), true, |perfil| Ok(perfil.map(|p| p.id).unwrap_or_default())).unwrap();
    assert_eq!(usada, "a");
}

/// O teto de sessões simultâneas por conta conta só agente vivo daquela conta.
#[cfg(unix)]
#[test]
fn concorrencia_conta_sessoes_vivas_da_conta() {
    let (dir, state) = fixture();
    atomic_write_json(&dir.path().join(ARQUIVO), &json!({"etapas": ["a", "b"], "contas": {"a": {"max_concorrencia": 1}}})).unwrap();
    let cwd = dir.path().to_string_lossy().into_owned();
    spawn_terminal(&state, "p".into(), "t".into(), cwd, Some("/bin/sh".into()), Some("sleep 30".into()), 24, 80,
        SessionOrigin { headless: true, provider: Some("claude".into()), profile_id: Some("a".into()), ..SessionOrigin::default() }).unwrap();
    assert_eq!(sessoes_por_perfil(&state).get("a"), Some(&1));
    let escolha = escolher_para(&state, "claude", None, false, false, &[]);
    assert_eq!(escolha.perfil_id.as_deref(), Some("b"));
    assert!(escolha.explicacao.iter().any(|l| l.contains("concorrência no máximo (1 de 1)")), "{:?}", escolha.explicacao);
    let ids: Vec<String> = state.sessions.lock().unwrap().keys().cloned().collect();
    for id in ids { let _ = close_session(&state, &id); }
}

/// Fork (fix-orquestracao, achado 2): pasta confidencial vale também nas worktrees do mesmo
/// repositório — a isolada do Medza (`.medza-worktrees/<repo>/<nome>`) e uma `git worktree` comum —, e
/// numa subpasta delas. Antes o orquestrador numa worktree escapava da regra e "auto" usava qualquer conta.
#[cfg(unix)]
#[test]
fn confidencial_vale_nas_worktrees_do_projeto_original() {
    let (dir, state) = fixture();
    let base = dir.path().canonicalize().unwrap();
    let repo = base.join("empresa");
    std::fs::create_dir_all(repo.join("src")).unwrap();
    let git = |pasta: &Path, args: &[&str]| assert!(std::process::Command::new("git").args(args).current_dir(pasta)
        .output().unwrap().status.success(), "{args:?}");
    git(&repo, &["init", "-q", "-b", "main"]);
    std::fs::write(repo.join("src/a.txt"), "a").unwrap();
    git(&repo, &["add", "."]);
    git(&repo, &["-c", "user.email=t@t", "-c", "user.name=t", "commit", "-q", "-m", "inicio"]);
    let isolada = omni_core::worktree_isolada::criar(&repo, "tarefa").unwrap();
    let comum = base.join("empresa-wt");
    git(&repo, &["worktree", "add", "-q", "-b", "outra", &comum.to_string_lossy(), "HEAD"]);
    atomic_write_json(&dir.path().join(ARQUIVO), &json!({"etapas": ["a", "b"], "contas": {"b": {"confidencial_ok": true}},
        "projetos_confidenciais": [repo.to_string_lossy()]})).unwrap();

    let conta = |projeto: &Path| escolher_para(&state, "claude", Some(&projeto.to_string_lossy()), false, false, &[]).perfil_id;
    assert_eq!(conta(&base.join("livre")).as_deref(), Some("a"), "fora do projeto vale a fila");
    assert_eq!(conta(&repo).as_deref(), Some("b"));
    assert_eq!(conta(Path::new(&isolada.caminho)).as_deref(), Some("b"), "worktree isolada do Medza");
    assert_eq!(conta(&Path::new(&isolada.caminho).join("src")).as_deref(), Some("b"), "subpasta da worktree isolada");
    assert_eq!(conta(&comum).as_deref(), Some("b"), "git worktree comum");
    // Worktree isolada já apagada (sem git para perguntar): o caminho `.medza-worktrees` ainda diz de onde veio.
    assert_eq!(conta(&base.join(".medza-worktrees/empresa/sumiu")).as_deref(), Some("b"));
    // Só a subpasta é confidencial: a mesma subpasta na worktree também é; outra pasta dela não.
    atomic_write_json(&dir.path().join(ARQUIVO), &json!({"etapas": ["a", "b"], "contas": {"b": {"confidencial_ok": true}},
        "projetos_confidenciais": [repo.join("src").to_string_lossy()]})).unwrap();
    assert_eq!(conta(&Path::new(&isolada.caminho).join("src")).as_deref(), Some("b"));
    assert_eq!(conta(Path::new(&isolada.caminho)).as_deref(), Some("a"));
}

/// Fork (fix-orquestracao, achado 9): execução sem tela dos fluxos ocupa vaga da conta enquanto roda.
/// Duas execuções juntas numa fila com teto 1 na primeira conta não caem as duas nela.
#[test]
fn execucao_sem_tela_conta_no_teto_de_concorrencia() {
    let (dir, state) = fixture();
    atomic_write_json(&dir.path().join(ARQUIVO), &json!({"etapas": ["a", "b"], "contas": {"a": {"max_concorrencia": 1}}})).unwrap();
    // Enquanto roda, a vaga da conta está ocupada para qualquer outra escolha.
    let usada = com_queda_sem_tela(&state, "claude", "auto", "/p", None, false, |perfil| {
        assert_eq!(sessoes_por_perfil(&state).get("a"), Some(&1));
        assert_eq!(escolher_para(&state, "claude", None, false, false, &[]).perfil_id.as_deref(), Some("b"));
        Ok(perfil.map(|p| p.id).unwrap_or_default())
    }).unwrap();
    assert_eq!(usada, "a");
    assert_eq!(sessoes_por_perfil(&state).get("a"), None, "terminou: a vaga volta");

    // Duas execuções ao mesmo tempo: escolha e reserva na mesma seção crítica.
    let barreira = std::sync::Barrier::new(2);
    let usadas: Vec<String> = std::thread::scope(|s| {
        let rodar = || s.spawn(|| { barreira.wait(); com_queda_sem_tela(&state, "claude", "auto", "/p", None, false, |perfil| {
            std::thread::sleep(std::time::Duration::from_millis(300));
            Ok(perfil.map(|p| p.id).unwrap_or_default())
        }).unwrap() });
        let (um, outro) = (rodar(), rodar());
        vec![um.join().unwrap(), outro.join().unwrap()]
    });
    let mut usadas = usadas;
    usadas.sort();
    assert_eq!(usadas, ["a", "b"]);

    // Conta fixa do modelo de agente (fora da fila) também ocupa a vaga enquanto roda.
    {
        let _vaga = ocupar_sem_tela(&state, "a");
        assert_eq!(sessoes_por_perfil(&state).get("a"), Some(&1));
        assert_eq!(escolher_para(&state, "claude", None, false, false, &[]).perfil_id.as_deref(), Some("b"));
    }
    assert_eq!(sessoes_por_perfil(&state).get("a"), None);
}
