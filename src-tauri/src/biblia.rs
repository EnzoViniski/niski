//! Fork (Bíblia do dia): a Bíblia Sagrada Católica (tradução Ave Maria, 73 livros) em JSON, do
//! repositório público `fidalgobr/bibliaAveMariaJSON`, num commit fixo. O instalador não leva o
//! texto: o app baixa uma vez para a pasta de dados (como o modelo de voz, pelo `curl` do sistema)
//! e só aceita o arquivo se o tamanho e o SHA-256 baterem. Assim o texto é sempre o mesmo, letra por
//! letra, e o domínio (trilhas, passagens, o que já foi lido) mora no front, em `src/features/biblia/`.

use crate::voz::integridade_modelo;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

const URL: &str = "https://raw.githubusercontent.com/fidalgobr/bibliaAveMariaJSON/bb8c7ac2fd91f25234c78107db94342a5923281b/bibliaAveMaria.json";
const BYTES: u64 = 6_618_747;
const SHA256: &str = "d7198b6ef375b0c51c8cb42dbdfcd70f8c7ac6d8389ca69c53115b00a0b0ee97";
const PASTA: &str = "biblia";
const ARQUIVO: &str = "ave-maria.json";

static BAIXANDO: AtomicBool = AtomicBool::new(false);

fn pasta() -> Result<PathBuf, String> {
    Ok(crate::engine_client::engine_dir()?.join(PASTA))
}

/// O texto, se já foi baixado e continua íntegro. Arquivo mexido conta como não baixado.
fn ler_de(pasta: &std::path::Path) -> Result<Option<String>, String> {
    let caminho = pasta.join(ARQUIVO);
    if !std::fs::symlink_metadata(&caminho).map(|m| m.file_type().is_file()).unwrap_or(false) { return Ok(None); }
    if integridade_modelo::conferir(&caminho, BYTES, SHA256).is_err() { return Ok(None); }
    std::fs::read_to_string(&caminho).map(Some).map_err(|e| e.to_string())
}

#[tauri::command(async)]
pub fn biblia_ler() -> Result<Option<String>, String> {
    ler_de(&pasta()?)
}

#[tauri::command]
pub async fn biblia_baixar() -> Result<String, String> {
    if BAIXANDO.swap(true, Ordering::SeqCst) {
        return Err("A Bíblia já está sendo baixada".into());
    }
    let resultado = tauri::async_runtime::spawn_blocking(baixar).await.map_err(|e| e.to_string()).and_then(|r| r);
    BAIXANDO.store(false, Ordering::SeqCst);
    resultado
}

fn baixar() -> Result<String, String> {
    let pasta = pasta()?;
    if let Some(texto) = ler_de(&pasta)? { return Ok(texto); }
    std::fs::create_dir_all(&pasta).map_err(|e| e.to_string())?;
    let parcial = pasta.join(format!("{ARQUIVO}.parcial"));
    let _ = std::fs::remove_file(&parcial);
    let inicio = Instant::now();
    let status = Command::new("curl")
        .args(["-L", "--fail", "--silent", "--show-error", "--retry", "3", "--max-time", "120", "-o"])
        .arg(&parcial)
        .arg(URL)
        .status()
        .map_err(|e| format!("Não consegui rodar o curl: {e}"))?;
    if !status.success() {
        let _ = std::fs::remove_file(&parcial);
        return Err("Não consegui baixar a Bíblia. Confira a internet e tente de novo.".into());
    }
    if let Err(erro) = integridade_modelo::conferir(&parcial, BYTES, SHA256) {
        let _ = std::fs::remove_file(&parcial);
        return Err(format!("O arquivo da Bíblia não passou na verificação ({erro}). Tente de novo."));
    }
    std::fs::rename(&parcial, pasta.join(ARQUIVO)).map_err(|e| e.to_string())?;
    omni_core::registro::info("biblia_baixada", &[("ms", &inicio.elapsed().as_millis().to_string())]);
    ler_de(&pasta)?.ok_or_else(|| "O arquivo da Bíblia sumiu depois de baixado".into())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn arquivo_ausente_ou_diferente_conta_como_nao_baixado() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(ler_de(dir.path()).unwrap(), None);
        std::fs::write(dir.path().join(ARQUIVO), "{\"antigoTestamento\":[]}").unwrap();
        assert_eq!(ler_de(dir.path()).unwrap(), None);
    }

    #[cfg(unix)]
    #[test]
    fn link_no_lugar_do_arquivo_nao_vale() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("outro.json"), "{}").unwrap();
        std::os::unix::fs::symlink(dir.path().join("outro.json"), dir.path().join(ARQUIVO)).unwrap();
        assert_eq!(ler_de(dir.path()).unwrap(), None);
    }
}
