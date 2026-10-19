//! Fork: as ferramentas MCP do orquestrador (ver `orquestracao.rs`).
//!
//! Plataformas, contas e modelos saem do que o app já publica (`workspace.agents`, perfis,
//! `modelos_claude`, cache de modelos do Codex, `agy models`): uma CLI nova que o app passar a
//! suportar aparece aqui sem mudar nada. Todo acesso a um subagente passa por `do_orquestrador`:
//! o token só enxerga os subagentes da própria conversa.

use super::*;

pub(super) fn lista() -> Value {
    let agente = json!({"type": "string", "description": "Id do subagente (o campo `agente` devolvido por criar_agente ou listar_agentes)"});
    json!([
        {"name": "listar_plataformas_e_contas",
         "description": "Lista as plataformas de agente que o app suporta (Claude Code, Codex, Antigravity…), as contas de cada uma e os modelos conhecidos. Um modelo fora da lista também é aceito.",
         "inputSchema": {"type": "object", "properties": {}}},
        {"name": "criar_agente",
         "description": "Abre um subagente na mesma pasta deste orquestrador e manda a tarefa assim que a CLI dele ficar pronta. Respeita o limite de subagentes vivos. Depois use esperar e ler_conversa.",
         "inputSchema": {"type": "object", "required": ["plataforma", "tarefa"], "properties": {
            "plataforma": {"type": "string", "description": "Id da plataforma (ex.: claude, codex, antigravity)"},
            "conta": {"type": "string", "description": "Id ou nome da conta; sem ela, a conta padrão da plataforma"},
            "modelo": {"type": "string", "description": "Modelo para a CLI (ex.: haiku). Sem ele, o padrão da CLI"},
            "esforco": {"type": "string", "description": "Esforço de raciocínio (ex.: low, medium, high)"},
            "tarefa": {"type": "string", "description": "Primeira mensagem para o subagente"},
            "titulo": {"type": "string", "description": "Nome da conversa do subagente"}}}},
        {"name": "mandar_prompt",
         "description": "Manda uma mensagem a um subagente. Com ele trabalhando, entra na fila da CLI dele.",
         "inputSchema": {"type": "object", "required": ["agente", "texto"], "properties": {"agente": agente, "texto": {"type": "string"}}}},
        {"name": "ler_conversa",
         "description": "Últimas mensagens de um subagente e o estado dele.",
         "inputSchema": {"type": "object", "required": ["agente"], "properties": {"agente": agente,
            "ultimas": {"type": "integer", "minimum": 1, "maximum": regra::ULTIMAS_MAXIMO, "description": "Quantas mensagens (padrão 5)"}}}},
        {"name": "esperar",
         "description": "Espera o subagente terminar o turno (ou pedir algo ao usuário), até `segundos` (máx. 120). Se estourar, devolve 'ainda trabalhando': chame de novo.",
         "inputSchema": {"type": "object", "required": ["agente"], "properties": {"agente": agente,
            "segundos": {"type": "integer", "minimum": 1, "maximum": regra::ESPERA_MAXIMA_S}}}},
        {"name": "listar_agentes",
         "description": "Os subagentes deste orquestrador, com plataforma, conta, modelo, tarefa, estado e última mensagem.",
         "inputSchema": {"type": "object", "properties": {}}},
        {"name": "parar_agente",
         "description": "Encerra a CLI de um subagente (a conversa continua no histórico). Libera vaga no limite.",
         "inputSchema": {"type": "object", "required": ["agente"], "properties": {"agente": agente}}},
    ])
}

fn texto_do_campo<'a>(args: &'a Value, campo: &str) -> Option<&'a str> {
    args.get(campo).and_then(Value::as_str).map(str::trim).filter(|v| !v.is_empty())
}

fn obrigatorio<'a>(args: &'a Value, campo: &str) -> Result<&'a str, String> {
    texto_do_campo(args, campo).ok_or_else(|| format!("Falta o campo `{campo}`"))
}

