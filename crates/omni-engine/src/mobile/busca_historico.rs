//! Rota da paleta. Usa snapshots do índice/cache; não refaz a varredura das contas por tecla.
use super::*;
use omni_core::busca_historico::{buscar, Fonte, Resultado};
use std::time::{Duration, Instant};

static TRAVA: Mutex<()> = Mutex::new(());
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Pedido {
    texto: String,
    #[serde(default = "limite_padrao")] limite: usize,
    #[serde(default = "tempo_padrao")] tempo_ms: u64,
}
fn limite_padrao() -> usize { 40 }
fn tempo_padrao() -> u64 { 750 }

pub(super) async fn rota(State(state): State<Arc<EngineState>>, Json(pedido): Json<Pedido>) -> Result<Json<Resultado>, ApiError> {
    if pedido.texto.trim().is_empty() || pedido.texto.chars().count() > 200 {
        return Err(error(StatusCode::BAD_REQUEST, "Digite de 1 a 200 caracteres"));
    }
    let tempo = Duration::from_millis(pedido.tempo_ms.clamp(1, 1500));
    let tarefa = tokio::task::spawn_blocking(move || {
        let _trava = TRAVA.try_lock().map_err(|_| error(StatusCode::TOO_MANY_REQUESTS, "Busca em andamento"))?;
        let inicio = Instant::now();
        let dir = state.state_file.parent().unwrap();
        let indice = omni_core::conversations::read_index(dir);
        let mut fontes = Vec::new();
        let mut conhecidos = HashSet::new();
        let mut sessoes = HashSet::new();
        for conversa in indice {
            for segmento in conversa.segments {
                if inicio.elapsed() >= tempo { return Ok(Json(Resultado { parcial: true, ..Resultado::default() })); }
                if let Some(id) = segmento.external_session_id { sessoes.insert(history_key(&segmento.provider, &id)); }
                if let Some(caminho) = segmento.transcript_path {
                    conhecidos.insert(normalized_path(&caminho));
                    fontes.push(Fonte { conversa_id: conversa.id.clone(), titulo: conversa.title.clone(), caminho: caminho.into(), provider: segmento.provider });
                }
            }
        }
        // O cache é aquecido ao iniciar e pela listagem /painel. Não segura a trava durante o I/O.
        let historico = state.mobile.history.lock().map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Índice indisponível"))?.entries.clone();
        for entrada in historico {
            if conhecidos.contains(&normalized_path(&entrada.path)) || sessoes.contains(&history_key(&entrada.provider, &entrada.session_id)) { continue; }
            fontes.push(Fonte {
                conversa_id: history_id(&entrada),
                titulo: entrada.title.or(entrada.first_prompt).unwrap_or_else(|| "Conversa anterior".into()),
                caminho: entrada.path.into(), provider: entrada.provider,
            });
        }
        Ok(Json(buscar(&fontes, &pedido.texto, pedido.limite.clamp(1, 100), tempo.saturating_sub(inicio.elapsed()))))
    });
    // A resposta também tem teto; o worker verifica seu prazo a cada bloco de leitura.
    match tokio::time::timeout(tempo + Duration::from_millis(100), tarefa).await {
        Ok(resultado) => resultado.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao buscar no histórico"))?,
        Err(_) => Ok(Json(Resultado { parcial: true, ..Resultado::default() })),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn rota_local_busca_no_indice_sem_receber_caminhos() {
        let dir = tempfile::tempdir().unwrap();
        let state = Arc::new(EngineState { persist_lock: Mutex::new(()), mobile: MobileRuntime::new(dir.path()),
            token: "test-only".into(), state_file: dir.path().join("sessions.json"), sessions: Mutex::new(HashMap::new()), id_sequence: AtomicU64::new(1) });
        let caminho = dir.path().join("sintetico.jsonl");
        std::fs::write(&caminho, r#"{"type":"user","timestamp":"2026-10-19T12:00:00Z","message":{"role":"user","content":"Procure navegação no conteúdo"}}"#).unwrap();
        let conversa = omni_core::conversations::Conversation {
            id: "conv-busca".into(), project_id: "p".into(), cwd: "/sintetico".into(), title: "Título sintético".into(),
            title_source: Default::default(), created_at_ms: 1,
            segments: vec![omni_core::conversations::Segment { provider: "claude".into(), profile_id: None,
                external_session_id: None, transcript_path: Some(caminho.to_string_lossy().into()), terminal_session_id: None, started_at_ms: 1, ended_at_ms: None }],
        };
        omni_core::conversations::write_index(dir.path(), &[conversa]).unwrap();
        let r = local(state.clone(), "POST", "/historico/buscar", Some(r#"{"texto":"navegação","limite":1}"#.into()), None, vec![]).await;
        let EngineResponse::Api { status, body } = r else { panic!("resposta inesperada") };
        assert_eq!(status, 200);
        let valor: Value = serde_json::from_str(&body).unwrap();
        assert_eq!(valor["achados"][0]["conversa_id"], "conv-busca");
        assert_eq!(valor["achados"][0]["papel"], "user");
        assert!(valor["achados"][0]["trecho"].as_str().unwrap().contains("conteúdo"));
        let EngineResponse::Api { status, .. } = local(state, "POST", "/historico/buscar", Some(r#"{"texto":"x","path":"/qualquer"}"#.into()), None, vec![]).await else { panic!() };
        assert_eq!(status, 422);
    }
}
