//! Fork (retomar-sempre): rotas para retomar uma conversa sem beco sem saída.
//!
//! - `POST /retomada/preparar`: antes de o PC abrir `claude --resume`/`codex resume` (Histórico), diz
//!   se a conversa já está viva numa aba (a tela foca a aba em vez de abrir outra), se a pasta ainda
//!   existe (senão sugere o repositório principal) e em que conta está o transcript; com
//!   `preparar`, copia o transcript para onde a CLI da conta escolhida vai procurar.
//! - `POST /retomada/continuar-nova`: a CLI não retoma (conversa apagada, formato que ela não lê
//!   mais): abre uma conversa nova na pasta e manda, assim que a CLI fica pronta, as últimas
//!   mensagens da antiga como contexto.
//! - `POST /retomada/mudar-pasta`: aba cuja pasta sumiu (worktree apagada) reabre em outra pasta,
//!   com o transcript levado para lá.
//!
//! Engine antigo não tem estas rotas (404): a interface cai no caminho de antes.
use super::*;
use omni_core::achar_transcript;

/// Mensagens que entram no contexto da conversa nova e o teto de caracteres dele (o texto vai para a
/// CLI numa colagem só).
const MENSAGENS_DE_CONTEXTO: usize = 40;
const CONTEXTO_MAX: usize = 12_000;

