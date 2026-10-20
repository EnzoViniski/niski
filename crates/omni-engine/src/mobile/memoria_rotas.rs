use super::*;
use omni_core::memoria_compartilhada::{self as memoria, Memoria};

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new().route("/memoria", get(ler).post(salvar))
}

async fn ler(State(state): State<Arc<EngineState>>) -> Result<Json<Memoria>, ApiError> {
    let dir = state.state_file.parent().unwrap().to_owned();
    tokio::task::spawn_blocking(move || memoria::ler(&dir).map(Json)
        .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Não foi possível ler a memória. O arquivo foi preservado.")))
        .await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Memória indisponível"))?
}

async fn salvar(State(state): State<Arc<EngineState>>, Json(body): Json<Memoria>) -> Result<Json<Memoria>, ApiError> {
    let dir = state.state_file.parent().unwrap().to_owned();
    tokio::task::spawn_blocking(move || {
        memoria::salvar(&dir, &body).map_err(|erro| error(
            if erro.kind() == std::io::ErrorKind::InvalidInput { StatusCode::BAD_REQUEST } else { StatusCode::INTERNAL_SERVER_ERROR },
            "Não foi possível salvar. Use até 12000 caracteres de texto."))?;
        Ok(Json(body))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Memória indisponível"))?
}
