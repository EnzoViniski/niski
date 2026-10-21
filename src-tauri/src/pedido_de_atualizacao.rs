//! Fork: pedido de "Atualizar o PC" deixado pelo celular (`omni-engine/src/mobile/atualizar_pc.rs`).
//! A interface pergunta a cada poucos segundos (`src/lib/pedidoDeAtualizacao.ts`); o pedido é lido
//! uma vez só (o arquivo some) e pedido velho não vale, para um pedido esquecido não reiniciar o app
//! dias depois.
use std::fs;
use std::path::Path;

const ARQUIVO: &str = "pedido-de-atualizacao.json";
const VALIDADE_MS: u64 = 15 * 60 * 1000;

pub fn consumir(dir: &Path, agora_ms: u64) -> bool {
    let caminho = dir.join(ARQUIVO);
    let Ok(texto) = fs::read_to_string(&caminho) else { return false };
    let _ = fs::remove_file(&caminho);
    let pedido_em = serde_json::from_str::<serde_json::Value>(&texto).ok()
        .and_then(|valor| valor.get("pedido_em_ms").and_then(|v| v.as_u64()));
    matches!(pedido_em, Some(quando) if agora_ms.saturating_sub(quando) <= VALIDADE_MS)
}

#[tauri::command(async)]
pub fn consumir_pedido_de_atualizacao() -> bool {
    let Ok(dir) = crate::engine_client::engine_dir() else { return false };
    let agora = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0);
    let pedido = consumir(&dir, agora);
    if pedido { omni_core::registro::info("pedido_de_atualizacao_atendido", &[]); }
    pedido
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pedir(dir: &Path, quando: u64) { fs::write(dir.join(ARQUIVO), format!("{{\"pedido_em_ms\":{quando}}}")).unwrap(); }

    #[test]
    fn pedido_vale_uma_vez() {
        let dir = tempfile::tempdir().unwrap();
        pedir(dir.path(), 1_000);
        assert!(consumir(dir.path(), 2_000));
        assert!(!consumir(dir.path(), 2_000));
    }

    #[test]
    fn pedido_velho_ou_quebrado_nao_vale_e_some() {
        let dir = tempfile::tempdir().unwrap();
        pedir(dir.path(), 1_000);
        assert!(!consumir(dir.path(), 1_000 + VALIDADE_MS + 1));
        assert!(!dir.path().join(ARQUIVO).exists());
        fs::write(dir.path().join(ARQUIVO), "lixo").unwrap();
        assert!(!consumir(dir.path(), 0));
        assert!(!dir.path().join(ARQUIVO).exists());
    }

    #[test]
    fn sem_pedido_nao_faz_nada() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!consumir(dir.path(), 0));
    }
}
