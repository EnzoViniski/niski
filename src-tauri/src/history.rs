//! Histórico nativo do Claude e do Codex para a tela Histórico. A varredura mora no `omni-core`;
//! aqui só ficam o cache entre chamadas e o acesso às contas cadastradas.
use omni_core::history::{self, HistoryEntry, HistoryTranscript, ScanCache};
use std::{
    path::Path,
    sync::{Mutex, OnceLock},
};

static CACHE: OnceLock<Mutex<ScanCache>> = OnceLock::new();

#[tauri::command]
pub async fn agent_history(provider: String) -> Result<Vec<HistoryEntry>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let profiles = crate::profiles::list_profiles();
        let dir = crate::engine_client::engine_dir()?;
        let mut cache = CACHE.get_or_init(Default::default).lock().map_err(|_| "cache do histórico corrompido".to_string())?;
        Ok(omni_core::arquivo_de_conversas::scan(&dir, &provider, &profiles, &mut cache))
    })
    .await
    .map_err(|error| error.to_string())?
}

#[tauri::command]
pub async fn agent_history_transcript(provider: String, path: String) -> Result<HistoryTranscript, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let dir = crate::engine_client::engine_dir()?;
        let profiles = omni_core::arquivo_de_conversas::com_arquivo(&dir, &crate::profiles::list_profiles());
        history::transcript(Path::new(&path), &provider, &profiles)
            .ok_or_else(|| "Transcript não encontrado ou fora das contas cadastradas".to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}