fn mensagem_do_erro(erro: ApiError) -> String {
    erro.1 .0["error"].as_str().unwrap_or("Falha").to_owned()
}

fn em_json(valor: Value) -> String { serde_json::to_string_pretty(&valor).unwrap_or_default() }

/// O subagente **deste** orquestrador, ou erro. É a trava de escopo do token.
fn do_orquestrador(state: &EngineState, orquestrador: &str, agente: &str) -> Result<regra::Agente, String> {
    ler_registro(state).agente_de(orquestrador, agente).cloned()
        .ok_or_else(|| "Esse agente não existe ou não é deste orquestrador. Use listar_agentes.".to_owned())
}

pub(super) async fn chamar(state: &Arc<EngineState>, orquestrador: &str, nome: &str, args: &Value) -> Result<String, String> {
    let args = if args.is_object() { args.clone() } else { json!({}) };
    let (state, orquestrador) = (state.clone(), orquestrador.to_owned());
    match nome {
        "esperar" => esperar(&state, &orquestrador, &args).await,
        "mandar_prompt" => {
            let agente = obrigatorio(&args, "agente")?.to_owned();
            let texto = obrigatorio(&args, "texto")?.to_owned();
            bloqueante(move || mandar(&state, &orquestrador, &agente, &texto)).await
        }
        "listar_plataformas_e_contas" => bloqueante(move || Ok(em_json(catalogo(&state)))).await,
        "criar_agente" => bloqueante(move || criar(&state, &orquestrador, &args)).await,
        "ler_conversa" => bloqueante(move || ler(&state, &orquestrador, &args)).await,
        "listar_agentes" => bloqueante(move || Ok(em_json(listar_agentes(&state, &orquestrador)))).await,
        "parar_agente" => {
            let agente = obrigatorio(&args, "agente")?.to_owned();
            bloqueante(move || parar(&state, &orquestrador, &agente)).await
        }
        _ => Err(format!("Ferramenta desconhecida: {nome}")),
    }
}

async fn bloqueante(f: impl FnOnce() -> Result<String, String> + Send + 'static) -> Result<String, String> {
    tokio::task::spawn_blocking(f).await.map_err(|_| "Falha interna".to_owned())?
}

/* ── listar_plataformas_e_contas ─────────────────────────────────────────────────────────────── */

/// Modelos conhecidos de uma plataforma, como a tela de ajustes mostra. Plataforma sem lista: vazio
/// (o modelo pedido passa mesmo assim, se for um argumento seguro).
fn modelos_de(state: &EngineState, plataforma: &str, perfil: Option<&omni_core::Profile>) -> Vec<Value> {
    match plataforma {
        "claude" => omni_core::modelos_claude::MODELOS.iter().map(|(id, _, esforcos)| json!({"id": id, "esforcos": esforcos})).collect(),
        "codex" => perfil.map(|p| modelos_codex(Path::new(&p.config_dir))).unwrap_or_default().into_iter()
            .map(|m| json!({"id": m["id"], "esforcos": m["efforts"]})).collect(),
        "antigravity" => omni_core::antigravity::familias(&saida_agy_models(dir(state))).into_iter()
            .map(|f| json!({"id": f.id, "esforcos": f.esforcos})).collect(),
        _ => Vec::new(),
    }
}

fn catalogo(state: &EngineState) -> Value {
    let agentes = state.mobile.workspace.lock().expect("workspace poisoned").agents.clone();
    let perfis = omni_core::profiles(dir(state));
    json!({
        "plataformas": agentes.iter().map(|agente| {
            let contas: Vec<&omni_core::Profile> = perfis.iter().filter(|p| p.provider == agente.id).collect();
            let padrao = contas.iter().find(|p| p.builtin).or(contas.first()).copied();
            json!({
                "plataforma": agente.id, "nome": agente.label,
                "contas": contas.iter().map(|p| json!({"id": p.id, "nome": p.name, "padrao": p.builtin})).collect::<Vec<_>>(),
                "modelos": modelos_de(state, &agente.id, padrao),
            })
        }).collect::<Vec<_>>(),
        "limite_de_subagentes_vivos": ler_registro(state).limite(),
    })
}

