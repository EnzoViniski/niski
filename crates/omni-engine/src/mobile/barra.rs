//! Fork: `/` no campo da Conversa. Lista o que a conversa oferece (comandos da CLI que o chat roda,
//! skills e comandos personalizados) e digita na CLI `/skill argumento` do Claude — só se o nome
//! estiver na lista desta conversa agora. Comandos da CLI continuam pela lista fechada (`comandos.rs`).
use super::*;
use omni_core::comandos_da_barra as barra;

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new().route("/conversas/{id}/barra", get(listar).post(enviar))
}

fn casa() -> Option<PathBuf> {
    std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")).map(PathBuf::from)
}

/// Itens da barra para a conta e a pasta da última sessão da conversa.
pub(super) fn itens_da_conversa(state: &EngineState, id: &str) -> Result<(String, Vec<barra::ItemDaBarra>), ApiError> {
    let segmento = ultima_sessao(state, id)?;
    let perfis = omni_core::profiles(state.state_file.parent().unwrap());
    let config = perfis.iter().find(|p| Some(&p.id) == segmento.profile_id.as_ref()).map(|p| PathBuf::from(&p.config_dir));
    let cwd = pasta_da_conversa(state, id).map(PathBuf::from).filter(|p| p.is_dir());
    let itens = barra::listar(&segmento.provider, config.as_deref(), cwd.as_deref(), casa().as_deref());
    Ok((segmento.provider, itens))
}

async fn listar(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let (provider, itens) = itens_da_conversa(&state, &id)?;
        Ok(Json(json!({ "provider": provider, "itens": itens })))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao listar comandos"))?
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)]
struct EnviarBody { texto: String }

async fn enviar(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, Json(body): Json<EnviarBody>) -> Result<(StatusCode, Json<Value>), ApiError> {
    let (provider, itens) = itens_da_conversa(&state, &id)?;
    if provider != "claude" { return Err(error(StatusCode::BAD_REQUEST, "Skills por / só no Claude Code; nas outras CLIs a skill vai no texto")); }
    let texto = barra::texto_para_cli(&itens, &body.texto).map_err(|e| error(StatusCode::BAD_REQUEST, e))?;
    enqueue_async(state, id, headers, ActionKind::Comando(comandos::Pedido::Barra(texto))).await
}

#[cfg(test)]
#[path = "barra_testes.rs"]
mod testes;
