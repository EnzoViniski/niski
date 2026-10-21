use super::*;
type Desktop = Option<axum::Extension<orquestracao::jarvis::Desktop>>;
fn desktop(acesso: &Desktop) -> Result<(), ApiError> {
    if acesso.is_some() {
        Ok(())
    } else {
        Err(error(
            StatusCode::FORBIDDEN,
            "Configure o Telegram pelo app do computador",
        ))
    }
}
fn falha(e: api::Falha) -> ApiError {
    error(StatusCode::BAD_GATEWAY, e.texto)
}

async fn status(
    State(state): State<Arc<EngineState>>,
    acesso: Desktop,
) -> Result<Json<Value>, ApiError> {
    desktop(&acesso)?;
    Ok(Json(state.mobile.telegram.status()))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Conectar {
    token: String,
}
fn token_valido(token: &str) -> bool {
    let Some((id, segredo)) = token.split_once(':') else {
        return false;
    };
    !id.is_empty()
        && id.len() <= 20
        && id.bytes().all(|b| b.is_ascii_digit())
        && (20..=128).contains(&segredo.len())
        && segredo
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}
async fn conectar(
    State(state): State<Arc<EngineState>>,
    acesso: Desktop,
    Json(p): Json<Conectar>,
) -> Result<Json<Value>, ApiError> {
    desktop(&acesso)?;
    let token = p.token.trim().to_owned();
    if !token_valido(&token) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Cole o token completo que o BotFather forneceu",
        ));
    }
    let _operacao = state.mobile.telegram.operacao.lock().await;
    let cliente = api::Cliente::novo().map_err(falha)?;
    let bot = cliente
        .chamar(&token, "getMe", json!({}))
        .await
        .map_err(falha)?;
    let nome = bot["username"]
        .as_str()
        .filter(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
        .ok_or_else(|| {
            error(
                StatusCode::BAD_GATEWAY,
                "O Telegram não informou o nome do bot",
            )
        })?;
    // Não apaga webhook de outra integração. Um bot exclusivo evita disputar getUpdates.
    let webhook = cliente
        .chamar(&token, "getWebhookInfo", json!({}))
        .await
        .map_err(falha)?;
    if webhook["url"].as_str().is_some_and(|u| !u.is_empty()) {
        return Err(error(
            StatusCode::CONFLICT,
            "Este bot já tem outra integração. Crie um bot exclusivo para o Niski.",
        ));
    }
    let config = Config {
        token,
        bot: nome.into(),
        tipos: TIPOS.iter().map(|s| (*s).into()).collect(),
        ..Default::default()
    };
    salvar(state.state_file.parent().unwrap(), &config).map_err(|_| falha(api::Falha::disco()))?;
    *state
        .mobile
        .telegram
        .dados
        .lock()
        .expect("telegram poisoned") = Dados {
        config,
        pareando: Some(Pareando {
            codigo: uuid::Uuid::new_v4().simple().to_string(),
            inicio: now_ms(),
            expira: now_ms() + 600_000,
            offset: 0,
        }),
        erro: None,
    };
    state.mobile.telegram.geracao.fetch_add(1, Ordering::SeqCst);
    Ok(Json(state.mobile.telegram.status()))
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Preferencias {
    ativo: bool,
    tipos: BTreeSet<String>,
    incluir_titulos: bool,
}
async fn preferencias(
    State(state): State<Arc<EngineState>>,
    acesso: Desktop,
    Json(p): Json<Preferencias>,
) -> Result<Json<Value>, ApiError> {
    desktop(&acesso)?;
    if p.tipos.iter().any(|t| !TIPOS.contains(&t.as_str())) {
        return Err(error(StatusCode::BAD_REQUEST, "Tipo de aviso inválido"));
    }
    let _operacao = state.mobile.telegram.operacao.lock().await;
    let mut d = state
        .mobile
        .telegram
        .dados
        .lock()
        .expect("telegram poisoned");
    if p.ativo && d.config.chat.is_none() {
        return Err(error(StatusCode::CONFLICT, "Conecte o Telegram primeiro"));
    }
    let mut config = d.config.clone();
    config.ativo = p.ativo;
    config.tipos = p.tipos;
    config.incluir_titulos = p.incluir_titulos;
    salvar(state.state_file.parent().unwrap(), &config).map_err(|_| falha(api::Falha::disco()))?;
    d.config = config;
    state.mobile.telegram.geracao.fetch_add(1, Ordering::SeqCst);
    drop(d);
    Ok(Json(state.mobile.telegram.status()))
}
async fn desconectar(
    State(state): State<Arc<EngineState>>,
    acesso: Desktop,
) -> Result<Json<Value>, ApiError> {
    desktop(&acesso)?;
    let _operacao = state.mobile.telegram.operacao.lock().await;
    salvar(state.state_file.parent().unwrap(), &Config::default())
        .map_err(|_| falha(api::Falha::disco()))?;
    *state
        .mobile
        .telegram
        .dados
        .lock()
        .expect("telegram poisoned") = Dados::default();
    state.mobile.telegram.geracao.fetch_add(1, Ordering::SeqCst);
    Ok(Json(state.mobile.telegram.status()))
}
async fn testar(
    State(state): State<Arc<EngineState>>,
    acesso: Desktop,
) -> Result<Json<Value>, ApiError> {
    desktop(&acesso)?;
    let _operacao = state.mobile.telegram.operacao.lock().await;
    let config = state
        .mobile
        .telegram
        .dados
        .lock()
        .expect("telegram poisoned")
        .config
        .clone();
    if config.chat.is_none() {
        return Err(error(StatusCode::CONFLICT, "Conecte o Telegram primeiro"));
    }
    let cliente = api::Cliente::novo().map_err(falha)?;
    cliente.chamar(&config.token,"sendMessage",json!({"chat_id":config.chat, "text":format!("{}: Telegram conectado. Você receberá os avisos escolhidos nas configurações. Para responder aos agentes, abra a conversa pelo acesso remoto com o Tailscale conectado.", omni_protocol::NOME_DO_PRODUTO)})).await.map_err(falha)?;
    Ok(Json(json!({"ok":true})))
}
/// Fork: manda um exemplo de cada caso de aviso (`exemplos.rs`).
async fn exemplos(
    State(state): State<Arc<EngineState>>,
    acesso: Desktop,
) -> Result<Json<Value>, ApiError> {
    desktop(&acesso)?;
    let _operacao = state.mobile.telegram.operacao.lock().await;
    let config = state.mobile.telegram.dados.lock().expect("telegram poisoned").config.clone();
    if config.chat.is_none() {
        return Err(error(StatusCode::CONFLICT, "Conecte o Telegram primeiro"));
    }
    let cliente = api::Cliente::novo().map_err(falha)?;
    let enviados = super::exemplos::enviar(&state, &cliente, &config, std::time::Duration::from_millis(700)).await.map_err(falha)?;
    Ok(Json(json!({"ok":true, "enviados":enviados})))
}
pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new()
        .route("/telegram", get(status).post(preferencias))
        .route("/telegram/conectar", post(conectar))
        .route("/telegram/desconectar", post(desconectar))
        .route("/telegram/testar", post(testar))
        .route("/telegram/exemplos", post(exemplos))
}
