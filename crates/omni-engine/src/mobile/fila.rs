//! Fila de ações do chat (`Action`, `enqueue`, `execute`, `execute_checked`) e as rotas que entram nela: prompt, aprovar, escolher, modo, teclas e destravar.
//! Saiu do antigo `mobile.rs` na Onda 2.0, sem mudança de lógica.
use super::*;

#[derive(Clone, Serialize)]
pub(super) struct Action {
    pub(super) id: String, pub(super) conversation_id: String, pub(super) session_id: String, pub(super) state: String,
    pub(super) created_at_ms: u64, pub(super) error: Option<String>,
    #[serde(skip)] pub(super) key: String,
    #[serde(skip)] pub(super) revision: String,
    #[serde(skip)] pub(super) kind: ActionKind,
}
#[derive(Clone, PartialEq)]
pub(super) enum ActionKind { Prompt(String), Approval(bool), Spawn(SpawnPlan), Modo(String), Ajustes(Ajustes), Retomar(Retomada),
    Comando(comandos::Pedido), // Comandos oficiais da Conversa (frente 2C).
    /// Mensagem mandada com o agente no meio de um turno: entra na fila da CLI.
    Fila(String),
    /// Resposta a um menu da CLI (`interaction::escolha`).
    Escolha(Resposta),
    /// Fork (perguntas na Conversa): outra opção do diálogo de permissão (`opcoes_da_permissao`).
    OpcaoDaPermissao(u8) }

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Resposta {
    /// O número da opção: escolhe, ou marca/desmarca quando o menu é de marcar várias.
    Opcao(u8),
    /// Tab: no marcar-várias, segue para a próxima pergunta ou para a revisão.
    Avancar,
    /// Esc.
    Fechar,
}