fn id_valido(id: &str) -> bool {
    !id.is_empty() && id.len() <= 128 && id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

fn pasta_valida(cwd: &str) -> bool { !cwd.is_empty() && cwd.len() <= 4096 && !cwd.chars().any(char::is_control) }

/// O PID da PTY é do shell, que continua vivo depois de `/exit`. Só o agente rodando permite
/// focar aquela sessão. A breve janela de abertura evita duplicar o agente antes de o comando
/// inicial chegar ao shell; app-server tem ciclo próprio, sem CLI na PTY.
fn agente_rodando(meta: &TerminalSession) -> bool {
    if meta.pid.is_none() || matches!(meta.state, SessionState::Stopped | SessionState::Crashed | SessionState::Orphan) {
        return false;
    }
    if codex_app_server::ativo(&meta.id) { return true; }
    if meta.state == SessionState::Working && now_ms().saturating_sub(meta.last_activity_at_ms) <= ANSWERED_IDLE_MS {
        return true;
    }
    meta.provider.as_deref().is_some_and(|provider| interaction::provider_running(meta.pid, provider))
}

/// Sessão com agente rodando desta conversa nativa: pela própria sessão ou pelo índice.
pub(super) fn sessao_viva(state: &EngineState, provider: &str, externo: &str) -> Option<TerminalSession> {
    let indice = omni_core::conversations::read_index(state.state_file.parent()?);
    let pelo_indice: Vec<String> = indice.iter().flat_map(|c| c.segments.iter())
        .filter(|s| s.provider == provider && s.external_session_id.as_deref() == Some(externo))
        .filter_map(|s| s.terminal_session_id.clone()).collect();
    let sessoes = state.sessions.lock().ok()?;
    sessoes.values().find_map(|entrada| {
        let SessionEntry::Live(sessao) = entrada else { return None };
        let meta = sessao.meta.lock().ok()?.clone();
        let mesma = meta.provider.as_deref() == Some(provider)
            && (meta.external_session_id.as_deref() == Some(externo) || pelo_indice.contains(&meta.id));
        (mesma && agente_rodando(&meta)).then_some(meta)
    })
}

fn achar(perfis: &[omni_core::Profile], provider: &str, conta: Option<&str>, cwd: &str, id: &str) -> Option<achar_transcript::Achado> {
    match provider {
        "claude" => achar_transcript::achar_claude(perfis, conta, cwd, id),
        "codex" => achar_transcript::achar_codex(perfis, conta, id),
        _ => None,
    }
}

fn pastas_conhecidas(state: &EngineState) -> Vec<String> { known_projects(state).into_iter().map(|p| p.path).collect() }

/// Histórico de um modelo local não deve voltar implicitamente para o provedor online da conta.
fn modelo_local(state: &EngineState, provider: &str, externo: &str) -> Option<Value> {
    if provider != "codex" { return None }
    let dir = state.state_file.parent()?;
    let terminais: Vec<_> = omni_core::conversations::read_index(dir).iter().flat_map(|c| c.segments.iter())
        .filter(|s| s.provider == provider && s.external_session_id.as_deref() == Some(externo))
        .filter_map(|s| s.terminal_session_id.clone()).collect();
    let sessoes = state.sessions.lock().ok()?;
    let meta = sessoes.values().map(SessionEntry::metadata)
        .filter(|meta| meta.provider.as_deref() == Some(provider) && (meta.external_session_id.as_deref() == Some(externo) || terminais.contains(&meta.id)))
        .max_by_key(|meta| meta.last_activity_at_ms)?;
        let comando = meta.initial_command.as_deref()?;
        let partes: Vec<_> = comando.split_whitespace().collect();
        if !partes.contains(&"--oss") { return None }
        let valor = |flag: &str| partes.windows(2).find(|p| p[0] == flag).map(|p| p[1]);
        let provedor = valor("--local-provider")?;
        let modelo = valor("--model").or_else(|| valor("-m"))?;
        if !matches!(provedor, "ollama" | "lmstudio") || modelo.len() > 200 || !modelo.chars().all(|c| c.is_ascii_alphanumeric() || "._:/+@-".contains(c)) { return None }
        Some(json!({"provedor":provedor,"modelo":modelo}))
}

/// 409 com código: a tela oferece a pasta sugerida em vez de só mostrar o erro.
pub(super) fn erro_pasta_sumiu(state: &EngineState, cwd: &str) -> ApiError {
    (StatusCode::CONFLICT, Json(json!({
        "error": format!("A pasta {cwd} não existe mais (worktree apagada?)."),
        "codigo": "pasta_sumiu",
        "pasta_principal": achar_transcript::pasta_principal(cwd, &pastas_conhecidas(state)),
    })))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PrepararBody {
    provider: String,
    session_id: String,
    #[serde(default)] profile_id: Option<String>,
    #[serde(default)] cwd: Option<String>,
    #[serde(default)] preparar: bool,
}

pub(super) fn preparar_retomada(state: &EngineState, body: &PrepararBody) -> Result<Value, ApiError> {
    if !id_valido(&body.session_id) || body.cwd.as_deref().is_some_and(|c| !pasta_valida(c)) {
        return Err(error(StatusCode::BAD_REQUEST, "Pedido inválido"));
    }
    let dir = state.state_file.parent().unwrap();
    let viva = sessao_viva(state, &body.provider, &body.session_id);
    let cwd = body.cwd.as_deref().unwrap_or("");
    let pasta_existe = !cwd.is_empty() && Path::new(cwd).is_dir();
    let pasta_principal = (!cwd.is_empty() && !pasta_existe).then(|| achar_transcript::pasta_principal(cwd, &pastas_conhecidas(state))).flatten();
    let perfis = omni_core::arquivo_de_conversas::com_arquivo(dir, &omni_core::profiles(dir));
    let achado = achar(&perfis, &body.provider, body.profile_id.as_deref(), cwd, &body.session_id);
    let mut conta = achado.as_ref().map(|a| a.perfil.id.clone());
    let mut pronto = false;
    if let (true, true, None, Some(achado)) = (body.preparar, pasta_existe, &viva, &achado) {
        let alvo = body.profile_id.as_ref()
            .and_then(|id| perfis.iter().find(|p| &p.id == id && p.provider == body.provider)).cloned()
            .or_else(|| perfis.iter().find(|p| p.id == achado.perfil.id && p.provider == body.provider).cloned())
            .unwrap_or_else(|| achado.perfil.clone());
        match achar_transcript::colocar_onde_a_cli_procura(achado, &alvo, cwd, &body.session_id) {
            Ok(_) => { pronto = true; conta = Some(alvo.id.clone()); }
            Err(erro) => registro::aviso("retomada_nao_preparada", &[("provider", &body.provider), ("erro", &erro.to_string())]),
        }
    }
    Ok(json!({
        "viva": viva.map(|m| json!({"terminal_session_id": m.id, "conversation_id": m.conversation_id, "project_id": m.project_id})),
        "pasta_existe": pasta_existe,
        "pasta_principal": pasta_principal,
        "transcript": achado.is_some(),
        "conta": conta,
        "pronto": pronto,
        "modelo_local": modelo_local(state, &body.provider, &body.session_id),
    }))
}

pub(super) async fn preparar(State(state): State<Arc<EngineState>>, Json(body): Json<PrepararBody>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || preparar_retomada(&state, &body).map(Json))
        .await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao preparar a retomada"))?
}

/// Primeira mensagem da conversa nova: as últimas falas da antiga, cortadas no teto (fica o fim, que
/// é o mais recente).
pub(super) fn montar_contexto(titulo: Option<&str>, mensagens: &[omni_core::history::HistoryMessage], total: usize) -> String {
    let mut corpo = String::new();
    for mensagem in mensagens.iter().rev().take(MENSAGENS_DE_CONTEXTO).collect::<Vec<_>>().into_iter().rev() {
        let quem = if mensagem.role == "user" { "Usuário" } else { "Agente" };
        corpo.push_str(&format!("\n[{quem}]\n{}\n", mensagem.text.trim()));
    }
    if corpo.chars().count() > CONTEXTO_MAX {
        let pular = corpo.chars().count() - CONTEXTO_MAX;
        corpo = format!("\n[…início cortado…]{}", corpo.chars().skip(pular).collect::<String>());
    }
    let mostradas = mensagens.len().min(MENSAGENS_DE_CONTEXTO);
    format!(
        "Esta é a continuação de uma conversa anterior{} que a CLI não conseguiu retomar. Abaixo estão as últimas {mostradas} de {total} mensagens dela, como contexto. Leia, não execute nada agora e responda só com um resumo curto de onde a conversa parou; depois espere a minha próxima mensagem.\n\n--- conversa anterior ---{corpo}--- fim da conversa anterior ---",
        titulo.map(|t| format!(" (\"{}\")", t.chars().take(80).collect::<String>())).unwrap_or_default(),
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ContinuarBody {
    provider: String,
    session_id: String,
    cwd: String,
    #[serde(default)] profile_id: Option<String>,
    #[serde(default)] project_id: Option<String>,
    #[serde(default)] titulo: Option<String>,
}

pub(super) fn continuar_em_conversa_nova(state: &Arc<EngineState>, body: &ContinuarBody) -> Result<Value, ApiError> {
    if !id_valido(&body.session_id) || !pasta_valida(&body.cwd) || body.project_id.as_deref().is_some_and(|p| !id_valido(p))
        || body.titulo.as_deref().is_some_and(|t| t.len() > 200) {
        return Err(error(StatusCode::BAD_REQUEST, "Pedido inválido"));
    }
    if !matches!(body.provider.as_str(), "claude" | "codex") {
        return Err(error(StatusCode::BAD_REQUEST, "Só Claude e Codex continuam com o histórico como contexto"));
    }
    if !Path::new(&body.cwd).is_dir() { return Err(erro_pasta_sumiu(state, &body.cwd)); }
    limite_de_sessoes::conferir(state)?;
    let dir = state.state_file.parent().unwrap();
    let nativos = omni_core::profiles(dir);
    let perfis = omni_core::arquivo_de_conversas::com_arquivo(dir, &nativos);
    let achado = achar(&perfis, &body.provider, body.profile_id.as_deref(), &body.cwd, &body.session_id);
    let lido = achado.as_ref().and_then(|a| omni_core::history::transcript(&a.caminho, &body.provider, &perfis));
    let contexto = lido.as_ref().filter(|t| !t.messages.is_empty())
        .map(|t| montar_contexto(body.titulo.as_deref(), &t.messages, t.total));

    let perfil = body.profile_id.as_ref().and_then(|id| nativos.iter().find(|p| &p.id == id && p.provider == body.provider))
        .or_else(|| achado.as_ref().and_then(|a| nativos.iter().find(|p| p.id == a.perfil.id && p.provider == body.provider)))
        .or_else(|| nativos.iter().find(|p| p.provider == body.provider && p.builtin)).cloned();
    let base = state.mobile.workspace.lock().map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "workspace poisoned"))?
        .agents.iter().find(|a| a.id == body.provider).map(|a| a.command.clone()).unwrap_or_else(|| body.provider.clone());
    let (command, external_session_id, transcript_path) = if body.provider == "claude" {
        let novo = uuid::Uuid::new_v4().to_string();
        let caminho = perfil.as_ref().map(|p| omni_core::conversations::claude_transcript_path(Path::new(&p.config_dir), &body.cwd, &novo)
            .to_string_lossy().into_owned());
        (format!("{base} --session-id {novo}"), Some(novo), caminho)
    } else { (base, None, None) };
    let project_id = body.project_id.clone()
        .or_else(|| known_projects(state).into_iter().find(|p| same_path(&p.path, &body.cwd)).map(|p| p.id))
        .unwrap_or_else(|| history_project_id(&body.cwd));
    let conversa = format!("conv-{}", uuid::Uuid::new_v4());
    let titulo = format!("{} (continuação)", body.titulo.as_deref().unwrap_or("Conversa").chars().take(80).collect::<String>());
    let plano = SpawnPlan {
        project_id, cwd: body.cwd.clone(), title: titulo, titulo_do_usuario: true, provider: body.provider.clone(),
        profile_id: perfil.as_ref().map(|p| p.id.clone()), env: perfil.as_ref().map(omni_core::env_for).unwrap_or_default(),
        command, external_session_id, transcript_path, conversation_id: Some(conversa.clone()),
    };
    let acao = Action { id: state.next_id(), conversation_id: String::new(), session_id: String::new(), state: "executing".into(),
        created_at_ms: now_ms(), error: None, key: String::new(), revision: String::new(), kind: ActionKind::Spawn(plano.clone()) };
    spawn_from_mobile(state, &acao, &plano)
        .map_err(|e| error(StatusCode::CONFLICT, &format!("Não consegui abrir a conversa nova: {e}")))?;
    let terminal = omni_core::conversations::read_index(dir).into_iter().find(|c| c.id == conversa)
        .and_then(|c| c.segments.last().and_then(|s| s.terminal_session_id.clone()));
    registro::info("retomada_continuou_em_nova", &[("provider", &body.provider), ("contexto", if contexto.is_some() { "sim" } else { "nao" })]);
    if let Some(texto) = contexto.clone() {
        let (estado, id) = (state.clone(), conversa.clone());
        std::thread::spawn(move || mandar_quando_pronta(&estado, &id, &texto, std::time::Duration::from_secs(180)));
    }
    Ok(json!({"conversation_id": conversa, "terminal_session_id": terminal, "contexto": contexto.is_some()}))
}

pub(super) async fn continuar_nova(State(state): State<Arc<EngineState>>, Json(body): Json<ContinuarBody>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || continuar_em_conversa_nova(&state, &body).map(Json))
        .await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao abrir a conversa nova"))?
}

