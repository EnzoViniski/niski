//! Fork: caminho citado pelo agente na Conversa (`~/omni/x.md`, `src/App.tsx:12`) vira link.
//!
//! Clicar abre a pasta no Finder ou o arquivo no app padrão — mas só o que não executa nada:
//! documento, imagem e texto abrem direto; pasta comum abre no Finder; qualquer outra coisa (script,
//! `.app`, `.command`, binário) só é **mostrada** na pasta, selecionada. No macOS o `open` de um
//! script ou de um `.app` o executaria.

use std::path::{Path, PathBuf};

/// Abrem direto, além dos entregáveis (`omni_core::entregaveis`): texto puro, que o Mac abre num editor.
const TEXTO: &[&str] = &["md", "markdown", "txt", "json", "csv", "log", "yaml", "yml", "toml", "xml"];
/// Pastas que o Finder trata como programa: abrir rodaria. Ficam só selecionadas.
const PACOTES: &[&str] = &["app", "bundle", "framework", "plugin", "kext", "prefpane", "workflow", "pkg", "mpkg", "action", "saver"];

#[derive(Debug, PartialEq)]
enum Acao { Abrir(PathBuf), Mostrar(PathBuf) }

/// `~/`, relativo à pasta da conversa, e sufixo de linha (`:12`, `:12:3`, `#L3`) fora.
fn resolver(trecho: &str, cwd: Option<&str>) -> Option<PathBuf> {
    let trecho = trecho.trim().trim_matches(|c| matches!(c, '`' | '"' | '\'' | '<' | '>'));
    let trecho = trecho.strip_prefix("file://").unwrap_or(trecho);
    let trecho = trecho.split('#').next().unwrap_or(trecho);
    let mut trecho = trecho.to_string();
    // `arquivo.ts:12:3` → `arquivo.ts` (só tira sufixos numéricos).
    for _ in 0..2 {
        if let Some((antes, depois)) = trecho.rsplit_once(':') {
            if !depois.is_empty() && depois.chars().all(|c| c.is_ascii_digit()) { trecho = antes.to_string(); }
        }
    }
    if trecho.is_empty() || trecho.contains("://") || trecho.contains('\n') { return None; }
    if let Some(resto) = trecho.strip_prefix("~/") {
        return Some(PathBuf::from(std::env::var_os("HOME")?).join(resto));
    }
    let caminho = PathBuf::from(&trecho);
    if caminho.is_absolute() { Some(caminho) } else { Some(Path::new(cwd?).join(caminho)) }
}

fn extensao(caminho: &Path) -> String {
    caminho.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase()
}

fn decidir(trecho: &str, cwd: Option<&str>) -> Result<Acao, String> {
    let caminho = resolver(trecho, cwd).ok_or("Caminho inválido")?;
    let meta = std::fs::metadata(&caminho).map_err(|_| "Não achei esse arquivo ou pasta".to_string())?;
    let ext = extensao(&caminho);
    if meta.is_dir() {
        return Ok(if PACOTES.contains(&ext.as_str()) { Acao::Mostrar(caminho) } else { Acao::Abrir(caminho) });
    }
    let seguro = omni_core::entregaveis::eh_documento(&caminho) || omni_core::entregaveis::eh_imagem(&caminho)
        || TEXTO.contains(&ext.as_str());
    Ok(if seguro { Acao::Abrir(caminho) } else { Acao::Mostrar(caminho) })
}

#[tauri::command]
pub fn abrir_caminho_da_conversa(caminho: String, cwd: Option<String>) -> Result<(), String> {
    match decidir(&caminho, cwd.as_deref())? {
        Acao::Abrir(alvo) => tauri_plugin_opener::open_path(alvo, None::<&str>).map_err(|e| e.to_string()),
        Acao::Mostrar(alvo) => tauri_plugin_opener::reveal_item_in_dir(alvo).map_err(|e| e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn abre_texto_e_pasta_e_so_mostra_o_que_executaria() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().to_str().unwrap();
        for nome in ["notas.md", "script.sh", "relatorio.pdf"] { std::fs::write(dir.path().join(nome), "x").unwrap(); }
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::create_dir(dir.path().join("Outro.app")).unwrap();

        assert_eq!(decidir("notas.md:12", Some(base)), Ok(Acao::Abrir(dir.path().join("notas.md"))));
        assert_eq!(decidir("relatorio.pdf", Some(base)), Ok(Acao::Abrir(dir.path().join("relatorio.pdf"))));
        assert_eq!(decidir("src", Some(base)), Ok(Acao::Abrir(dir.path().join("src"))));
        assert_eq!(decidir("script.sh", Some(base)), Ok(Acao::Mostrar(dir.path().join("script.sh"))));
        assert_eq!(decidir(&format!("{base}/Outro.app"), None), Ok(Acao::Mostrar(dir.path().join("Outro.app"))));
        assert!(decidir("nao-existe.md", Some(base)).is_err());
        assert!(decidir("relativo.md", None).is_err());
        assert!(decidir("https://exemplo.com/a.md", None).is_err());
    }
}