/* ── criar_agente ────────────────────────────────────────────────────────────────────────────── */

fn vivos(state: &EngineState, orquestrador: &str) -> usize {
    ler_registro(state).orquestrador(orquestrador).map(|o| o.agentes.iter()
        .filter(|a| estado_de(state, &a.conversation_key).0.vivo()).count()).unwrap_or(0)
}

fn criar(state: &Arc<EngineState>, orquestrador: &str, args: &Value) -> Result<String, String> {
    let plataforma = obrigatorio(args, "plataforma")?.to_owned();
    let tarefa = obrigatorio(args, "tarefa")?.to_owned();
    regra::texto_valido(&tarefa)?;
    let modelo = texto_do_campo(args, "modelo").map(str::to_owned);
    let esforco = texto_do_campo(args, "esforco").map(str::to_owned);
    let extras = regra::argumentos_de_modelo(&plataforma, modelo.as_deref(), esforco.as_deref())?;
    let titulo = texto_do_campo(args, "titulo").map(str::to_owned);
    let (mae, _) = current(state, orquestrador).map_err(mensagem_do_erro)?;

    let publicados = state.mobile.workspace.lock().expect("workspace poisoned").agents.clone();
    let Some(publicado) = publicados.iter().find(|a| a.id == plataforma) else {
        let nomes: Vec<&str> = publicados.iter().map(|a| a.id.as_str()).collect();
        return Err(format!("Plataforma desconhecida: {plataforma}. Disponíveis: {}", nomes.join(", ")));
    };
    let perfis = omni_core::profiles(dir(state));
    let conta = match texto_do_campo(args, "conta") {
        Some(pedida) => Some(perfis.iter().filter(|p| p.provider == plataforma)
            .find(|p| p.id == pedida || p.name.eq_ignore_ascii_case(pedida)).map(|p| p.id.clone())
            .ok_or_else(|| format!("Conta desconhecida para {plataforma}: {pedida}. Use listar_plataformas_e_contas."))?),
        None => None,
    };

    // Conferir o limite e abrir sob a mesma trava: duas chamadas juntas não passam as duas.
    static CRIANDO: Mutex<()> = Mutex::new(());
    let _criando = CRIANDO.lock().expect("criando poisoned");
    regra::pode_criar(vivos(state, orquestrador), ler_registro(state).limite())?;

    let corpo = SpawnBody { project_id: mae.project_id.clone(), provider: plataforma.clone(), profile_id: conta,
        titulo: Some(titulo.clone().unwrap_or_else(|| format!("{} · subagente", publicado.label))) };
    let mut plano = resolve_spawn(state, &corpo).map_err(mensagem_do_erro)?;
    plano.titulo_do_usuario = titulo.is_some();
    // Pasta = a do orquestrador (pode ser uma worktree, não a raiz do projeto).
    if Path::new(&mae.cwd).is_dir() && mae.cwd != plano.cwd {
        plano.cwd = mae.cwd.clone();
        if let (Some(sessao), Some(perfil)) = (&plano.external_session_id, perfis.iter().find(|p| Some(&p.id) == plano.profile_id.as_ref())) {
            plano.transcript_path = Some(omni_core::conversations::claude_transcript_path(Path::new(&perfil.config_dir), &plano.cwd, sessao)
                .to_string_lossy().into_owned());
        }
    }
    plano.command += &extras;
    let conversa = format!("conv-{}", uuid::Uuid::new_v4());
    plano.conversation_id = Some(conversa.clone());
    let acao = Action { id: state.next_id(), conversation_id: String::new(), session_id: String::new(), state: "executing".into(),
        created_at_ms: now_ms(), error: None, key: String::new(), revision: String::new(), kind: ActionKind::Spawn(plano.clone()) };
    spawn_from_mobile(state, &acao, &plano).map_err(|e| format!("Não consegui abrir o subagente: {e}"))?;

    let agente = regra::Agente { conversation_key: conversa.clone(), plataforma: plataforma.clone(), conta: plano.profile_id.clone(),
        modelo: modelo.clone(), esforco: esforco.clone(), tarefa: tarefa.clone(), criado_em: now_ms() };
    mudar_registro(state, |r| r.adicionar(orquestrador, agente)).map_err(|_| "Abri o subagente, mas não consegui gravar o registro".to_owned())?;
    drop(_criando);
    registro::info("orquestracao_agente_criado", &[("orquestrador", orquestrador), ("agente", &conversa), ("provider", &plataforma),
        ("perfil", plano.profile_id.as_deref().unwrap_or("-")), ("modelo", modelo.as_deref().unwrap_or("-"))]);

    state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").insert(conversa.clone(), "esperando a CLI ficar pronta".into());
    let (estado, id) = (state.clone(), conversa.clone());
    std::thread::spawn(move || mandar_tarefa_quando_pronta(&estado, &id, &tarefa, std::time::Duration::from_secs(180)));
    Ok(em_json(json!({
        "agente": conversa, "plataforma": plataforma, "conta": nome_da_conta(&perfis, plano.profile_id.as_deref()),
        "modelo": modelo, "esforco": esforco, "estado": "abrindo",
        "nota": "A tarefa vai assim que a CLI ficar pronta. Chame esperar(agente) e depois ler_conversa(agente).",
    })))
}