/// Espera a CLI chegar no campo vazio e manda o contexto pela fila de ações (a mesma do chat).
fn mandar_quando_pronta(state: &Arc<EngineState>, conversa: &str, texto: &str, prazo: std::time::Duration) {
    let limite = std::time::Instant::now() + prazo;
    while std::time::Instant::now() < limite {
        if let Ok((_, Some(sessao))) = current(state, conversa) {
            let cap = interaction::capabilities(&sessao);
            if cap.prompt {
                let mut headers = HeaderMap::new();
                headers.insert("idempotency-key", format!("contexto-{conversa}").parse().expect("chave"));
                if let Ok(revisao) = cap.revision.parse() { headers.insert("if-match", revisao); }
                if enqueue(state, conversa, &headers, ActionKind::Prompt(texto.to_owned())).is_ok() {
                    registro::info("retomada_contexto_enviado", &[("conversa", conversa)]);
                    return;
                }
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(500));
    }
    registro::aviso("retomada_contexto_nao_enviado", &[("conversa", conversa)]);
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MudarPastaBody { terminal_session_id: String, cwd: String }

/// Aba cuja pasta sumiu: troca a pasta da sessão (e da conversa), leva o transcript Claude para a
/// pasta nova e reabre com `--resume`.
pub(super) fn reabrir_em_outra_pasta(state: &Arc<EngineState>, body: &MudarPastaBody) -> Result<Value, ApiError> {
    if !pasta_valida(&body.cwd) || !Path::new(&body.cwd).is_dir() {
        return Err(error(StatusCode::BAD_REQUEST, "Escolha uma pasta que exista"));
    }
    let dir = state.state_file.parent().unwrap().to_path_buf();
    let meta = {
        let mut sessoes = state.sessions.lock().map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "sessions poisoned"))?;
        match sessoes.get_mut(&body.terminal_session_id).ok_or_else(|| error(StatusCode::NOT_FOUND, "Sessão não encontrada"))? {
            SessionEntry::Historical(meta) => { meta.cwd = body.cwd.clone(); meta.clone() }
            SessionEntry::Live(sessao) => {
                let mut meta = sessao.meta.lock().map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "metadata poisoned"))?;
                if meta.pid.is_some() && Path::new(&meta.cwd).is_dir() {
                    return Err(error(StatusCode::CONFLICT, "Esta sessão ainda está rodando na pasta dela"));
                }
                meta.cwd = body.cwd.clone();
                meta.clone()
            }
        }
    };
    let externo = meta.external_session_id.clone();
    let mut caminho_novo = None;
    if let (Some("claude"), Some(id)) = (meta.provider.as_deref(), externo.as_deref()) {
        let perfis = omni_core::arquivo_de_conversas::com_arquivo(&dir, &omni_core::profiles(&dir));
        if crate::retomada::transcript_claude_pronto(&meta, id, &perfis) {
            caminho_novo = crate::retomada::pasta_da_conta(&meta, "CLAUDE_CONFIG_DIR", ".claude")
                .map(|conta| omni_core::conversations::claude_transcript_path(&conta, &body.cwd, id).to_string_lossy().into_owned());
        }
    }
    let _ = omni_core::indice_de_conversas::mudar_indice(&dir, |indice| {
        let Some(conversa) = indice.iter_mut().find(|c| c.segments.iter().any(|s| s.terminal_session_id.as_deref() == Some(meta.id.as_str()))) else { return false };
        conversa.cwd = body.cwd.clone();
        if let (Some(caminho), Some(ultimo)) = (&caminho_novo, conversa.segments.last_mut()) { ultimo.transcript_path = Some(caminho.clone()); }
        true
    });
    let _ = state.persist();
    registro::info("retomada_pasta_trocada", &[("sessao", &meta.id), ("provider", meta.provider.as_deref().unwrap_or("-"))]);
    match restart_session(state, &meta.id) {
        Ok(EngineResponse::Session { session }) => Ok(json!({"session": session})),
        Ok(_) => Ok(json!({"session": null})),
        Err(e) => Err(error(StatusCode::CONFLICT, &format!("Não consegui reabrir: {e}"))),
    }
}

