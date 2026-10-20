use super::*;
use omni_core::agendamentos::Regra;

#[path = "agendamentos_sessao_testes.rs"]
mod sessao;

fn agenda(regra: Regra) -> Agendamento {
    Agendamento {
        id: "teste".into(),
        pedido: Pedido {
            id: None,
            titulo: "Rotina".into(),
            prompt: "Prompt sintético".into(),
            regra,
            destino: Destino::Conversa {
                conversa: "conv-teste".into(),
            },
        },
        pausado: false,
        proxima_ms: Some(1_000),
        fuso: "local_do_engine".into(),
        historico: vec![],
        pendente: None,
        tentar_em_ms: 0,
        rodar_agora: false,
    }
}
struct Falso {
    chamadas: usize,
}
impl Executor for Falso {
    fn passo(&mut self, a: &Agendamento) -> Passo {
        self.chamadas += 1;
        // Confere que a ocorrência já está persistida antes de acionar a CLI falsa.
        match a.pendente.as_ref().unwrap().fase.as_str() {
            "preparar" => Passo::Preparada("conv-falsa".into()),
            "entregar" => Passo::Enfileirada("acao-falsa".into()),
            _ => Passo::Terminou,
        }
    }
}
#[test]
fn sono_recupera_uma_vez_persiste_e_nao_empilha() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = Registro {
        itens: vec![agenda(Regra::Intervalo { minutos: 5 })],
    };
    let mut cli = Falso { chamadas: 0 };
    for agora in [100_000_000, 100_030_000, 100_060_000, 100_090_000] {
        tique(dir.path(), &mut r, agora, &mut cli).unwrap();
    }
    assert_eq!(cli.chamadas, 3);
    assert_eq!(r.itens[0].historico.len(), 1);
    assert_eq!(r.itens[0].historico[0].estado, "enviado");
    let lido = ler(dir.path()).unwrap();
    assert_eq!(lido.itens[0].proxima_ms, Some(100_300_000));
    let json = serde_json::to_value(&lido.itens[0].historico)
        .unwrap()
        .to_string();
    assert!(!json.contains("Prompt"));
}
#[test]
fn pausado_e_concluido_nao_rodam_de_novo() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = agenda(Regra::UmaVez { em_ms: 1_000 });
    a.pausado = true;
    let mut r = Registro { itens: vec![a] };
    let mut cli = Falso { chamadas: 0 };
    tique(dir.path(), &mut r, 2_000, &mut cli).unwrap();
    assert_eq!(cli.chamadas, 0);
    r.itens[0].pausado = false;
    for agora in [3_000, 4_000, 5_000, 6_000] {
        tique(dir.path(), &mut r, agora, &mut cli).unwrap();
    }
    assert_eq!(cli.chamadas, 3);
    assert_eq!(r.itens[0].proxima_ms, None);
}
#[test]
fn ocupado_adia_e_nao_cria_outra_ocorrencia() {
    struct Ocupado(usize);
    impl Executor for Ocupado {
        fn passo(&mut self, _: &Agendamento) -> Passo {
            self.0 += 1;
            Passo::Aguardar
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let mut r = Registro {
        itens: vec![agenda(Regra::Intervalo { minutos: 5 })],
    };
    let mut cli = Ocupado(0);
    for agora in [2_000, 32_000, 62_000, 362_000] {
        tique(dir.path(), &mut r, agora, &mut cli).unwrap();
    }
    assert_eq!(cli.0, 3);
    assert_eq!(r.itens[0].historico.len(), 1);
    assert_eq!(r.itens[0].historico[0].estado, "adiado");
}
#[test]
fn arquivo_corrompido_nao_vira_lista_vazia() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join(ARQUIVO), b"invalid").unwrap();
    assert!(ler(dir.path()).is_err());
}