/// Põe o prompt na fila de ações do chat (a mesma do celular) e espera ele ser digitado.
fn enviar(state: &Arc<EngineState>, conversa: &str, kind: ActionKind, revisao: &str) -> Result<(), String> {
    let mut headers = HeaderMap::new();
    headers.insert("idempotency-key", format!("orq-{}", uuid::Uuid::new_v4()).parse().expect("chave"));
    headers.insert("if-match", revisao.parse().map_err(|_| "Revisão inválida".to_owned())?);
    let (_, Json(acao)) = enqueue(state, conversa, &headers, kind).map_err(mensagem_do_erro)?;
    let id = acao["id"].as_str().unwrap_or_default().to_owned();
    let limite = std::time::Instant::now() + std::time::Duration::from_secs(20);
    loop {
        let situacao = state.mobile.actions.lock().expect("actions poisoned").iter().find(|a| a.id == id)
            .map(|a| (a.state.clone(), a.error.clone()));
        match situacao {
            Some((estado, _)) if estado == "sent" => {
                state.mobile.orquestracao.enviado_em.lock().expect("enviado poisoned").insert(conversa.to_owned(), now_ms());
                return Ok(());
            }
            Some((estado, erro)) if estado == "rejected" => return Err(erro.unwrap_or_else(|| "Recusada".into())),
            None => return Err("A ação sumiu da fila".into()),
            _ if std::time::Instant::now() >= limite => return Err("A CLI não recebeu a mensagem a tempo".into()),
            _ => std::thread::sleep(std::time::Duration::from_millis(100)),
        }
    }
}

