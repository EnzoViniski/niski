//! Fork (vídeo na conversa): o vídeo que o agente gerou (HyperFrames, por exemplo) toca na conversa do
//! celular, em pedaços, sem baixar inteiro.
//!
//! O `<video>` não manda cabeçalho, então não leva o token do aparelho. Por isso dois passos:
//! - `POST /conversas/{id}/videos/{n}/{k}` (com o token, como toda a API): confere que a mensagem `n`
//!   da conversa tem o vídeo `k` (o mesmo `entregaveis` que mostra o cartão) e devolve um bilhete;
//! - `GET /video/{bilhete}` (caminho público em `seguranca.rs`): serve **só aquele arquivo**, com
//!   `Range` (206) para o player pular para qualquer ponto. Bilhete aleatório, de uma conversa, vale 2 h.
//!
//! Nada sai do Mac: o celular busca pelo mesmo servidor da conversa. No app do PC o vídeo toca direto
//! do disco (`convertFileSrc`), sem passar por aqui.
use super::*;
use axum::{body::Body, http::header, response::Response};
use std::sync::LazyLock;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

const VALIDADE_MS: u64 = 2 * 60 * 60 * 1000;
const MAXIMO_DE_BILHETES: usize = 256;

struct Bilhete { caminho: PathBuf, tipo: &'static str, expira_em_ms: u64 }
static BILHETES: LazyLock<Mutex<HashMap<String, Bilhete>>> = LazyLock::new(Default::default);

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new()
        .route("/conversas/{id}/videos/{n}/{k}", post(bilhete))
        .route("/video/{bilhete}", get(servir))
}

fn novo_bilhete(caminho: PathBuf, tipo: &'static str) -> String {
    let mut bytes = [0_u8; 24];
    rand::RngCore::fill_bytes(&mut rand::rng(), &mut bytes);
    let id: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    let agora = now_ms();
    let mut bilhetes = BILHETES.lock().expect("bilhetes poisoned");
    bilhetes.retain(|_, b| b.expira_em_ms > agora);
    if bilhetes.len() >= MAXIMO_DE_BILHETES {
        if let Some(velho) = bilhetes.iter().min_by_key(|(_, b)| b.expira_em_ms).map(|(k, _)| k.clone()) { bilhetes.remove(&velho); }
    }
    bilhetes.insert(id.clone(), Bilhete { caminho, tipo, expira_em_ms: agora + VALIDADE_MS });
    id
}

async fn bilhete(State(state): State<Arc<EngineState>>, RoutePath((id, n, k)): RoutePath<(String, usize, usize)>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let fontes = fontes_da_conversa(&state, &id)?;
        let achado = {
            let mut feeds = state.mobile.feeds.lock().expect("feeds poisoned");
            omni_core::feed::video(&fontes, n, k, &mut feeds)
        };
        let (caminho, tipo) = achado.ok_or_else(|| error(StatusCode::NOT_FOUND, "Vídeo não encontrado nesta mensagem"))?;
        let tamanho = std::fs::metadata(&caminho).map(|m| m.len()).unwrap_or(0);
        let nome = caminho.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let bilhete = novo_bilhete(caminho, tipo);
        Ok(Json(json!({"url": format!("/video/{bilhete}"), "nome": nome, "tipo": tipo, "tamanho": tamanho})))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao preparar o vídeo"))?
}

/// `bytes=a-b`, `bytes=a-` ou `bytes=-n` → (início, fim inclusivo). Fora do arquivo: `None`.
pub(super) fn intervalo(cabecalho: &str, total: u64) -> Option<(u64, u64)> {
    let pedido = cabecalho.trim().strip_prefix("bytes=")?;
    if pedido.contains(',') || total == 0 { return None; }
    let (a, b) = pedido.split_once('-')?;
    let (inicio, fim) = match (a.trim(), b.trim()) {
        ("", n) => { let n: u64 = n.parse().ok()?; (total.saturating_sub(n.min(total)), total - 1) }
        (a, "") => (a.parse().ok()?, total - 1),
        (a, b) => (a.parse().ok()?, b.parse::<u64>().ok()?.min(total - 1)),
    };
    (inicio <= fim && inicio < total).then_some((inicio, fim))
}

async fn servir(RoutePath(id): RoutePath<String>, headers: HeaderMap) -> Response {
    let achado = {
        let bilhetes = BILHETES.lock().expect("bilhetes poisoned");
        bilhetes.get(&id).filter(|b| b.expira_em_ms > now_ms()).map(|b| (b.caminho.clone(), b.tipo))
    };
    let Some((caminho, tipo)) = achado else {
        return error(StatusCode::NOT_FOUND, "Link do vídeo expirou; abra a conversa de novo").into_response();
    };
    let Ok(mut arquivo) = tokio::fs::File::open(&caminho).await else {
        return error(StatusCode::NOT_FOUND, "O vídeo não está mais no computador").into_response();
    };
    let total = arquivo.metadata().await.map(|m| m.len()).unwrap_or(0);
    let nome = caminho.file_name().map(|n| n.to_string_lossy().replace('"', "")).unwrap_or_default();
    let pedido = headers.get(header::RANGE).and_then(|v| v.to_str().ok());
    let (status, inicio, fim) = match pedido {
        None => (StatusCode::OK, 0, total.saturating_sub(1)),
        Some(texto) => match intervalo(texto, total) {
            Some((a, b)) => (StatusCode::PARTIAL_CONTENT, a, b),
            None => return Response::builder().status(StatusCode::RANGE_NOT_SATISFIABLE)
                .header(header::CONTENT_RANGE, format!("bytes */{total}")).body(Body::empty()).unwrap_or_default(),
        },
    };
    let tamanho = if total == 0 { 0 } else { fim - inicio + 1 };
    if inicio > 0 && arquivo.seek(std::io::SeekFrom::Start(inicio)).await.is_err() {
        return error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao ler o vídeo").into_response();
    }
    let corpo = Body::from_stream(tokio_util::io::ReaderStream::new(arquivo.take(tamanho)));
    let mut resposta = Response::builder().status(status)
        .header(header::CONTENT_TYPE, tipo)
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, tamanho)
        .header(header::CONTENT_DISPOSITION, format!("inline; filename=\"{nome}\""));
    if status == StatusCode::PARTIAL_CONTENT {
        resposta = resposta.header(header::CONTENT_RANGE, format!("bytes {inicio}-{fim}/{total}"));
    }
    resposta.body(corpo).unwrap_or_default()
}

