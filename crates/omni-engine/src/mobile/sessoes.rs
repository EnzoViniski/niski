//! Abrir sessão pelo celular (plano de spawn, confiança na pasta), bifurcar, ocultar e renomear conversas.
//! Fork: saiu do antigo `mobile.rs` na Onda 2.0, sem mudança de lógica.
use super::*;

/// Tudo já resolvido **no servidor** antes de entrar na fila. O corpo do request não contribui com
/// nenhum caminho nem binário: o `cwd` vem do projeto conhecido, o `command` da lista de CLIs que o
/// desktop publicou, e o `env` do perfil gravado em `profiles.json`.
#[derive(Clone, PartialEq)]
pub(super) struct SpawnPlan {
    pub(super) project_id: String,
    pub(super) cwd: String,
    pub(super) title: String,
    /// O título veio do usuário (e não do rótulo padrão do agente).
    pub(super) titulo_do_usuario: bool,
    pub(super) provider: String,
    pub(super) profile_id: Option<String>,
    pub(super) command: String,
    pub(super) env: Vec<(String,String)>,
    pub(super) external_session_id: Option<String>,
    pub(super) transcript_path: Option<String>,
    /// Fork: id da conversa escolhido antes (o orquestrador registra o subagente com ele). `None` = novo.
    pub(super) conversation_id: Option<String>,
}

/// Conversas que você tirou da lista na web ou no celular, só pelo id. Nada é apagado: o transcript
/// e o índice ficam como estão, e reabrir é tirar o id daqui.
pub(super) const OCULTAS: &str = "conversas-ocultas.json";
/// Teto da lista: id é curto, mas um cliente em loop não deve crescer o arquivo sem fim.
pub(super) const OCULTAS_MAX: usize = 5_000;
pub(super) static OCULTAS_TRAVA: Mutex<()> = Mutex::new(());