/// A tarefa inicial: espera a CLI chegar no campo vazio (passando pela confiança na pasta do Claude,
/// ver `passar_pela_confianca`). Menu ou permissão na tela fica para o usuário responder no fluxo.
pub(super) fn mandar_tarefa_quando_pronta(state: &Arc<EngineState>, conversa: &str, tarefa: &str, prazo: std::time::Duration) {
    let limite = std::time::Instant::now() + prazo;
    let resultado = loop {
        if std::time::Instant::now() >= limite { break Err("a CLI não ficou pronta a tempo".to_owned()); }
        if !state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").contains_key(conversa) { return; } // parado
        match current(state, conversa) {
            Ok((_, Some(sessao))) => {
                let cap = interaction::capabilities(&sessao);
                if cap.prompt {
                    match enviar(state, conversa, ActionKind::Prompt(tarefa.to_owned()), &cap.revision) {
                        Ok(()) => break Ok(()),
                        // A tela mudou entre ler e digitar: tenta de novo.
                        Err(_) => {}
                    }
                }
            }
            Ok((_, None)) => break Err("a sessão do subagente fechou".to_owned()),
            Err(_) => {}
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
    };
    let mut pendentes = state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned");
    match resultado {
        Ok(()) => { pendentes.remove(conversa); registro::info("orquestracao_tarefa_enviada", &[("agente", conversa)]); }
        Err(motivo) => {
            registro::aviso("orquestracao_tarefa_nao_enviada", &[("agente", conversa), ("motivo", &motivo)]);
            pendentes.insert(conversa.to_owned(), format!("tarefa não enviada: {motivo}; mande com mandar_prompt"));
        }
    }
}

/* ── mandar_prompt, ler_conversa, esperar, listar, parar ─────────────────────────────────────── */

fn mandar(state: &Arc<EngineState>, orquestrador: &str, agente: &str, texto: &str) -> Result<String, String> {
    do_orquestrador(state, orquestrador, agente)?;
    regra::texto_valido(texto)?;
    if let Some(motivo) = state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").get(agente) {
        if motivo.starts_with("esperando") { return Err("A tarefa inicial ainda não foi enviada (a CLI está abrindo). Use esperar antes.".into()); }
    }
    let (estado, sessao, cap) = estado_de(state, agente);
    let (Some(_), Some(cap)) = (sessao, cap) else { return Err("O agente está parado. Crie outro com criar_agente.".into()) };
    if estado == regra::Estado::EsperandoVoce {
        return Err("O agente está esperando o usuário (permissão ou pergunta na tela). Peça ao usuário para responder no fluxo.".into());
    }
    let (kind, como) = if cap.prompt { (ActionKind::Prompt(texto.to_owned()), "enviada") }
        else if cap.enfileirar { (ActionKind::Fila(texto.to_owned()), "na fila da CLI (o agente está trabalhando)") }
        else { return Err(format!("O agente não pode receber mensagem agora: {}", cap.reason.unwrap_or_else(|| "tela não reconhecida".into()))) };
    enviar(state, agente, kind, &cap.revision)?;
    state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").remove(agente);
    Ok(format!("Mensagem {como}. Use esperar(agente) para a resposta."))
}

/// Últimas linhas com texto da tela: para a plataforma sem transcript que o app leia.
fn tela(sessao: &Arc<LiveSession>) -> String {
    let texto = sessao.interaction.lock().map(|c| c.parser.screen().contents()).unwrap_or_default();
    let linhas: Vec<&str> = texto.lines().map(str::trim_end).filter(|l| !l.trim().is_empty()).collect();
    linhas[linhas.len().saturating_sub(15)..].join("\n")
}

fn ler(state: &Arc<EngineState>, orquestrador: &str, args: &Value) -> Result<String, String> {
    let agente = obrigatorio(args, "agente")?;
    do_orquestrador(state, orquestrador, agente)?;
    let quantas = regra::ultimas(args.get("ultimas").and_then(Value::as_u64).map(|n| n as usize));
    let todas = mensagens(state, agente);
    let (estado, sessao, _) = estado_de(state, agente);
    let ultimas: Vec<Value> = todas[todas.len().saturating_sub(quantas)..].iter().map(|(papel, texto)| json!({
        "de": if papel == "assistant" { "agente" } else { "orquestrador" }, "texto": regra::resumir(texto, 4_000),
    })).collect();
    let mut saida = json!({"agente": agente, "estado": estado.texto(), "mensagens": ultimas});
    if let Some(motivo) = state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").get(agente) { saida["tarefa"] = json!(motivo); }
    if todas.is_empty() { if let Some(sessao) = sessao { saida["tela"] = json!(tela(&sessao)); } }
    Ok(em_json(saida))
}

