//! Fork (Sobre mim): ler e gravar as áreas, os ajustes e as sugestões dos agentes. Serve o desktop
//! (`EngineRequest::Api`) e o celular pareado. Erros respondem só mensagens fixas: o texto do
//! usuário nunca volta em erro nem vai para o log.
use super::*;
use omni_core::sobre_mim::{self, Ajustes, SobreMim};

/// Uma área cheia (10 mil caracteres) passa dos 32 KB do limite global em UTF-8 com acentos.
const CORPO_MAXIMO: usize = 128 * 1024;

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new()
        .route("/sobre-mim", get(ler))
        .route("/sobre-mim/areas/{id}", post(salvar_area).layer(DefaultBodyLimit::max(CORPO_MAXIMO)))
        .route("/sobre-mim/ajustes", post(salvar_ajustes))
        .route("/sobre-mim/propostas/aceitar", post(aceitar))
        .route("/sobre-mim/propostas/descartar", post(descartar))
}

#[derive(serde::Deserialize)]
struct Texto { texto: String }

#[derive(serde::Deserialize)]
struct Aceite { linha: String, area: String, texto: String }

#[derive(serde::Deserialize)]
struct Descarte { linha: String }

fn status(erro: &std::io::Error) -> StatusCode {
    match erro.kind() {
        std::io::ErrorKind::InvalidInput => StatusCode::BAD_REQUEST,
        std::io::ErrorKind::NotFound => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

/// Roda `acao` fora do runtime e devolve o estado inteiro relido (a tela troca tudo de uma vez).
async fn com_estado(state: Arc<EngineState>, acao: impl FnOnce(&std::path::Path) -> std::io::Result<()> + Send + 'static)
    -> Result<Json<SobreMim>, ApiError> {
    let dir = state.state_file.parent().unwrap().to_owned();
    tokio::task::spawn_blocking(move || {
        acao(&dir).map_err(|erro| {
            // Mensagens daqui são fixas (nunca citam o texto); as de dados ilegíveis viram genéricas.
            let mensagem = if matches!(erro.kind(), std::io::ErrorKind::InvalidInput | std::io::ErrorKind::NotFound | std::io::ErrorKind::InvalidData) {
                erro.to_string()
            } else { "Não foi possível gravar o Sobre mim.".into() };
            error(status(&erro), &mensagem)
        })?;
        sobre_mim::ler(&dir).map(Json).map_err(|erro| error(StatusCode::INTERNAL_SERVER_ERROR,
            &if erro.kind() == std::io::ErrorKind::InvalidData { erro.to_string() } else { "Não foi possível ler o Sobre mim.".into() }))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Sobre mim indisponível"))?
}

async fn ler(State(state): State<Arc<EngineState>>) -> Result<Json<SobreMim>, ApiError> {
    com_estado(state, |_| Ok(())).await
}

async fn salvar_area(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Json(corpo): Json<Texto>)
    -> Result<Json<SobreMim>, ApiError> {
    com_estado(state, move |dir| sobre_mim::salvar_area(dir, &id, &corpo.texto)).await
}

async fn salvar_ajustes(State(state): State<Arc<EngineState>>, Json(ajustes): Json<Ajustes>) -> Result<Json<SobreMim>, ApiError> {
    com_estado(state, move |dir| sobre_mim::salvar_ajustes(dir, &ajustes)).await
}

async fn aceitar(State(state): State<Arc<EngineState>>, Json(corpo): Json<Aceite>) -> Result<Json<SobreMim>, ApiError> {
    com_estado(state, move |dir| sobre_mim::aceitar(dir, &corpo.linha, &corpo.area, &corpo.texto)).await
}

async fn descartar(State(state): State<Arc<EngineState>>, Json(corpo): Json<Descarte>) -> Result<Json<SobreMim>, ApiError> {
    com_estado(state, move |dir| sobre_mim::descartar(dir, &corpo.linha)).await
}
