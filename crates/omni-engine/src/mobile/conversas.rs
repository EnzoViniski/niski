//! Conversas: sessão ligada, histórico nativo, resumo/lista, painel, timeline, imagens e arquivos entregues.
//! Fork: saiu do antigo `mobile.rs` na Onda 2.0, sem mudança de lógica.
use super::*;

pub(super) fn current(state: &EngineState, id: &str) -> Result<(omni_core::conversations::Conversation, Option<Arc<LiveSession>>), ApiError> {
    let dir = state.state_file.parent().unwrap();
    let conversation = omni_core::conversations::read_index(dir).into_iter().find(|c|c.id == id)
        .ok_or_else(||error(StatusCode::NOT_FOUND,"Conversa não encontrada"))?;
    let session = linked_session(state,&conversation);
    Ok((conversation,session))
}

pub(super) fn linked_session(state: &EngineState, conversation: &omni_core::conversations::Conversation) -> Option<Arc<LiveSession>> {
    conversation.segments.last().and_then(|segment| {
        let sessions = state.sessions.lock().ok()?;
        let SessionEntry::Live(session) = sessions.get(segment.terminal_session_id.as_deref()?)? else { return None };
        let meta = session.meta.lock().ok()?;
        (meta.conversation_id.as_deref() == Some(conversation.id.as_str()) && meta.profile_id == segment.profile_id && meta.provider.as_deref() == Some(segment.provider.as_str()))
            .then(||session.clone())
    })
}

pub(super) const HISTORY_REFRESH_MS: u64 = 5_000;

pub(super) fn historical_entries(state: &EngineState) -> Vec<omni_core::history::HistoryEntry> {
    let now = now_ms();
    let mut history = state.mobile.history.lock().expect("history poisoned");
    if history.scanned_at_ms != 0 && now.saturating_sub(history.scanned_at_ms) < HISTORY_REFRESH_MS {
        return history.entries.clone();
    }
    let profiles = omni_core::profiles(state.state_file.parent().unwrap());
    let mut entries = omni_core::history::scan("claude", &profiles, &mut history.scan);
    entries.extend(omni_core::history::scan("codex", &profiles, &mut history.scan));
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.updated_at_ms));
    history.entries = entries;
    history.scanned_at_ms = now;
    history.entries.clone()
}