pub(super) async fn mudar_pasta(State(state): State<Arc<EngineState>>, Json(body): Json<MudarPastaBody>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || reabrir_em_outra_pasta(&state, &body).map(Json))
        .await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao trocar a pasta"))?
}

fn sem_ajustes(ajustes: &Ajustes) -> bool { ajustes.profile_id.is_none() && ajustes.model.is_none() && ajustes.effort.is_none() }

/// "Retomar" numa conversa que já tem sessão: viva e rodando = nada a abrir (a tela vai para ela);
/// processo que saiu = reabre a mesma sessão (com os ajustes, se houver); aba que o engine anterior
/// deixou (histórica) = reabre com o mesmo id, como ao clicar na aba. `None` = não há sessão: segue o
/// caminho normal (sessão nova com `--resume`).
pub(super) fn reabrir_se_ja_existe(state: &Arc<EngineState>, id: &str, ajustes: &Ajustes) -> Result<Option<(StatusCode, Json<Value>)>, ApiError> {
    let Ok((conversa, viva)) = current(state, id) else { return Ok(None) };
    let resposta = |chave: &str, terminal: &str| Some((StatusCode::OK, Json(json!({"conversation_id": id, chave: true, "terminal_session_id": terminal}))));
    let falha = |e: anyhow::Error| {
        let texto = e.to_string();
        match texto.strip_prefix(crate::retomada::PASTA_SUMIU) {
            Some(_) => erro_pasta_sumiu(state, &conversa.cwd),
            None => error(StatusCode::CONFLICT, &format!("Não consegui reabrir: {texto}")),
        }
    };
    if let Some(sessao) = viva {
        let meta = sessao.meta.lock().map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "metadata poisoned"))?.clone();
        if agente_rodando(&meta) {
            return Ok(resposta("ja_aberta", &meta.id));
        }
        if sem_ajustes(ajustes) { restart_session(state, &meta.id).map_err(falha)?; } else { reabrir_com(state, id, ajustes).map_err(falha)?; }
        registro::info("retomada_reabriu_sessao", &[("sessao", &meta.id), ("motivo", "processo_saiu")]);
        return Ok(resposta("reaberta", &meta.id));
    }
    if !sem_ajustes(ajustes) { return Ok(None); }
    let Some(terminal) = conversa.segments.last().and_then(|s| s.terminal_session_id.clone()) else { return Ok(None) };
    let historica = state.sessions.lock().ok().is_some_and(|sessoes| matches!(sessoes.get(&terminal),
        Some(SessionEntry::Historical(meta)) if meta.conversation_id.as_deref().is_none_or(|c| c == id)));
    if !historica { return Ok(None); }
    restart_session(state, &terminal).map_err(falha)?;
    registro::info("retomada_reabriu_sessao", &[("sessao", &terminal), ("motivo", "historica")]);
    Ok(resposta("reaberta", &terminal))
}

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new()
        .route("/retomada/preparar", post(preparar))
        .route("/retomada/continuar-nova", post(continuar_nova))
        .route("/retomada/mudar-pasta", post(mudar_pasta))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(role: &str, text: &str) -> omni_core::history::HistoryMessage {
        omni_core::history::HistoryMessage { role: role.into(), text: text.into(), timestamp: None }
    }

    #[test]
    fn contexto_tem_as_ultimas_mensagens_e_respeita_o_teto() {
        let poucas = [msg("user", "corrige o login"), msg("assistant", "corrigido em auth.ts")];
        let texto = montar_contexto(Some("Login"), &poucas, 2);
        assert!(texto.contains("(\"Login\")") && texto.contains("[Usuário]\ncorrige o login") && texto.contains("[Agente]\ncorrigido em auth.ts"), "{texto}");
        assert!(texto.contains("últimas 2 de 2"));

        let muitas: Vec<_> = (0..100).map(|i| msg(if i % 2 == 0 { "user" } else { "assistant" }, &format!("m{i} {}", "x".repeat(500)))).collect();
        let longo = montar_contexto(None, &muitas, 300);
        assert!(longo.contains("m99 ") && !longo.contains("m10 "), "fica o fim da conversa");
        assert!(longo.contains("[…início cortado…]") && longo.chars().count() < CONTEXTO_MAX + 600);
        assert!(longo.contains("últimas 40 de 300"));
    }
}