fn fixture() -> (tempfile::TempDir, Arc<EngineState>) {
    let dir = tempfile::tempdir().unwrap();
    let state = Arc::new(EngineState {
        persist_lock: Mutex::new(()),
        mobile: MobileRuntime::new(dir.path()),
        token: "teste".into(),
        state_file: dir.path().join("sessions.json"),
        sessions: Mutex::new(HashMap::new()),
        id_sequence: AtomicU64::new(1),
    });
    atomic_write_json(&dir.path().join("conversations.json"), &json!({"version":1,"conversations":[{
        "id":"conv-teste","project_id":"p","cwd":dir.path().to_string_lossy(),"title":"Sintética","created_at_ms":1,"segments":[]
    }]})).unwrap();
    (dir, state)
}
async fn api(
    state: Arc<EngineState>,
    metodo: &str,
    caminho: &str,
    body: Value,
) -> (StatusCode, Value) {
    use axum::body::{to_bytes, Body};
    use tower::ServiceExt;
    let req = axum::http::Request::builder()
        .method(metodo)
        .uri(caminho)
        .header("content-type", "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let res = rotas().with_state(state).oneshot(req).await.unwrap();
    let status = res.status();
    let bytes = to_bytes(res.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}
#[tokio::test]
async fn rotas_criam_editam_pausam_retomam_rodam_e_apagam() {
    let (_dir, state) = fixture();
    let mut pedido = serde_json::to_value(&agenda(Regra::Intervalo { minutos: 5 }).pedido).unwrap();
    let (status, salvo) = api(state.clone(), "POST", "/agendamentos", pedido.clone()).await;
    assert_eq!(status, StatusCode::OK, "{salvo}");
    let id = salvo["id"].as_str().unwrap();
    let caminho = format!("/agendamentos/{id}");
    for acao in ["pausar", "retomar", "rodar"] {
        assert_eq!(
            api(
                state.clone(),
                "POST",
                &format!("{caminho}/{acao}"),
                json!({})
            )
            .await
            .0,
            StatusCode::OK
        );
    }
    pedido["id"] = json!(id);
    pedido["titulo"] = json!("Editado");
    assert_eq!(
        api(state.clone(), "POST", "/agendamentos", pedido).await.0,
        StatusCode::OK
    );
    let lista = api(state.clone(), "GET", "/agendamentos", Value::Null)
        .await
        .1;
    assert_eq!(lista[0]["titulo"], "Editado");
    assert_eq!(lista[0]["id"], id);
    assert!(lista[0]["proxima_ms"].as_i64().unwrap() > now_ms() as i64);
    assert_eq!(
        api(
            state.clone(),
            "POST",
            &format!("{caminho}/apagar"),
            json!({})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        api(state.clone(), "GET", "/agendamentos", Value::Null)
            .await
            .1,
        json!([])
    );
    assert_eq!(
        api(state, "POST", &format!("{caminho}/rodar"), json!({}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}
#[tokio::test]
async fn rotas_rejeitam_injecao_e_data_passada() {
    let (_dir, state) = fixture();
    for regra in [
        json!({"tipo":"intervalo", "minutos":4}),
        json!({"tipo":"uma_vez", "em_ms":1}),
    ] {
        let mut p = serde_json::to_value(&agenda(Regra::Intervalo { minutos: 5 }).pedido).unwrap();
        p["regra"] = regra;
        assert_eq!(
            api(state.clone(), "POST", "/agendamentos", p).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    let mut p = serde_json::to_value(&agenda(Regra::Intervalo { minutos: 5 }).pedido).unwrap();
    p["prompt"] = json!("/model proibido");
    assert_eq!(
        api(state.clone(), "POST", "/agendamentos", p).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        api(
            state.clone(),
            "POST",
            "/agendamentos/data",
            json!({"local":"2030-02-30T09:00"})
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    let (status, data) = api(
        state,
        "POST",
        "/agendamentos/data",
        json!({"local":"2030-10-20T09:00"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(data["em_ms"].as_i64().unwrap() > 0);
}
#[test]
fn atividade_tem_tipo_e_metadados_sem_prompt() {
    let (dir, state) = fixture();
    let mut a = agenda(Regra::Intervalo { minutos: 5 });
    a.historico.push(Execucao {
        em_ms: 1_000,
        estado: "erro".into(),
        conversa: None,
    });
    gravar(dir.path(), &Registro { itens: vec![a] }).unwrap();
    let itens = atividade(&state);
    assert_eq!(itens[0]["tipo"], "agendamento");
    assert!(!itens[0].to_string().contains("sintético"));
}

/// CLI falsa de processo (sem PTY/ps): o relógio falso do executor entrega o prompt via stdin.
/// O caminho real da fila/PTY é conferido separadamente abaixo usando execute_checked.
#[cfg(unix)]
#[test]
fn executor_com_relogio_falso_e_cli_falsa() {
    struct Cli(std::path::PathBuf);
    impl Executor for Cli {
        fn passo(&mut self, a: &Agendamento) -> Passo {
            use std::io::Write;
            use std::process::{Command, Stdio};
            let mut filho = Command::new("/bin/sh")
                .arg("-c")
                .arg("cat > \"$1\"")
                .arg("cli-falsa")
                .arg(&self.0)
                .stdin(Stdio::piped())
                .spawn()
                .unwrap();
            filho
                .stdin
                .take()
                .unwrap()
                .write_all(a.pedido.prompt.as_bytes())
                .unwrap();
            assert!(filho.wait().unwrap().success());
            Passo::Terminou
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let mut r = Registro {
        itens: vec![agenda(Regra::UmaVez { em_ms: 1_000 })],
    };
    let mut cli = Cli(dir.path().join("recebido.txt"));
    tique(dir.path(), &mut r, 999, &mut cli).unwrap();
    assert!(!cli.0.exists());
    tique(dir.path(), &mut r, 1_000, &mut cli).unwrap();
    assert_eq!(std::fs::read_to_string(&cli.0).unwrap(), "Prompt sintético");
    assert_eq!(r.itens[0].historico[0].estado, "enviado");
    std::fs::remove_file(&cli.0).unwrap();
    tique(dir.path(), &mut r, 50_000, &mut cli).unwrap();
    assert!(!cli.0.exists());
}

#[test]
fn limite_e_conta_sao_conferidos_antes_de_abrir_agente() {
    let (dir, state) = fixture();
    publish(
        &state,
        vec![PublishedProject {
            id: "p".into(),
            name: "Teste".into(),
            path: dir.path().to_string_lossy().into_owned(),
        }],
        vec![PublishedAgent {
            id: "claude".into(),
            label: "CLI falsa".into(),
            command: "/bin/sh cli-inexistente".into(),
            resume: None,
        }],
        None,
    )
    .unwrap();
    let mut a = agenda(Regra::Intervalo { minutos: 5 });
    a.pedido.destino = Destino::NovoAgente {
        projeto: "p".into(),
        plataforma: "claude".into(),
        conta: Some("nao-existe".into()),
        modelo: None,
        pasta: dir.path().to_string_lossy().into_owned(),
        worktree: false,
    };
    assert_eq!(preparar(&state, &a).unwrap_err().0, StatusCode::CONFLICT);
    assert!(state.sessions.lock().unwrap().is_empty());
}

#[cfg(unix)]
#[test]
fn novo_agente_usa_cli_falsa_conta_modelo_e_pasta_escolhidos() {
    let (dir, state) = fixture();
    let pasta = dir.path().join("projeto");
    std::fs::create_dir_all(&pasta).unwrap();
    let perfil = dir.path().join("perfil-falso");
    std::fs::create_dir_all(&perfil).unwrap();
    atomic_write_json(&dir.path().join("profiles.json"), &json!({"profiles":[{"id":"falsa","provider":"codex","name":"Sintética","config_dir":perfil.to_string_lossy(),"builtin":false,"created_at_ms":0}]})).unwrap();
    let script = dir.path().join("cli-falsa.sh");
    std::fs::write(&script, "#!/bin/sh\nprintf '%s\\n' \"$@\" > argumentos.txt\nprintf '%s' \"$CODEX_HOME\" > conta.txt\npwd > pasta.txt\nprintf '\\033[?2004hOpenAI Codex'\nwhile :; do printf '\\r\\n❯ '; IFS= read -r linha || exit 0; [ -z \"$linha\" ] && continue; printf '%s\\n' \"$linha\" >> recebido.txt; done\n").unwrap();
    publish(
        &state,
        vec![PublishedProject {
            id: "p".into(),
            name: "Teste".into(),
            path: pasta.to_string_lossy().into_owned(),
        }],
        vec![PublishedAgent {
            id: "codex".into(),
            label: "CLI falsa".into(),
            command: format!("/bin/sh {}", script.display()),
            resume: None,
        }],
        None,
    )
    .unwrap();
    let mut a = agenda(Regra::UmaVez { em_ms: 1_000 });
    a.pedido.destino = Destino::NovoAgente {
        projeto: "p".into(),
        plataforma: "codex".into(),
        conta: Some("falsa".into()),
        modelo: Some("modelo-sintetico".into()),
        pasta: pasta.to_string_lossy().into_owned(),
        worktree: false,
    };
    let id = preparar(&state, &a).unwrap();
    let (_, Some(sessao)) = current(&state, &id).unwrap() else {
        panic!("sessão falsa ausente")
    };
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !pasta.join("pasta.txt").exists() && std::time::Instant::now() < limite {
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let sid = sessao.meta.lock().unwrap().id.clone();
    // A partir daqui nenhum `ps`: detecção do processo é injetada como nos testes da fila.
    let teste =
        || {
            assert_eq!(
                std::fs::read_to_string(pasta.join("argumentos.txt")).unwrap(),
                "--model\nmodelo-sintetico\n"
            );
            assert_eq!(
                std::fs::read_to_string(pasta.join("conta.txt")).unwrap(),
                perfil.to_string_lossy()
            );
            assert_eq!(
                std::fs::read_to_string(pasta.join("pasta.txt"))
                    .unwrap()
                    .trim(),
                pasta.canonicalize().unwrap().to_string_lossy()
            );
            omni_core::limite_de_sessoes::gravar(dir.path(), 4).unwrap();
            for n in 0..3 {
                state
                    .sessions
                    .lock()
                    .unwrap()
                    .insert(format!("vaga-{n}"), SessionEntry::Live(sessao.clone()));
            }
            assert_eq!(
                preparar(&state, &a).unwrap_err().0,
                StatusCode::TOO_MANY_REQUESTS
            );
            for n in 0..3 {
                state.sessions.lock().unwrap().remove(&format!("vaga-{n}"));
            }
            let mut action = acao_interna(&state, &id, ActionKind::Fila(a.pedido.prompt.clone()));
            action.session_id = sid.clone();
            // A CLI falsa desenhou o campo; aguarda o leitor da PTY aplicar a saída.
            while !interaction::ready(sessao.interaction.lock().unwrap().parser.screen(), "codex")
                && std::time::Instant::now() < limite
            {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            assert!(
                interaction::ready(sessao.interaction.lock().unwrap().parser.screen(), "codex"),
                "CLI falsa não pronta: {}",
                sessao
                    .interaction
                    .lock()
                    .unwrap()
                    .parser
                    .screen()
                    .contents()
            );
            sessao.interaction.lock().unwrap().parser.process(
                "\x1b[2J\x1b[HOpenAI Codex\r\nPensando (esc to interrupt)\r\n❯ ".as_bytes(),
            );
            assert!(interaction::pode_enfileirar(
                sessao.interaction.lock().unwrap().parser.screen(),
                "codex"
            ));
            execute_checked(&state, &action, |_, _| true).unwrap();
            let limite = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while !pasta.join("recebido.txt").exists() && std::time::Instant::now() < limite {
                std::thread::sleep(std::time::Duration::from_millis(25));
            }
            assert!(std::fs::read_to_string(pasta.join("recebido.txt"))
                .unwrap()
                .contains("Prompt sintético"));
            // Campo com rascunho: o mesmo caminho de execução recusa antes de escrever.
            sessao
                .interaction
                .lock()
                .unwrap()
                .parser
                .process("\x1b[2J\x1b[HOpenAI Codex\r\n❯ rascunho".as_bytes());
            assert!(execute_checked(&state, &action, |_, _| true).is_err());
        };
    let resultado = std::panic::catch_unwind(std::panic::AssertUnwindSafe(teste));
    let _ = close_session(&state, &sid);
    if let Err(panico) = resultado {
        std::panic::resume_unwind(panico);
    }
}

#[test]
fn reinicio_nao_reenvia_resultado_incerto_e_preserva_historico() {
    let dir = tempfile::tempdir().unwrap();
    let mut r = Registro {
        itens: vec![agenda(Regra::UmaVez { em_ms: 1_000 })],
    };
    let mut cli = Falso { chamadas: 0 };
    tique(dir.path(), &mut r, 1_000, &mut cli).unwrap(); // Preparada em disco, próxima consumida.
    recuperar(dir.path()).unwrap();
    let mut r = ler(dir.path()).unwrap();
    assert_eq!(r.itens[0].historico[0].estado, "interrompido");
    assert!(r.itens[0].pendente.is_none());
    tique(dir.path(), &mut r, 100_000, &mut cli).unwrap();
    assert_eq!(cli.chamadas, 1);
}
#[test]
fn rodar_agora_preserva_pausa_e_proxima_e_historico_tem_teto() {
    let dir = tempfile::tempdir().unwrap();
    let mut a = agenda(Regra::Intervalo { minutos: 5 });
    a.proxima_ms = Some(1_000_000);
    a.pausado = true;
    a.rodar_agora = true;
    let mut r = Registro { itens: vec![a] };
    let mut cli = Falso { chamadas: 0 };
    for agora in [1_000, 2_000, 3_000] {
        tique(dir.path(), &mut r, agora, &mut cli).unwrap();
    }
    assert!(r.itens[0].pausado);
    assert_eq!(r.itens[0].proxima_ms, Some(1_000_000));
    for n in 1..25 {
        r.itens[0].rodar_agora = true;
        for j in 0..3 {
            tique(dir.path(), &mut r, n * 10_000 + j, &mut cli).unwrap();
        }
    }
    assert_eq!(r.itens[0].historico.len(), MAX_HISTORICO);
}
#[test]
fn persistencia_da_ocorrencia_precede_qualquer_efeito_na_cli() {
    struct Conferir<'a>(&'a Path);
    impl Executor for Conferir<'_> {
        fn passo(&mut self, _: &Agendamento) -> Passo {
            let r = ler(self.0).unwrap();
            assert!(r.itens[0].pendente.is_some());
            assert_eq!(r.itens[0].proxima_ms, None);
            Passo::Terminou
        }
    }
    let dir = tempfile::tempdir().unwrap();
    let mut r = Registro {
        itens: vec![agenda(Regra::UmaVez { em_ms: 1_000 })],
    };
    tique(dir.path(), &mut r, 1_000, &mut Conferir(dir.path())).unwrap();
}

#[test]
fn corrida_com_rascunho_adia_so_quando_nada_foi_escrito() {
    let (_dir, state) = fixture();
    let mut a = agenda(Regra::Intervalo { minutos: 5 });
    a.pendente = Some(Pendente {
        chave: "teste".into(),
        fase: "enviada".into(),
        conversa: Some("conv-teste".into()),
        acao: Some("acao".into()),
    });
    let mut action = acao_interna(&state, "conv-teste", ActionKind::Fila("sintético".into()));
    action.id = "acao".into();
    action.state = "rejected".into();
    action.error = Some("Entrada não está disponível".into());
    state.mobile.actions.lock().unwrap().push_back(action);
    assert!(matches!(ExecutorReal(&state).passo(&a), Passo::Repetir));
    state.mobile.actions.lock().unwrap()[0].error =
        Some("falha de escrita possivelmente parcial".into());
    assert!(matches!(ExecutorReal(&state).passo(&a), Passo::Falhou));
}

#[cfg(unix)]
#[test]
fn conversa_fechada_reabre_com_resume_na_mesma_conta() {
    let (dir, state) = fixture();
    let perfil = dir.path().join("perfil");
    std::fs::create_dir_all(&perfil).unwrap();
    atomic_write_json(&dir.path().join("profiles.json"), &json!({"profiles":[{"id":"falsa","provider":"claude","name":"Sintética","config_dir":perfil.to_string_lossy(),"builtin":false,"created_at_ms":0}]})).unwrap();
    atomic_write_json(&dir.path().join("conversations.json"), &json!({"version":1,"conversations":[{
        "id":"conv-teste","project_id":"p","cwd":dir.path().to_string_lossy(),"title":"Retomar","created_at_ms":1,
        "segments":[{"provider":"claude","profile_id":"falsa","external_session_id":"sessao-sintetica","started_at_ms":1}]
    }]})).unwrap();
    let script = dir.path().join("cli-retomar.sh");
    std::fs::write(&script, "#!/bin/sh\nprintf '%s\\n' \"$@\" > argumentos.txt\nprintf '%s' \"$CLAUDE_CONFIG_DIR\" > conta.txt\nwhile :; do printf '\\033[?2004h\\r\\n❯ '; IFS= read -r linha || exit 0; done\n").unwrap();
    publish(
        &state,
        vec![PublishedProject {
            id: "p".into(),
            name: "Teste".into(),
            path: dir.path().to_string_lossy().into_owned(),
        }],
        vec![PublishedAgent {
            id: "claude".into(),
            label: "CLI falsa".into(),
            command: format!("/bin/sh {}", script.display()),
            resume: None,
        }],
        None,
    )
    .unwrap();
    let a = agenda(Regra::UmaVez { em_ms: 1_000 });
    assert_eq!(preparar(&state, &a).unwrap(), "conv-teste");
    let (_, Some(sessao)) = current(&state, "conv-teste").unwrap() else {
        panic!("não retomou")
    };
    let sid = sessao.meta.lock().unwrap().id.clone();
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(10);
    while !dir.path().join("conta.txt").exists() && std::time::Instant::now() < limite {
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    let args = std::fs::read_to_string(dir.path().join("argumentos.txt"));
    let conta = std::fs::read_to_string(dir.path().join("conta.txt"));
    // Outra ocorrência na mesma conversa reutiliza a sessão viva.
    assert_eq!(preparar(&state, &a).unwrap(), "conv-teste");
    assert_eq!(state.sessions.lock().unwrap().len(), 1);
    // Fork: Stop mantém a entrada Live, mas a próxima ocorrência deve retomar em outra PTY.
    stop_session(&state, &sid).unwrap();
    assert!(matches!(state.sessions.lock().unwrap().get(&sid), Some(SessionEntry::Live(_))));
    assert_eq!(preparar(&state, &a).unwrap(), "conv-teste");
    let (_, Some(retomada)) = current(&state, "conv-teste").unwrap() else { panic!("não retomou após Stop") };
    let nova_sid = retomada.meta.lock().unwrap().id.clone();
    assert_ne!(nova_sid, sid);
    assert_eq!(state.sessions.lock().unwrap().len(), 1);
    let _ = close_session(&state, &nova_sid);
    assert_eq!(args.unwrap(), "--resume\nsessao-sintetica\n");
    assert_eq!(conta.unwrap(), perfil.to_string_lossy());
}
