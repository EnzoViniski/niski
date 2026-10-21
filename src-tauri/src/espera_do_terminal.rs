//! Fork (energia): snapshot do terminal com espera (long-poll, ver `omni-engine/src/espera_de_saida.rs`).
//!
//! O pedido fica parado no engine até a PTY escrever, o estado mudar ou o prazo vencer. Por isso não
//! pode usar a conexão compartilhada de `engine_client` (seguraria o lock e travaria digitação e as
//! outras abas): cada terminal tem a sua, guardada entre um pedido e outro.
use crate::engine_client::{connect, engine_token, exchange};
use omni_protocol::{EngineRequest, EngineResponse};
use std::{io::BufReader, net::TcpStream, sync::Mutex, time::Duration};

/// Conexões ociosas por sessão. Poucas abas ficam visíveis ao mesmo tempo; o teto só evita acumular
/// conexões de abas fechadas.
static OCIOSAS: Mutex<Vec<(String, BufReader<TcpStream>)>> = Mutex::new(Vec::new());
const MAXIMO_OCIOSAS: usize = 16;
/// Teto da espera aceita do cliente (o engine também limita). A leitura desiste um pouco depois.
const ESPERA_MAXIMA_MS: u64 = 25_000;
const FOLGA_DA_LEITURA: Duration = Duration::from_secs(3);

#[tauri::command]
pub async fn terminal_snapshot_espera(session_id: String, since: u64, espera_ms: u64, estado_conhecido: Option<String>) -> EngineResponse {
    // Bloqueia até 25 s: fora das threads do runtime assíncrono.
    tauri::async_runtime::spawn_blocking(move || esperar(session_id, since, espera_ms.min(ESPERA_MAXIMA_MS), estado_conhecido))
        .await
        .unwrap_or_else(|erro| indisponivel(erro.to_string()))
}

fn esperar(session_id: String, since: u64, espera_ms: u64, estado_conhecido: Option<String>) -> EngineResponse {
    let token = match engine_token() {
        Ok(token) => token,
        Err(erro) => return indisponivel(erro),
    };
    let pedido = EngineRequest::Snapshot { token, session_id: session_id.clone(), since, espera_ms, estado_conhecido };
    let prazo = Duration::from_millis(espera_ms) + FOLGA_DA_LEITURA;
    // Snapshot é só leitura: se a conexão guardada morreu parada, repetir numa nova é seguro.
    if let Some(mut guardada) = tirar(&session_id) {
        if let Ok(resposta) = exchange(&mut guardada, &pedido, prazo) {
            devolver(session_id, guardada);
            return resposta;
        }
    }
    let mut nova = match connect() {
        Ok(conexao) => conexao,
        Err(erro) => return indisponivel(erro),
    };
    match exchange(&mut nova, &pedido, prazo) {
        Ok(resposta) => {
            devolver(session_id, nova);
            resposta
        }
        Err(erro) => indisponivel(erro.message),
    }
}

fn tirar(session_id: &str) -> Option<BufReader<TcpStream>> {
    let mut ociosas = OCIOSAS.lock().ok()?;
    let indice = ociosas.iter().position(|(id, _)| id == session_id)?;
    Some(ociosas.swap_remove(indice).1)
}

fn devolver(session_id: String, conexao: BufReader<TcpStream>) {
    let Ok(mut ociosas) = OCIOSAS.lock() else { return };
    if ociosas.len() >= MAXIMO_OCIOSAS {
        ociosas.remove(0);
    }
    ociosas.push((session_id, conexao));
}

fn indisponivel(message: String) -> EngineResponse {
    omni_core::registro::aviso_sem_repetir("engine_indisponivel", "engine_indisponivel", &[("erro", &message)]);
    EngineResponse::Error { code: "ENGINE_UNAVAILABLE".into(), message }
}