pub(super) fn conversas_ocultas(dir: &Path) -> HashSet<String> {
    std::fs::read(dir.join(OCULTAS)).ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<String>>(&bytes).ok())
        .unwrap_or_default().into_iter().collect()
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)] pub(super) struct OcultarBody { oculta: bool }
pub(super) async fn ocultar(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Json(body): Json<OcultarBody>) -> Result<Json<Value>, ApiError> {
    // Os ids são `conv-<uuid>` e `history-<hash>`: qualquer outra coisa é lixo, não vai para o disco.
    if id.is_empty() || id.len() > 120 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_') {
        return Err(error(StatusCode::BAD_REQUEST,"Conversa inválida"));
    }
    tokio::task::spawn_blocking(move || {
        let dir = state.state_file.parent().unwrap();
        let _trava = OCULTAS_TRAVA.lock().map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Lista de ocultas indisponível"))?;
        let mut lista: Vec<String> = conversas_ocultas(dir).into_iter().filter(|item| item != &id).collect();
        if body.oculta {
            if lista.len() >= OCULTAS_MAX { return Err(error(StatusCode::TOO_MANY_REQUESTS,"Conversas ocultas demais; reabra algumas antes")); }
            lista.push(id.clone());
        }
        lista.sort();
        atomic_write_json(&dir.join(OCULTAS),&lista).map_err(|erro|error(StatusCode::INTERNAL_SERVER_ERROR,&erro.to_string()))?;
        Ok(Json(json!({"id":id,"hidden":body.oculta})))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao ocultar"))?
}

/// Bifurcar pela web/celular: o mesmo "Bifurcar" da lateral do PC. Precisa da sessão aberta — é dela
/// que sai o id da CLI e a conta. Devolve o id da conversa nova para a tela abrir.
pub(super) async fn bifurcar_rota(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>) -> Result<(StatusCode,Json<Value>), ApiError> {
    tokio::task::spawn_blocking(move || {
        let (_, sessao) = current(&state,&id)?;
        let sessao = sessao.ok_or_else(||error(StatusCode::CONFLICT,"Retome a conversa antes de bifurcar: ela precisa estar aberta no PC"))?;
        let fonte = sessao.meta.lock().map(|meta| meta.clone()).map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Sessão indisponível"))?;
        let provider = fonte.provider.clone().filter(|p| p == "claude" || p == "codex")
            .ok_or_else(||error(StatusCode::CONFLICT,"Só dá para bifurcar conversas do Claude e do Codex"))?;
        limite_de_sessoes::conferir(&state)?; // Fork: teto único e configurável.
        let resposta = bifurcar(&state, fonte, &provider, true).map_err(|erro|error(StatusCode::INTERNAL_SERVER_ERROR,&erro.to_string()))?;
        let EngineResponse::Session { session } = resposta else { return Err(error(StatusCode::INTERNAL_SERVER_ERROR,"Resposta inesperada")); };
        Ok((StatusCode::CREATED,Json(json!({"id":session.conversation_id,"title":session.name}))))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao bifurcar"))?
}

/// Renomear a conversa. Vale para o celular e para a web de computador; o desktop chega aqui pelo
/// `EngineRequest::RenameConversation`, para o índice ter **um** escritor só.
#[derive(Deserialize)] pub(super) struct NomeBody { nome: String }
pub(super) async fn renomear(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Json(body): Json<NomeBody>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let dir = state.state_file.parent().unwrap();
        let conversa = omni_core::conversations::rename(dir,&id,&body.nome)
            .map_err(|erro| error(StatusCode::BAD_REQUEST,&erro))?;
        Ok(Json(json!({"id":conversa.id,"title":conversa.title})))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao renomear"))?
}

/// `deny_unknown_fields` é peça de segurança, não capricho: um `cwd` contrabandeado no corpo vira
/// **400**, em vez de um campo ignorado em silêncio que dá a falsa impressão de ter sido aceito.
#[derive(Deserialize)] #[serde(deny_unknown_fields)]
pub(super) struct SpawnBody { pub(super) project_id: String, pub(super) provider: String, pub(super) profile_id: Option<String>, #[serde(default)] pub(super) titulo: Option<String> }

/// Resolve o pedido do celular contra o que o servidor já conhece. Nada aqui sai do corpo do
/// request além de escolhas dentro de listas fechadas.
pub(super) fn resolve_spawn(state: &Arc<EngineState>, body: &SpawnBody) -> Result<SpawnPlan, ApiError> {
    let project = known_projects(state).into_iter().find(|p| p.id == body.project_id)
        .ok_or_else(||error(StatusCode::CONFLICT,"Projeto desconhecido; abra o app no PC uma vez para publicar a lista"))?;
    resolve_spawn_no_projeto(state, body, project)
}

/// Fork (Copiloto abre conversa na tela): o resto do plano com o projeto já resolvido **no servidor**.
/// O orquestrador usa isto com um projeto conhecido ou a pasta da própria sessão dele
/// (`orquestracao_copiloto.rs`); o `project_id` do corpo é ignorado aqui.
pub(super) fn resolve_spawn_no_projeto(state: &Arc<EngineState>, body: &SpawnBody, project: PublishedProject) -> Result<SpawnPlan, ApiError> {
    let dir = state.state_file.parent().unwrap();
    if !Path::new(&project.path).is_dir() {
        return Err(error(StatusCode::CONFLICT,"A pasta do projeto não existe mais neste PC"));
    }

    // O comando vem da lista publicada: o celular escolhe um id, nunca nomeia um binário.
    let agent = state.mobile.workspace.lock().expect("workspace poisoned").agents.iter()
        .find(|a| a.id == body.provider).cloned()
        .ok_or_else(||error(StatusCode::CONFLICT,"CLI não publicada pelo desktop; abra o app no PC uma vez"))?;

    let profiles = omni_core::profiles(dir);
    let profile = match &body.profile_id {
        Some(id) => Some(profiles.iter().find(|p| &p.id == id && p.provider == body.provider)
            .ok_or_else(||error(StatusCode::CONFLICT,"Conta desconhecida para este agente"))?.clone()),
        None => profiles.iter().find(|p| p.provider == body.provider && p.builtin).cloned(),
    };

    let titulo_do_usuario = body.titulo.is_some();
    let titulo = body.titulo.clone().unwrap_or_else(|| format!("{} · celular",agent.label));
    if titulo.len() > 120 || titulo.chars().any(|c| c.is_control()) {
        return Err(error(StatusCode::BAD_REQUEST,"Título inválido"));
    }

    limite_de_sessoes::conferir(state)?; // Fork: teto único e configurável.

    // Fixar o `--session-id` do Claude aqui é o que torna o caminho do transcript conhecido de
    // imediato — sem isso a timeline do celular ficaria vazia até alguém descobrir o arquivo.
    let (command,external_session_id,transcript_path) = if body.provider == "claude" {
        let session_id = uuid::Uuid::new_v4().to_string();
        let path = profile.as_ref().map(|p| omni_core::conversations::claude_transcript_path(
            Path::new(&p.config_dir),&project.path,&session_id).to_string_lossy().into_owned());
        (format!("{} --session-id {session_id}",agent.command),Some(session_id),path)
    } else {
        (agent.command.clone(),None,None)
    };

    Ok(SpawnPlan {
        project_id: project.id, cwd: project.path, title: titulo, titulo_do_usuario, provider: body.provider.clone(),
        profile_id: profile.as_ref().map(|p| p.id.clone()),
        env: profile.as_ref().map(omni_core::env_for).unwrap_or_default(),
        command, external_session_id, transcript_path, conversation_id: None,
    })
}

pub(super) async fn create_session(State(state): State<Arc<EngineState>>, headers: HeaderMap, Json(body): Json<SpawnBody>) -> Result<(StatusCode,Json<Value>),ApiError> {
    tokio::task::spawn_blocking(move || {
        let plan = resolve_spawn(&state,&body)?;
        enqueue(&state,"",&headers,ActionKind::Spawn(plan))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha na fila"))?
}

/// Abre a sessão que o celular pediu e registra a conversa que vai dar a timeline dela.
///
/// A conversa é criada **aqui** e não no desktop porque a janela pode estar fechada — é justamente
/// esse o caso de uso. Sem a conversa, a sessão existiria mas o celular não teria o que ler.
pub(super) fn spawn_from_mobile(state:&Arc<EngineState>, action:&Action, plan:&SpawnPlan) -> Result<()> {
    if now_ms().saturating_sub(action.created_at_ms) >= 60_000 { return Err(anyhow!("Ação expirada")); }
    let dir = state.state_file.parent().unwrap().to_path_buf();

    let conversation = omni_core::conversations::Conversation {
        id: plan.conversation_id.clone().unwrap_or_else(|| format!("conv-{}",uuid::Uuid::new_v4())),
        project_id: plan.project_id.clone(),
        cwd: plan.cwd.clone(),
        title: plan.title.clone(),
        // Nome escolhido pelo usuário no celular conta como manual; sem título, o padrão automático
        // deixa o primeiro prompt nomear a conversa depois.
        title_source: if plan.titulo_do_usuario {
            omni_core::conversations::TitleSource::Manual
        } else {
            omni_core::conversations::TitleSource::Auto
        },
        created_at_ms: now_ms(),
        segments: vec![omni_core::conversations::Segment {
            provider: plan.provider.clone(),
            profile_id: plan.profile_id.clone(),
            external_session_id: plan.external_session_id.clone(),
            transcript_path: plan.transcript_path.clone(),
            terminal_session_id: None,
            started_at_ms: now_ms(),
            ended_at_ms: None,
        }],
    };

    let session = spawn_terminal_inner(
        state,
        state.next_id(),
        plan.project_id.clone(),
        plan.title.clone(),
        plan.cwd.clone(),
        None,
        Some(plan.command.clone()),
        // A PTY nasce sem ninguém olhando; o desktop redimensiona quando anexar a aba.
        // ponytail: tamanho fixo, suficiente até alguém reclamar.
        30,
        120,
        now_ms(),
        SessionOrigin {
            // Aberta sem nenhum terminal exibindo: o engine responde o `ESC[6n` do ConPTY.
            headless: true,
            env: plan.env.clone(),
            provider: Some(plan.provider.clone()),
            profile_id: plan.profile_id.clone(),
            conversation_id: Some(conversation.id.clone()),
            external_session_id: plan.external_session_id.clone(),
        },
    )?;

    // Grava a conversa só depois do spawn dar certo: falhou, não fica conversa órfã na lista.
    let mut conversation = conversation;
    conversation.segments[0].terminal_session_id = Some(session.id.clone());
    // Fork (fix-orquestracao): ler-alterar-gravar sob a trava do índice.
    omni_core::indice_de_conversas::mudar_indice(&dir, |index| { index.push(conversation); true })?;
    if plan.provider == "claude" { passar_pela_confianca(state,&session.id); }
    Ok(())
}

/// Pasta que a conta nunca abriu: o Claude para no "confiar nesta pasta?" e o celular só via
/// "entrada não reconhecida", sem saída. A pasta é um projeto da lista do usuário — o mesmo que o app
/// do PC pré-aprova (`ensure_agent_trust`). Em thread à parte: a fila de ações não espera o Claude subir.
pub(super) fn passar_pela_confianca(state: &Arc<EngineState>, session_id: &str) {
    let viva = state.sessions.lock().ok().and_then(|sessoes| match sessoes.get(session_id) {
        Some(SessionEntry::Live(sessao)) => Some(sessao.clone()),
        _ => None,
    });
    if let Some(viva) = viva {
        std::thread::spawn(move || { interaction::wait_claude_ready(&viva,std::time::Duration::from_secs(45)); });
    }
}
