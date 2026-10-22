//! Fork (desempenho, 2026-10-22): só o resumo de uma conversa — estado, capacidades, título, tokens.
//!
//! A faixa da aba de agente (conta/modelo/esforço) e o topo da Conversa no PC liam a cada 3 s a
//! timeline inteira (150 itens com textos e diffs) só para pegar o `conversation` que vem junto,
//! enquanto a própria conversa já lia a mesma timeline no ritmo dela. Esta rota devolve só o
//! `resumo`, com `ETag`: parado, a consulta volta `304` sem corpo.
use super::*;

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new().route("/conversas/{id}/resumo", get(resumo_da_conversa))
}

async fn resumo_da_conversa(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap) -> Response {
    let resultado = tokio::task::spawn_blocking(move || current(&state, &id).map(|(conversa, _)| resumo(&state, &conversa)))
        .await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao ler a conversa"));
    match resultado {
        Ok(Ok(corpo)) => com_etag(&headers, &corpo),
        Ok(Err(erro)) | Err(erro) => erro.into_response(),
    }
}