/// Espera sem segurar thread: um `estado_de` por segundo. "Pronto" logo depois de um envio não vale
/// (a CLI ainda não começou o turno) até 5 s.
async fn esperar(state: &Arc<EngineState>, orquestrador: &str, args: &Value) -> Result<String, String> {
    let agente = obrigatorio(args, "agente")?.to_owned();
    { let (s, o, a) = (state.clone(), orquestrador.to_owned(), agente.clone());
      bloqueante(move || do_orquestrador(&s, &o, &a).map(|_| String::new())).await?; }
    let segundos = regra::segundos_de_espera(args.get("segundos").and_then(Value::as_u64));
    let inicio = std::time::Instant::now();
    let mut viu_trabalhando = false;
    loop {
        let (s, a) = (state.clone(), agente.clone());
        let (estado, pendente) = tokio::task::spawn_blocking(move || {
            let pendente = s.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").get(&a).cloned();
            (estado_de(&s, &a).0, pendente)
        }).await.map_err(|_| "Falha interna".to_owned())?;
        let recem_enviado = state.mobile.orquestracao.enviado_em.lock().expect("enviado poisoned").get(&agente)
            .is_some_and(|quando| now_ms().saturating_sub(*quando) < 5_000);
        viu_trabalhando |= estado == regra::Estado::Trabalhando;
        let esperando_tarefa = pendente.as_deref().is_some_and(|m| m.starts_with("esperando"));
        let terminou = match estado {
            regra::Estado::Trabalhando | regra::Estado::Abrindo => false,
            regra::Estado::Pronto => !esperando_tarefa && (viu_trabalhando || !recem_enviado),
            _ => true,
        };
        if terminou {
            let (s, a) = (state.clone(), agente.clone());
            let ultima = tokio::task::spawn_blocking(move || ultima_do_agente(&s, &a)).await.ok().flatten();
            return Ok(em_json(json!({"agente": agente, "estado": estado.texto(),
                "ultima_mensagem": ultima.map(|t| regra::resumir(&t, 4_000)), "tarefa": pendente})));
        }
        if inicio.elapsed().as_secs() >= segundos {
            return Ok(format!("ainda trabalhando (estado: {}). Chame esperar de novo.", estado.texto()));
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

fn listar_agentes(state: &EngineState, orquestrador: &str) -> Value {
    let registro = ler_registro(state);
    let perfis = omni_core::profiles(dir(state));
    let agentes: Vec<Value> = registro.orquestrador(orquestrador).map(|o| o.agentes.iter().map(|a| {
        let (estado, _, _) = estado_de(state, &a.conversation_key);
        json!({"agente": a.conversation_key, "plataforma": a.plataforma, "conta": nome_da_conta(&perfis, a.conta.as_deref()),
            "modelo": a.modelo, "tarefa": regra::resumir(&a.tarefa, 200), "estado": estado.texto(),
            "ultima_mensagem": ultima_do_agente(state, &a.conversation_key).map(|t| regra::resumir(&t, 300))})
    }).collect()).unwrap_or_default();
    let vivos = agentes.iter().filter(|a| !matches!(a["estado"].as_str(), Some("parado" | "erro"))).count();
    json!({"agentes": agentes, "vivos": vivos, "limite": registro.limite()})
}

/// Encerra a CLI do subagente (a mesma parada do botão do terminal). Também usado pela tela.
pub(super) fn parar(state: &EngineState, orquestrador: &str, agente: &str) -> Result<String, String> {
    do_orquestrador(state, orquestrador, agente)?;
    state.mobile.orquestracao.pendentes.lock().expect("pendentes poisoned").remove(agente);
    let Ok((_, Some(sessao))) = current(state, agente) else { return Ok("O agente já estava parado.".into()) };
    let id = sessao.meta.lock().map(|m| m.id.clone()).map_err(|_| "Sessão indisponível".to_owned())?;
    stop_session(state, &id).map_err(|e| e.to_string())?;
    registro::info("orquestracao_agente_parado", &[("orquestrador", orquestrador), ("agente", agente)]);
    Ok("Agente parado.".into())
}