/// Teclas mandadas pelo terminal da página — o mesmo que digitar no terminal do PC (Esc para
/// interromper, setas e Enter num menu da CLI). Limite pequeno: é teclado, não colagem de arquivo.
#[derive(Deserialize)] pub(super) struct TeclasBody { pub(super) dados: String }
pub(super) const TECLAS_MAX: usize = 8 * 1024;
pub(super) async fn teclas(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Json(body): Json<TeclasBody>) -> Result<Json<Value>, ApiError> {
    if body.dados.is_empty() || body.dados.len() > TECLAS_MAX {
        return Err(error(StatusCode::BAD_REQUEST,"Entrada vazia ou grande demais"));
    }
    tokio::task::spawn_blocking(move || {
        let (_, sessao) = current(&state,&id)?;
        let sessao = sessao.ok_or_else(||error(StatusCode::CONFLICT,"Sessão não está aberta no PC"))?;
        let sessao_id = sessao.meta.lock().map(|meta| meta.id.clone()).map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Sessão indisponível"))?;
        write_terminal(&state,&sessao_id,&body.dados).map_err(|erro|error(StatusCode::CONFLICT,&erro.to_string()))?;
        Ok(Json(json!({"ok":true})))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao enviar teclas"))?
}
/// Deixa a conversa pronta para receber mensagem quando a tela não foi reconhecida ("Entrada não
/// reconhecida"). Com o agente trabalhando ou perguntando algo, recusa (interromper mataria o turno).
/// 1. Agente fora do ar no terminal: reabre a sessão (`--resume`, mesma conversa).
/// 2. Senão, um Ctrl+C limpa o campo (rascunho digitado no terminal, menu perdido).
/// 3. Se nem assim, reabre a sessão.
/// Devolve a versão da tela pronta, para o prompt sair logo em seguida sem esbarrar no `If-Match`.
pub(super) async fn destravar_rota(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || destravar(&state,&id)).await
        .map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao destravar"))?
}

pub(super) fn destravar(state: &Arc<EngineState>, id: &str) -> Result<Json<Value>, ApiError> {
    let pronta = |state: &Arc<EngineState>, espera: std::time::Duration| -> Option<String> {
        let limite = std::time::Instant::now() + espera;
        loop {
            if let Ok((_, Some(sessao))) = current(state,id) {
                let cap = interaction::capabilities(&sessao);
                if cap.prompt { return Some(cap.revision); }
            }
            if std::time::Instant::now() >= limite { return None; }
            std::thread::sleep(std::time::Duration::from_millis(200));
        }
    };
    let (_, sessao) = current(state,id)?;
    let sessao = sessao.ok_or_else(||error(StatusCode::CONFLICT,"Sessão não está aberta no PC"))?;
    let cap = interaction::capabilities(&sessao);
    if cap.prompt { return Ok(Json(json!({"estado":"pronta","revision":cap.revision}))); }
    if cap.approve || cap.escolha.is_some() { return Err(error(StatusCode::CONFLICT,"Responda a pergunta do agente primeiro")); }
    // Fork (pergunta-da-cli): o Ctrl+C do destravar cancelaria (ou fecharia) o diálogo da CLI às cegas.
    if cap.espera.is_some() { return Err(error(StatusCode::CONFLICT,"A CLI está esperando algo no terminal; responda por lá")); }
    if sessao.reserved.load(Ordering::SeqCst) { return Err(error(StatusCode::CONFLICT,"Consulta de uso em andamento; tente de novo em alguns segundos")); }
    let (sessao_id, pid, provider) = sessao.meta.lock().map(|m| (m.id.clone(), m.pid, m.provider.clone().unwrap_or_default()))
        .map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Sessão indisponível"))?;
    let texto = sessao.interaction.lock().map(|c| c.parser.screen().contents()).unwrap_or_default();
    let rodando = interaction::provider_running(pid,&provider);
    if rodando && interaction::trabalhando(&texto) { return Err(error(StatusCode::CONFLICT,"O agente está trabalhando; a mensagem entra quando ele terminar")); }
    let reabrir = |state: &Arc<EngineState>| -> Result<Json<Value>, ApiError> {
        omni_core::registro::info("conversa_destravada", &[("sessao", &sessao_id), ("como", "reabriu")]);
        restart_session(state,&sessao_id).map_err(|erro|error(StatusCode::CONFLICT,&erro.to_string()))?;
        let revision = pronta(state, std::time::Duration::from_secs(40))
            .ok_or_else(||error(StatusCode::CONFLICT,"Reabri o agente, mas ele ainda não está pronto; tente de novo em instantes"))?;
        Ok(Json(json!({"estado":"reaberta","revision":revision})))
    };
    if !rodando { return reabrir(state); }
    write_terminal(state,&sessao_id,"\u{3}").map_err(|erro|error(StatusCode::CONFLICT,&erro.to_string()))?;
    if let Some(revision) = pronta(state, std::time::Duration::from_secs(3)) {
        omni_core::registro::info("conversa_destravada", &[("sessao", &sessao_id), ("como", "limpou")]);
        return Ok(Json(json!({"estado":"limpa","revision":revision})));
    }
    reabrir(state)
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)] pub(super) struct PromptBody { texto: String, #[serde(default)] fila: bool }
/// Fork (Onda 2, item 2): `pedido` é o id do pedido do gancho que a tela mostrou; `mensagem`, o "dizer o
/// que fazer" de quem nega. Sem pedido do gancho, os dois são ignorados e vale a tecla na tela.
#[derive(Deserialize)] #[serde(deny_unknown_fields)] pub(super) struct ApprovalBody { permitir: bool,
    #[serde(default)] pedido: Option<String>, #[serde(default)] mensagem: Option<String>,
    /// Fork (perguntas na Conversa): o número de uma opção do diálogo na tela além do Permitir/Negar.
    #[serde(default)] opcao: Option<u8> }
pub(super) async fn prompt(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, Json(body): Json<PromptBody>) -> Result<(StatusCode,Json<Value>),ApiError> {
    if body.texto.trim().is_empty() || body.texto.len() > 16_000 || body.texto.chars().any(|c|c.is_control() && c != '\n' && c != '\t') || body.texto.trim_start().starts_with('/') {
        return Err(error(StatusCode::BAD_REQUEST,"Envie texto de até 16000 bytes, sem comandos slash ou caracteres de controle"));
    }
    let kind = if body.fila { ActionKind::Fila(body.texto) } else { ActionKind::Prompt(body.texto) };
    enqueue_async(state,id,headers,kind).await
}
pub(super) async fn approve(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, Json(body): Json<ApprovalBody>) -> Result<(StatusCode,Json<Value>),ApiError> {
    // Fork (perguntas na Conversa): opção do diálogo da tela; o pedido do gancho/app-server tem cartão próprio.
    if let (Some(n), None) = (body.opcao, body.pedido.as_ref()) { return enqueue_async(state,id,headers,ActionKind::OpcaoDaPermissao(n)).await; }
    if let Some(r) = codex_rotas::aprovar(&state,&id,body.permitir,body.pedido.as_deref()).await { return r; } // 3A, mesma rota.
    // Fork (Onda 2): pedido segurado pelo gancho responde direto, sem tecla nem fila.
    if let Some(resposta) = ganchos_rotas::responder_pelo_gancho(&state,&id,body.permitir,body.pedido.clone(),body.mensagem.clone()).await { return resposta; }
    enqueue_async(state,id,headers,ActionKind::Approval(body.permitir)).await
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)] pub(super) struct EscolhaBody { opcao: Option<u8>, #[serde(default)] avancar: bool }
pub(super) async fn escolher(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, Json(body): Json<EscolhaBody>) -> Result<(StatusCode,Json<Value>),ApiError> {
    let resposta = match (body.avancar, body.opcao) { (true, _) => Resposta::Avancar, (false, Some(n)) => Resposta::Opcao(n), (false, None) => Resposta::Fechar };
    enqueue_async(state,id,headers,ActionKind::Escolha(resposta)).await
}

/// Modo de permissão, o que o `shift+tab` faz no terminal. Só `plan` e `auto`: são os dois que o
/// usuário troca no dia a dia, e um seletor de quatro estados numa tela de celular seria pior.
#[derive(Deserialize)] #[serde(deny_unknown_fields)] pub(super) struct ModoBody { modo: String }
pub(super) async fn modo(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, Json(body): Json<ModoBody>) -> Result<(StatusCode,Json<Value>),ApiError> {
    if body.modo != "plan" && body.modo != "auto" {
        return Err(error(StatusCode::BAD_REQUEST,"Modo deve ser plan ou auto"));
    }
    enqueue_async(state,id,headers,ActionKind::Modo(body.modo)).await
}

pub(super) async fn enqueue_async(state: Arc<EngineState>,id:String,headers:HeaderMap,kind:ActionKind) -> Result<(StatusCode,Json<Value>),ApiError> {
    tokio::task::spawn_blocking(move || enqueue(&state,&id,&headers,kind)).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha na fila"))?
}
pub(super) fn enqueue(state:&Arc<EngineState>,id:&str,headers:&HeaderMap,kind:ActionKind) -> Result<(StatusCode,Json<Value>),ApiError> {
    let key = headers.get("idempotency-key").and_then(|h|h.to_str().ok()).filter(|s|!s.is_empty() && s.len() <= 128)
        .ok_or_else(||error(StatusCode::BAD_REQUEST,"Idempotency-Key obrigatório"))?;
    let mut actions = state.mobile.actions.lock().expect("actions poisoned");
    if let Some(action) = actions.iter().find(|a| a.key == key) {
        if action.conversation_id != id || action.kind != kind { return Err(error(StatusCode::CONFLICT,"Chave já usada para outra ação")); }
        return Ok((StatusCode::ACCEPTED,Json(json!(action))));
    }
    actions.retain(|a| now_ms().saturating_sub(a.created_at_ms) < 600_000 || a.state == "queued" || a.state == "executing");
    if actions.len() >= 1024 || actions.iter().filter(|a|a.state == "queued").count() >= 128 {
        return Err(error(StatusCode::TOO_MANY_REQUESTS,"Fila cheia; aguarde"));
    }
    // Spawn não tem tela viva para ficar obsoleta: pula `capabilities` e `If-Match`, mas mantém
    // idempotência, limite de fila e expiração — o que impede um toque repetido de abrir duas PTYs.
    if matches!(kind, ActionKind::Spawn(_) | ActionKind::Retomar(_)) {
        // Retomar devolve a conversa que vai nascer (ou voltar): a tela navega para ela.
        let conversation_id = if matches!(kind, ActionKind::Retomar(_)) { id.to_string() } else { String::new() };
        let action = Action { id: state.next_id(),conversation_id,session_id:String::new(),
            state:"queued".into(),created_at_ms:now_ms(),error:None,key:key.into(),revision:String::new(),
            kind:kind.clone() };
        let response = json!(action); actions.push_back(action); drop(actions);
        state.mobile.pending.notify_one();
        return Ok((StatusCode::ACCEPTED,Json(response)));
    }
    let (_,session) = current(state,id)?;
    let session = session.ok_or_else(||error(StatusCode::CONFLICT,"Nenhuma sessão existente para esta conversa"))?;
    let capability = interaction::capabilities(&session);
    let supported = match kind {
        // Trocar de modo é tecla no composer: vale a mesma porta do prompt (CLI ocioso e tela
        // reconhecida). Com o agente no meio de um turno, o shift+tab não é para ele.
        // Reabrir no meio de um turno mataria o trabalho em andamento.
        ActionKind::Prompt(_) | ActionKind::Modo(_) | ActionKind::Ajustes(_) => capability.prompt,
        ActionKind::Comando(_) => capability.prompt, // Comandos oficiais (frente 2C).
        // Fila vale com o agente parado (vira um prompt comum) ou trabalhando com o campo livre.
        ActionKind::Fila(_) => capability.prompt || capability.enfileirar,
        ActionKind::Approval(_) => capability.approve,
        ActionKind::OpcaoDaPermissao(n) => capability.approve && capability.approval_options.iter().any(|o| o.numero == n),
        ActionKind::Escolha(resposta) => capability.escolha.as_ref().is_some_and(|menu| match resposta {
            Resposta::Opcao(n) => menu.opcoes.iter().any(|o| o.numero == n),
            Resposta::Avancar => menu.multipla,
            Resposta::Fechar => true,
        }),
        ActionKind::Spawn(_) | ActionKind::Retomar(_) => false,
    };
    if !supported { return Err(error(StatusCode::CONFLICT,"Sessão ocupada ou tela não reconhecida; atualize o status")); }
    // Fila não confere a versão da tela: com o agente trabalhando ela muda a cada quadro do spinner,
    // e a trava recusaria quase todo envio. O que protege a fila é o campo vazio, conferido de novo
    // na execução — colar no meio de um rascunho continua impossível.
    // Mensagem nova e trocar conta/modelo/esforço ou modo não respondem a nenhum diálogo — vão para um estado
    // absoluto e só passa com o agente parado e o campo livre (`capability.prompt`, logo acima). A versão
    // da tela vinda da faixa do terminal tem até 3 s e caía em 412 com qualquer redesenho.
    let sem_versao = match &kind { ActionKind::Fila(_) | ActionKind::Ajustes(_) | ActionKind::Modo(_) => true, ActionKind::Prompt(texto) => mensagem_comum(texto), _ => false };
    if !sem_versao && headers.get("if-match").and_then(|h|h.to_str().ok()) != Some(capability.revision.as_str()) {
        return Err(error(StatusCode::PRECONDITION_FAILED,"O contexto mudou; atualize antes de responder"));
    }
    let session_id = session.meta.lock().expect("metadata poisoned").id.clone();
    let action = Action { id: state.next_id(),conversation_id:id.into(),session_id,state:"queued".into(),created_at_ms:now_ms(),error:None,key:key.into(),revision:capability.revision,kind };
    let response = json!(action); actions.push_back(action); drop(actions);
    state.mobile.pending.notify_one();
    Ok((StatusCode::ACCEPTED,Json(response)))
}

pub(super) fn execute(state:&Arc<EngineState>, action:&Action) -> Result<()> {
    match &action.kind {
        ActionKind::Spawn(plan) => spawn_from_mobile(state,action,plan),
        // O modo tem laço próprio: aperta, relê a tela e repete, então não pode segurar os locks
        // como o caminho de prompt/aprovação segura.
        ActionKind::Modo(alvo) => {
            let (_,session) = current(state,&action.conversation_id).map_err(|_|anyhow!("Conversa removida"))?;
            let session = session.ok_or_else(||anyhow!("Sessão encerrada"))?;
            interaction::alternar_modo(state,&session,alvo).map(|_|())
        }
        ActionKind::Ajustes(ajustes) => reabrir_com(state,&action.conversation_id,ajustes),
        ActionKind::Retomar(plano) => retomar(state,action,plano),
        ActionKind::Comando(pedido) => comandos::executar(state,action,pedido), // Frente 2C.
        _ => execute_checked(state,action,interaction::provider_running),
    }
}

/// Mensagem do usuário, não comando da CLI (`/fast`, `/compact`… vêm da lista fechada e seguem presos
/// à versão da tela). A rota de prompt já recusa texto começando com `/`.
fn mensagem_comum(texto: &str) -> bool { !texto.trim_start().starts_with('/') }

pub(super) fn execute_checked(state:&Arc<EngineState>, action:&Action, is_running: impl Fn(Option<u32>, &str) -> bool) -> Result<()> {
    if now_ms().saturating_sub(action.created_at_ms) >= 60_000 { return Err(anyhow!("Ação expirada")); }
    let (_,current_session) = current(state,&action.conversation_id).map_err(|_|anyhow!("Conversa removida"))?;
    let session = current_session.ok_or_else(||anyhow!("Sessão encerrada"))?;
    if let Some(r) = codex_rotas::executar(state,action,&session) { return r; } // 3A, turn/start ou turn/steer.
    // Hold the sessions registry through validation/write so close cannot replace this session.
    let sessions = state.sessions.lock().map_err(|_|anyhow!("sessions poisoned"))?;
    if !matches!(sessions.get(&action.session_id),Some(SessionEntry::Live(s)) if Arc::ptr_eq(s,&session)) { return Err(anyhow!("Sessão trocada")); }
    let mut context = session.interaction.lock().map_err(|_|anyhow!("screen poisoned"))?;
    // Mensagem nova (Prompt) também não exige a mesma versão da tela: o que a protege é o campo
    // vazio sem diálogo (`interaction::ready`, conferido logo abaixo). A CLI redesenha a linha de status
    // logo depois de abrir e o primeiro envio caía em "Contexto mudou". Aprovação e menu continuam presos à
    // tela exata que o usuário viu.
    let fila = match &action.kind { ActionKind::Fila(_) => true, ActionKind::Prompt(texto) => mensagem_comum(texto), _ => false };
    if session.reserved.load(Ordering::SeqCst) || (!fila && context.revision(&session) != action.revision) { return Err(anyhow!("Contexto mudou antes da execução")); }
    let meta = session.meta.lock().map_err(|_|anyhow!("metadata poisoned"))?.clone();
    let provider = meta.provider.as_deref().unwrap_or("");
    if !is_running(meta.pid,provider) { return Err(anyhow!("CLI não está mais ativo")); }
    // Prompt vai em duas partes: a colagem agora, o Enter depois que ela aparece no campo (ver
    // `interaction::submeter_colagem`). Aprovação é tecla de menu e continua num write só.
    let (data, submeter) = match &action.kind {
        ActionKind::Prompt(text) if interaction::ready_no(&context,provider) => (format!("\x1b[200~{text}\x1b[201~"), true),
        ActionKind::Fila(text) if interaction::ready_no(&context,provider)
            || interaction::pode_enfileirar_no(&context,provider) => (format!("\x1b[200~{text}\x1b[201~"), true),
        ActionKind::Approval(allow) => { let dialog = interaction::approval(&context.parser.screen().contents(),provider).ok_or_else(||anyhow!("Aprovação não reconhecida"))?; (if *allow { dialog.0 } else { dialog.1 }, false) },
        // Fork (perguntas na Conversa): setas num write e a confirmação noutro, como no menu genérico.
        ActionKind::OpcaoDaPermissao(n) => {
            let opcao = crate::opcoes_da_permissao::opcoes(&context.parser.screen().contents(),provider).into_iter()
                .find(|o| o.numero == *n).ok_or_else(||anyhow!("Opção da permissão não reconhecida"))?;
            if let Some(setas) = &opcao.setas {
                let mut writer = session.writer.lock().map_err(|_|anyhow!("writer poisoned"))?;
                writer.write_all(setas.as_bytes())?; writer.flush()?;
                drop(writer);
                std::thread::sleep(std::time::Duration::from_millis(150));
            }
            (opcao.fim, false)
        }
        // O número sozinho escolhe (confirmado num Claude Code 2.1.302 real); Esc fecha o menu.
        ActionKind::Escolha(opcao) => {
            // Fork (pergunta-da-cli): o mesmo menu que a conversa mostrou, inclusive o genérico da CLI.
            let menu = interaction::menu_na_tela(context.parser.screen(),provider).ok_or_else(||anyhow!("Menu não reconhecido"))?;
            match opcao {
                // Fork (pergunta-da-cli): menu genérico com teclas próprias (setas + Enter, "y"…). As
                // setas vão num write e o Enter em outro, como na prévia: uma rajada só pode virar colagem.
                Resposta::Opcao(n) if crate::pergunta_da_cli::teclas_da_opcao(&menu,*n).is_some() => {
                    let (setas, fim) = crate::pergunta_da_cli::teclas_da_opcao(&menu,*n).unwrap_or_default();
                    if let Some(setas) = setas {
                        let mut writer = session.writer.lock().map_err(|_|anyhow!("writer poisoned"))?;
                        writer.write_all(setas.as_bytes())?; writer.flush()?;
                        drop(writer);
                        std::thread::sleep(std::time::Duration::from_millis(150));
                    }
                    (fim, false)
                }
                // Antigravity: menu de setas ("↑/↓ Navigate · enter Select"). Anda da opção marcada com
                // `>` até a pedida e confirma.
                Resposta::Opcao(n) if provider == "antigravity" && menu.opcoes.iter().any(|o| o.numero == *n) => {
                    let tela = context.parser.screen().contents();
                    let marcada = tela.lines().filter_map(|l| l.trim().strip_prefix("> ")?.split_once(". ")?.0.parse::<u8>().ok())
                        .last().ok_or_else(||anyhow!("Opção marcada não reconhecida"))?;
                    let seta = if *n >= marcada { "\x1b[B" } else { "\x1b[A" };
                    (format!("{}\r", seta.repeat(n.abs_diff(marcada) as usize)), false)
                }
                // Com prévia o número só move a seleção: manda o Enter depois, em outro write — junto, a
                // CLI recebe uma rajada só e pode não tratar como duas teclas.
                Resposta::Opcao(n) if menu.previa && menu.opcoes.iter().any(|o| o.numero == *n) => {
                    let mut writer = session.writer.lock().map_err(|_|anyhow!("writer poisoned"))?;
                    writer.write_all(n.to_string().as_bytes())?; writer.flush()?;
                    drop(writer);
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    ("\r".into(), false)
                }
                Resposta::Opcao(n) if menu.opcoes.iter().any(|o| o.numero == *n) => (n.to_string(), false),
                Resposta::Opcao(_) => return Err(anyhow!("Opção inexistente")),
                Resposta::Avancar if menu.multipla => ("\t".into(), false),
                Resposta::Avancar => return Err(anyhow!("Este menu não é de marcar várias")),
                Resposta::Fechar => ("\x1b".into(), false),
            }
        }
        _ => return Err(anyhow!("Entrada não está disponível")),
    };
    context.input_revision += 1;
    {
        let mut writer = session.writer.lock().map_err(|_|anyhow!("writer poisoned"))?;
        writer.write_all(data.as_bytes())?; writer.flush()?;
    }
    if !submeter { return Ok(()); }
    // Solta os locks antes de esperar: o leitor da PTY precisa deles para a tela andar.
    drop(context);
    drop(sessions);
    interaction::submeter_colagem(&session)
}
