//! Configurações → Geral → Sessões. Grava o `sessoes.json` na pasta do engine, que o relê a
//! cada conferência (`omni_core::limite_de_sessoes`): vale na hora, sem reiniciar nada.

use omni_core::limite_de_sessoes as limite;
use serde::Serialize;

#[derive(Serialize)]
pub struct LimiteDeSessoes { limite: usize, padrao: usize, minimo: usize, maximo: usize }

fn atual(valor: usize) -> LimiteDeSessoes {
    LimiteDeSessoes { limite: valor, padrao: limite::PADRAO, minimo: limite::MINIMO, maximo: limite::MAXIMO }
}

#[tauri::command]
pub fn limite_de_sessoes() -> Result<LimiteDeSessoes, String> {
    Ok(atual(limite::ler(&crate::engine_client::engine_dir()?)))
}

#[tauri::command]
pub fn definir_limite_de_sessoes(valor: usize) -> Result<LimiteDeSessoes, String> {
    limite::gravar(&crate::engine_client::engine_dir()?, valor).map(atual)
}