pub(super) fn stable_id(prefix: &str, parts: &[&str]) -> String {
    // FNV-1a explícito: o id precisa continuar igual depois de reiniciar ou atualizar o engine;
    // `DefaultHasher` não promete algoritmo estável entre versões do Rust.
    let mut hash = 0xcbf29ce484222325_u64;
    for part in parts {
        for byte in part.bytes().chain(std::iter::once(0)) {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
    }
    format!("{prefix}-{hash:016x}")
}

pub(super) fn history_id(entry: &omni_core::history::HistoryEntry) -> String {
    stable_id("history", &[&entry.provider,&entry.profile_id,&entry.session_id])
}

pub(super) fn history_project_id(cwd: &str) -> String {
    stable_id("history-project", &[&normalized_path(cwd)])
}

pub(super) fn normalized_path(path: &str) -> String {
    let normalized = path.replace('\\',"/").trim_end_matches('/').to_lowercase();
    normalized.strip_prefix("//wsl$/").map_or(normalized.clone(),|rest|format!("//wsl.localhost/{rest}"))
}

pub(super) fn same_path(left: &str, right: &str) -> bool {
    normalized_path(left) == normalized_path(right)
}

pub(super) fn history_key(provider: &str, session_id: &str) -> String {
    format!("{provider}\0{session_id}")
}

/// Resumo de uma conversa do índice: o que a lista mostra e o que o chat precisa para saber se dá
/// para responder agora. Estado e capacidades saem da sessão viva, quando há.
pub(super) fn resumo(state: &EngineState, c: &omni_core::conversations::Conversation) -> Value {
    let live = linked_session(state,c);
    let capabilities = live.as_ref().map(interaction::capabilities);
    let meta = live.as_ref().map(|s| settle(s));
    let status = meta.as_ref().map(|m| m.state.clone());
    // Identidade do último evento de atenção (fim de turno, pedido de aprovação). A tela usa para
    // saber se já foi vista: muda só quando acontece algo novo — nunca com redesenho da TUI. A
    // sessão entra junto porque o contador recomeça do zero a cada sessão nova.
    let attention = meta.as_ref().map(|m| format!("{}:{}", m.id, m.attention_seq));
    // `mode` sai da tela viva (o rodapé responde na hora); sessão fechada não tem modo.
    let mode = live.as_ref().and_then(comandos::modo_da_sessao); // Fork: confirma também default pelo composer.
    let fast = live.as_ref().and_then(comandos::fast_da_sessao);
    // Tokens do turno em andamento (o "↓ 113 tokens" da linha de status), para a conversa mostrar
    // junto do tempo, como o terminal. Só com o agente trabalhando — parado, o número é velho.
    let tokens = live.as_ref().filter(|_| status.as_ref().is_some_and(|s| matches!(s, omni_protocol::SessionState::Working)))
        .and_then(|s| s.interaction.lock().ok().and_then(|c| interaction::tokens_do_turno(&c.parser.screen().contents())));
    json!({"id":c.id,"title":omni_core::conversations::display_title(c),"project_id":c.project_id,"provider":c.segments.last().map(|s|&s.provider),
        "profile_id":c.segments.last().and_then(|s|s.profile_id.as_ref()),"state":status,"mode":mode,"capabilities":capabilities,
        "attention":attention,"tokens":tokens,"fast":fast,"ganchos":meta.as_ref().map(|m| crate::ganchos::resumo(&m.id)), // Fork (Onda 2)
        "codex_app_server":meta.as_ref().and_then(|m|crate::codex_app_server::resumo(&m.id)), // Fork: contexto do protocolo.
        "notice":meta.as_ref().and_then(|m|m.notice.clone()),"live":live.is_some(),"updated_at_ms":atividade(c,live.as_ref())})
}

/// Último sinal de vida da conversa: saída do terminal, escrita no transcript ou a criação. É o que
/// ordena a lista — a conversa em que você está mexendo fica no topo, como num app de mensagens.
pub(super) fn atividade(c: &omni_core::conversations::Conversation, live: Option<&Arc<LiveSession>>) -> u64 {
    let terminal = live.and_then(|s| s.meta.lock().ok().map(|meta| meta.last_activity_at_ms)).unwrap_or(0);
    let transcript = c.segments.last().and_then(|s| s.transcript_path.as_deref())
        .and_then(|path| omni_protocol::modified_ms(Path::new(path))).unwrap_or(0);
    c.created_at_ms.max(terminal).max(transcript)
}

/// Intervalo mínimo entre buscas de rollout do Codex sem caminho conhecido.
pub(super) const CODEX_DESCOBERTA_MS: u64 = 15_000;

pub(super) fn summaries(state: &EngineState) -> Vec<Value> {
    let dir = state.state_file.parent().unwrap();
    let mut index = omni_core::conversations::read_index(dir);
    let profiles = omni_core::profiles(dir);
    let descobrir = {
        let mut ultima = state.mobile.codex_descoberto_em.lock().expect("codex poisoned");
        let agora = now_ms();
        let vencida = agora.saturating_sub(*ultima) >= CODEX_DESCOBERTA_MS;
        if vencida { *ultima = agora; }
        vencida
    };
    if descobrir {
        // Fork (fix-orquestracao): descoberta grava sob a trava do índice, sobre o índice relido nela.
        let _ = omni_core::indice_de_conversas::mudar_indice(dir, |indice| {
            let mudou = omni_core::conversations::discover_codex_transcripts(indice, &profiles);
            index = indice.clone();
            mudou
        });
    }
    let projects = known_projects(state);
    let history = historical_entries(state);
    let mut known_sessions = HashSet::new();
    let mut known_transcripts = HashSet::new();
    let mut result = Vec::new();
    for c in &index {
        for segment in &c.segments {
            if let Some(session_id) = &segment.external_session_id {
                known_sessions.insert(history_key(&segment.provider,session_id));
            }
            if let Some(path) = &segment.transcript_path {
                known_transcripts.insert(path.replace('\\',"/").to_lowercase());
            }
        }
        let resumo = resumo(state,c);
        result.push((resumo["updated_at_ms"].as_u64().unwrap_or(c.created_at_ms),resumo));
    }
    for entry in history {
        let transcript = entry.path.replace('\\',"/").to_lowercase();
        if known_sessions.contains(&history_key(&entry.provider,&entry.session_id)) || known_transcripts.contains(&transcript) {
            continue;
        }
        let project_id = entry.cwd.as_deref()
            .and_then(|cwd|projects.iter().find(|project|same_path(&project.path,cwd)).map(|project|project.id.clone()))
            .unwrap_or_else(||history_project_id(entry.cwd.as_deref().unwrap_or("")));
        let title = entry.title.clone().or(entry.first_prompt.clone()).unwrap_or_else(||"Conversa anterior".into());
        result.push((entry.updated_at_ms,json!({"id":history_id(&entry),"title":title,"project_id":project_id,"provider":entry.provider,
            "profile_id":entry.profile_id,"state":Value::Null,"mode":Value::Null,"capabilities":Value::Null,"historical":true,
            "live":false,"updated_at_ms":entry.updated_at_ms})));
    }
    result.sort_by_key(|(updated,_)|std::cmp::Reverse(*updated));
    let ocultas = conversas_ocultas(dir);
    result.into_iter().map(|(_,mut summary)| {
        if summary["id"].as_str().is_some_and(|id| ocultas.contains(id)) { summary["hidden"] = json!(true); }
        summary
    }).collect()
}

pub(super) async fn conversations(State(state): State<Arc<EngineState>>) -> Json<Value> {
    Json(tokio::task::spawn_blocking(move || json!(summaries(&state))).await.unwrap_or(json!([])))
}
pub(super) async fn attention(State(state): State<Arc<EngineState>>) -> Json<Value> {
    Json(tokio::task::spawn_blocking(move || json!(summaries(&state).into_iter().filter(|c|
        c["capabilities"]["approve"] == true || c["capabilities"]["prompt"] == true || c["state"] == "approval_required").collect::<Vec<_>>())).await.unwrap_or(json!([])))
}

/// Resposta JSON com `ETag`. Se o cliente já tem exatamente esse conteúdo (`If-None-Match`), volta
/// `304` sem corpo: a lista e a conversa aberta são consultadas a cada poucos segundos e, quase
/// sempre, nada mudou — sem isto cada consulta remontava a tela inteira à toa.
pub(super) fn com_etag(headers: &HeaderMap, corpo: &Value) -> Response {
    let texto = corpo.to_string();
    let etag = format!("\"{:016x}\"", texto.bytes().fold(0xcbf29ce484222325_u64, |h, b| (h ^ u64::from(b)).wrapping_mul(0x100000001b3)));
    if headers.get("if-none-match").and_then(|v| v.to_str().ok()) == Some(etag.as_str()) {
        return (StatusCode::NOT_MODIFIED, [("etag", etag)]).into_response();
    }
    (StatusCode::OK, [("etag", etag), ("content-type", "application/json".into())], texto).into_response()
}

/// Lista de conversas e catálogo numa chamada só. Antes eram três (`/conversas`, `/atencao` e
/// `/projetos`), cada uma montando a lista inteira de novo — o "precisa de você" agora é calculado
/// no cliente a partir da mesma lista.
pub(super) async fn painel(State(state): State<Arc<EngineState>>, headers: HeaderMap) -> Response {
    let corpo = tokio::task::spawn_blocking(move || json!({"conversations":summaries(&state),"catalog":catalogo(&state)}))
        .await.unwrap_or(json!({"conversations":[],"catalog":{"published_at_ms":0,"projects":[],"agents":[],"profiles":[]}}));
    com_etag(&headers, &corpo)
}

/// Sem `cursor` a timeline abre no fim da conversa, como um chat.
#[derive(Deserialize)] pub(super) struct Page { #[serde(default)] cursor: Option<usize> }
/// Itens por página. Ações do agente contam como item, então é maior que uma página só de texto.
pub(super) const TIMELINE_PAGINA: usize = 150;
pub(super) async fn timeline(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Query(page): Query<Page>, headers: HeaderMap) -> Response {
    let resultado = tokio::task::spawn_blocking(move || -> Result<Value, ApiError> {
        let dir = state.state_file.parent().unwrap();
        let profiles = omni_core::profiles(dir);
        if let Ok((conversation,_)) = current(&state,&id) {
            let mut timeline = {
                let mut feeds = state.mobile.feeds.lock().expect("feeds poisoned");
                omni_core::conversations::timeline_cached(&conversation,&profiles,page.cursor,TIMELINE_PAGINA,&mut feeds)
            };
            if let Some(t)=conversation.segments.last().and_then(|s|s.terminal_session_id.as_deref()) { crate::codex_app_server::complementar(t,&mut timeline); } // Fork: plano vivo do RPC.
            let actions: Vec<_> = state.mobile.actions.lock().expect("actions poisoned").iter().filter(|a|a.conversation_id == id).cloned().collect();
            // O estado da conversa vem junto: o chat aberto não precisa recarregar a lista inteira a
            // cada 1,5 s só para saber se o agente terminou.
            return Ok(json!({"timeline":timeline,"actions":actions,"conversation":resumo(&state,&conversation)}));
        }
        let entry = historical_entries(&state).into_iter().find(|entry|history_id(entry) == id)
            .ok_or_else(||error(StatusCode::NOT_FOUND,"Conversa não encontrada"))?;
        let caminho = omni_core::history::validated(Path::new(&entry.path),&entry.provider,&profiles)
            .ok_or_else(||error(StatusCode::NOT_FOUND,"Histórico não encontrado"))?;
        let (itens, estado): (Vec<_>, _) = {
            let mut feeds = state.mobile.feeds.lock().expect("feeds poisoned");
            let itens = feeds.read(&caminho,&entry.provider).map_err(|_|error(StatusCode::NOT_FOUND,"Histórico não encontrado"))?
                .iter().map(|e|(entry.provider.clone(),e.item.clone())).collect();
            // Fork: lista de tarefas e modo de permissão também na conversa do histórico.
            (itens, feeds.estado(&caminho).cloned())
        };
        let mut timeline = omni_core::conversations::page(&id,itens,page.cursor,TIMELINE_PAGINA);
        if let Some(estado) = estado { timeline.tarefas = estado.tarefas(); timeline.modo_de_permissao = estado.modo; }
        Ok(json!({"timeline":timeline,"actions":[]}))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao ler timeline"));
    match resultado {
        Ok(Ok(corpo)) => com_etag(&headers,&corpo),
        Ok(Err(erro)) | Err(erro) => erro.into_response(),
    }
}

/// Imagem colada numa mensagem, lida do transcript (a CLI a guarda lá em base64). Vai em JSON
/// (`{tipo, dados}`) e não como arquivo: o app do PC chega aqui pelo protocolo do engine, que só
/// carrega texto, e a web precisa do cabeçalho de token, que um `<img src>` não manda.
/// Os transcripts de uma conversa aberta ou do histórico, na ordem do timeline.
pub(super) fn fontes_da_conversa(state: &EngineState, id: &str) -> Result<Vec<(std::path::PathBuf, String)>, ApiError> {
    let dir = state.state_file.parent().unwrap();
    let profiles = omni_core::profiles(dir);
    Ok(match current(state,id) {
        Ok((conversation,_)) => omni_core::conversations::fontes(&conversation,&profiles),
        Err(_) => {
            let entry = historical_entries(state).into_iter().find(|entry|history_id(entry) == id)
                .ok_or_else(||error(StatusCode::NOT_FOUND,"Conversa não encontrada"))?;
            let caminho = omni_core::history::validated(Path::new(&entry.path),&entry.provider,&profiles)
                .ok_or_else(||error(StatusCode::NOT_FOUND,"Histórico não encontrado"))?;
            vec![(caminho, entry.provider)]
        }
    })
}

pub(super) async fn imagem_da_mensagem(State(state): State<Arc<EngineState>>, RoutePath((id, n, k)): RoutePath<(String, usize, usize)>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let fontes = fontes_da_conversa(&state,&id)?;
        let mut feeds = state.mobile.feeds.lock().expect("feeds poisoned");
        let (tipo, dados) = omni_core::feed::imagem(&fontes,n,k,&mut feeds).ok_or_else(||error(StatusCode::NOT_FOUND,"Imagem não encontrada"))?;
        Ok(Json(json!({"tipo":tipo,"dados":dados})))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao ler a imagem"))?
}

/// Fork: o documento `k` da mensagem `n` (`omni_core::feed::arquivo`), em base64.
pub(super) async fn arquivo_da_mensagem(State(state): State<Arc<EngineState>>, RoutePath((id, n, k)): RoutePath<(String, usize, usize)>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let fontes = fontes_da_conversa(&state,&id)?;
        let mut feeds = state.mobile.feeds.lock().expect("feeds poisoned");
        let (nome, tipo, dados) = omni_core::feed::arquivo(&fontes,n,k,&mut feeds)
            .ok_or_else(||error(StatusCode::NOT_FOUND,"Arquivo não encontrado ou grande demais para baixar"))?;
        Ok(Json(json!({"nome":nome,"tipo":tipo,"dados":dados})))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao ler o arquivo"))?
}
