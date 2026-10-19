//! Fork: abre no app padrão do Mac (Visualização, Numbers, Keynote, navegador…) o arquivo que o
//! agente gerou e a Conversa mostra como cartão.
//!
//! Comando próprio em vez do `opener:allow-open-path` do webview: aqui só passa arquivo comum com
//! extensão de entregável (`omni_core::entregaveis`). Assim um caminho vindo do texto do agente nunca
//! abre um `.app`, `.command` ou script — no macOS `open` executaria esses.

use std::path::Path;

fn validar(caminho: &str) -> Result<&Path, String> {
    let caminho = Path::new(caminho);
    if !caminho.is_absolute() { return Err("Caminho inválido".into()); }
    let entregavel = omni_core::entregaveis::eh_documento(caminho) || omni_core::entregaveis::eh_imagem(caminho);
    if !entregavel { return Err("Este tipo de arquivo não abre pela conversa".into()); }
    match std::fs::metadata(caminho) {
        Ok(m) if m.is_file() => Ok(caminho),
        _ => Err("O arquivo não existe mais (foi movido ou apagado)".into()),
    }
}

#[tauri::command]
pub fn abrir_entregavel(caminho: String) -> Result<(), String> {
    let caminho = validar(&caminho)?;
    tauri_plugin_opener::open_path(caminho, None::<&str>).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn mostrar_entregavel_na_pasta(caminho: String) -> Result<(), String> {
    let caminho = validar(&caminho)?;
    tauri_plugin_opener::reveal_item_in_dir(caminho).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn so_abre_entregavel_que_existe() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("a.pdf");
        let script = dir.path().join("rodar.command");
        std::fs::write(&pdf, b"%PDF").unwrap();
        std::fs::write(&script, b"echo").unwrap();
        assert!(validar(pdf.to_str().unwrap()).is_ok());
        assert!(validar(script.to_str().unwrap()).is_err());
        assert!(validar(dir.path().join("b.pdf").to_str().unwrap()).is_err());
        assert!(validar("relativo.pdf").is_err());
    }
}
