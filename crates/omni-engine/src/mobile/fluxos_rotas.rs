//! Rotas dos fluxos de agentes (o executor mora em `crate::fluxos`).
//! Saiu do antigo `mobile.rs` na Onda 2.0, sem mudança de lógica.
use super::*;

/// Fluxos de agentes (o executor mora em `fluxos.rs`). Valem para o app do PC (pelo protocolo local),
/// a web e o celular — aprovar uma etapa de longe é o mesmo POST.
pub(super) async fn fluxos_listar() -> Json<Value> { Json(json!(crate::fluxos::listar())) }

#[derive(Deserialize)] pub(super) struct IniciarFluxo { fluxo: omni_core::fluxos::Fluxo, projeto_id: String, entrada: String }
pub(super) async fn fluxos_iniciar(State(state): State<Arc<EngineState>>, Json(body): Json<IniciarFluxo>) -> Result<(StatusCode,Json<Value>), ApiError> {
    tokio::task::spawn_blocking(move || {
        let projeto = known_projects(&state).into_iter().find(|p| p.id == body.projeto_id && !p.path.is_empty())
            .ok_or_else(||error(StatusCode::NOT_FOUND,"Projeto não encontrado neste PC"))?;
        let execucao = crate::fluxos::iniciar(&state, body.fluxo, projeto.path, projeto.id, body.entrada)
            .map_err(|e| error(StatusCode::BAD_REQUEST,&e.to_string()))?;
        Ok((StatusCode::CREATED,Json(json!(execucao))))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao iniciar o fluxo"))?
}

pub(super) async fn fluxos_obter(RoutePath(id): RoutePath<String>, headers: HeaderMap) -> Response {
    match crate::fluxos::obter(&id) { Some(e) => com_etag(&headers,&json!(e)), None => error(StatusCode::NOT_FOUND,"Execução não encontrada").into_response() }
}

pub(super) async fn fluxos_acao(State(state): State<Arc<EngineState>>, RoutePath((id, acao)): RoutePath<(String, String)>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let r = match acao.as_str() {
            "pausar" => crate::fluxos::pausar(&state,&id),
            "retomar" => crate::fluxos::retomar(&state,&id),
            "cancelar" => crate::fluxos::cancelar(&state,&id),
            _ => return Err(error(StatusCode::NOT_FOUND,"Ação desconhecida")),
        };
        r.map_err(|e| error(StatusCode::CONFLICT,&e.to_string()))?;
        Ok(Json(json!({"ok":true})))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha na ação"))?
}

pub(super) async fn fluxos_historico(State(state): State<Arc<EngineState>>, RoutePath((id, no)): RoutePath<(String, String)>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || crate::fluxos::historico(&state,&id,&no).map(|h| Json(json!(h))).map_err(|e| error(StatusCode::BAD_REQUEST,&e.to_string())))
        .await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao ler o histórico"))?
}

#[derive(Deserialize)] pub(super) struct AprovarFluxo { sim: bool }
pub(super) async fn fluxos_aprovar(State(state): State<Arc<EngineState>>, RoutePath((id, no)): RoutePath<(String, String)>, Json(body): Json<AprovarFluxo>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || crate::fluxos::aprovar(&state,&id,&no,body.sim).map(|_| Json(json!({"ok":true}))).map_err(|e| error(StatusCode::CONFLICT,&e.to_string())))
        .await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao aprovar"))?
}

pub(super) async fn fluxos_conversa(State(state): State<Arc<EngineState>>, RoutePath((id, no)): RoutePath<(String, String)>) -> Result<(StatusCode,Json<Value>), ApiError> {
    tokio::task::spawn_blocking(move || {
        let sessao = crate::fluxos::abrir_conversa(&state,&id,&no).map_err(|e| error(StatusCode::CONFLICT,&e.to_string()))?;
        Ok((StatusCode::CREATED,Json(json!({"session_id":sessao.id,"name":sessao.name}))))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao abrir a conversa"))?
}
