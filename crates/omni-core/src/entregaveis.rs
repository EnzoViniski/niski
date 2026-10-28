//! O que o agente **gerou** numa conversa (imagem, PDF, planilha, apresentação, página…),
//! para a conversa mostrar como imagem ou como cartão de arquivo, igual aos apps do Claude e do Codex.
//!
//! Três origens, todas lidas do transcript que a própria CLI grava:
//! - imagem gerada pelo Codex (`item_completed` → `Extension` `image_gen.*` com `savedPath`): a CLI
//!   dizia "gerei a imagem" e nada aparecia, porque o arquivo nunca é citado no texto;
//! - arquivo citado na resposta: caminho absoluto, `~/…` ou relativo à pasta da conversa, solto, entre
//!   crases ou como link markdown — só vale se o arquivo existir;
//! - arquivo criado por ferramenta (`Write` do Claude, `*** Add File:` do Codex).
//!
//! Só extensões de "entregável" entram: código e texto puro ficam de fora, senão cada `src/x.ts`
//! citado viraria cartão.

use std::path::{Path, PathBuf};

/// Imagens: viram imagem na conversa (as mesmas do `feed`).
pub use crate::feed::EXTENSOES_DE_IMAGEM;

/// Documentos e mídia: viram cartão que abre no app padrão (PC) ou baixa (celular).
pub const EXTENSOES_DE_DOCUMENTO: &[(&str, &str)] = &[
    (".pdf", "application/pdf"),
    (".docx", "application/vnd.openxmlformats-officedocument.wordprocessingml.document"),
    (".doc", "application/msword"),
    (".xlsx", "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
    (".xls", "application/vnd.ms-excel"),
    (".csv", "text/csv"),
    (".tsv", "text/tab-separated-values"),
    (".pptx", "application/vnd.openxmlformats-officedocument.presentationml.presentation"),
    (".ppt", "application/vnd.ms-powerpoint"),
    (".odt", "application/vnd.oasis.opendocument.text"),
    (".ods", "application/vnd.oasis.opendocument.spreadsheet"),
    (".odp", "application/vnd.oasis.opendocument.presentation"),
    (".rtf", "application/rtf"),
    (".epub", "application/epub+zip"),
    (".html", "text/html"),
    (".htm", "text/html"),
    (".svg", "image/svg+xml"),
    (".mp4", "video/mp4"),
    (".mov", "video/quicktime"),
    (".webm", "video/webm"),
    (".mp3", "audio/mpeg"),
    (".wav", "audio/wav"),
    (".m4a", "audio/mp4"),
    (".zip", "application/zip"),
];

/// Maior arquivo que o celular baixa pela conversa (vai em base64 dentro do JSON).
pub const ARQUIVO_MAX_BYTES: u64 = 25 * 1024 * 1024;

fn tipo_em(caminho: &Path, tabela: &[(&str, &'static str)]) -> Option<&'static str> {
    let nome = caminho.to_string_lossy().to_ascii_lowercase();
    tabela.iter().find(|(ext, _)| nome.ends_with(ext)).map(|(_, tipo)| *tipo)
}

pub fn eh_imagem(caminho: &Path) -> bool { tipo_em(caminho, EXTENSOES_DE_IMAGEM).is_some() }
pub fn eh_documento(caminho: &Path) -> bool { tipo_em(caminho, EXTENSOES_DE_DOCUMENTO).is_some() }

/// Tipo MIME de um entregável (documento ou imagem).
pub fn tipo(caminho: &Path) -> Option<&'static str> {
    tipo_em(caminho, EXTENSOES_DE_DOCUMENTO).or_else(|| tipo_em(caminho, EXTENSOES_DE_IMAGEM))
}

/// `~/x`, `file:///x`, `%20` e relativo à pasta da conversa viram caminho absoluto.
pub(crate) fn resolver(trecho: &str, cwd: Option<&Path>) -> Option<PathBuf> { // Também usado em `videos_gerados`.
    let trecho = trecho.trim().trim_matches(|c| matches!(c, '<' | '>' | '"' | '\''));
    let trecho = trecho.strip_prefix("file://").unwrap_or(trecho);
    // Link markdown com título (`[a](x.pdf "título")`) e âncora/linha (`x.pdf#L3`) perdem o resto.
    let trecho = trecho.split(" \"").next().unwrap_or(trecho);
    let trecho = trecho.split('#').next().unwrap_or(trecho);
    let trecho = trecho.replace("%20", " ");
    if trecho.is_empty() || trecho.contains("://") || trecho.contains('\n') { return None; }
    let caminho = if let Some(resto) = trecho.strip_prefix("~/") {
        PathBuf::from(std::env::var_os("HOME")?).join(resto)
    } else {
        let caminho = PathBuf::from(&trecho);
        if caminho.is_absolute() { caminho } else { cwd?.join(caminho) }
    };
    Some(caminho)
}

/// Trechos entre crases e destinos de link markdown: onde caminho relativo costuma aparecer.
fn trechos_marcados(texto: &str) -> Vec<&str> {
    let mut trechos = Vec::new();
    let mut resto = texto;
    while let Some(i) = resto.find('`') {
        let depois = &resto[i + 1..];
        let Some(f) = depois.find('`') else { break };
        trechos.push(&depois[..f]);
        resto = &depois[f + 1..];
    }
    let mut resto = texto;
    while let Some(i) = resto.find("](") {
        let depois = &resto[i + 2..];
        let Some(f) = depois.find(')') else { break };
        trechos.push(&depois[..f]);
        resto = &depois[f + 1..];
    }
    trechos
}

/// Imagens e documentos citados num texto do agente, na ordem e sem repetir. Só arquivos que existem.
pub fn citados(texto: &str, cwd: Option<&Path>) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut imagens = crate::feed::caminhos_de_imagem(texto);
    let mut documentos = crate::feed::caminhos_citados(texto, EXTENSOES_DE_DOCUMENTO, u64::MAX);
    for trecho in trechos_marcados(texto) {
        let Some(caminho) = resolver(trecho, cwd) else { continue };
        let lista = if eh_imagem(&caminho) { &mut imagens } else if eh_documento(&caminho) { &mut documentos } else { continue };
        if lista.contains(&caminho) { continue; }
        let cabe = std::fs::metadata(&caminho).is_ok_and(|m| m.is_file()
            && (!eh_imagem(&caminho) || m.len() <= crate::feed::IMAGEM_MAX_BYTES));
        if cabe { lista.push(caminho); }
    }
    (imagens, documentos)
}

/// Arquivo criado por ferramenta que é entregável. O arquivo pode ainda não existir quando a linha
/// é lida (a chamada é gravada antes de rodar), então aqui não se confere — o cartão é que avisa.
pub fn criado(caminho: &str, cwd: Option<&Path>) -> Option<PathBuf> {
    let caminho = resolver(caminho, cwd)?;
    (eh_documento(&caminho) || eh_imagem(&caminho)).then_some(caminho)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn documento_citado_por_caminho_relativo_absoluto_ou_link_vira_entregavel() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("saida")).unwrap();
        std::fs::write(dir.path().join("saida/relatorio.pdf"), b"%PDF").unwrap();
        std::fs::write(dir.path().join("planilha final.xlsx"), b"x").unwrap();
        std::fs::write(dir.path().join("logo.png"), b"x").unwrap();
        std::fs::write(dir.path().join("main.ts"), b"x").unwrap();
        let absoluto = dir.path().join("planilha final.xlsx");
        let texto = format!(
            "Pronto: `saida/relatorio.pdf`, a planilha em {} e [o logo](logo.png). Mexi em `main.ts` e `sumiu.pdf`.",
            absoluto.display());
        let (imagens, documentos) = citados(&texto, Some(dir.path()));
        assert_eq!(imagens, vec![dir.path().join("logo.png")]);
        assert_eq!(documentos, vec![absoluto, dir.path().join("saida/relatorio.pdf")]);
    }

    #[test]
    fn imagem_gerada_pelo_codex_aparece_mesmo_sem_ser_citada() {
        let dir = tempfile::tempdir().unwrap();
        let png = dir.path().join("exec-1.png");
        std::fs::write(&png, b"\x89PNG").unwrap();
        let linha = serde_json::json!({"timestamp":"2026-10-19T03:23:25Z","type":"event_msg","payload":{"type":"item_completed",
            "item":{"type":"Extension","kind":"image_gen.generation","status":"completed","savedPath":png}}});
        let itens = crate::feed::items(&linha, "codex");
        assert_eq!(itens.len(), 1);
        assert_eq!((itens[0].role.as_str(), itens[0].imagens), ("assistant", 1));
        assert_eq!(itens[0].midia, vec![png]);
        // Já apagada: nada aparece (em vez de uma imagem quebrada).
        let sumiu = serde_json::json!({"type":"event_msg","payload":{"type":"item_completed",
            "item":{"type":"Extension","kind":"image_gen.generation","savedPath":dir.path().join("x.png")}}});
        assert!(crate::feed::items(&sumiu, "codex").is_empty());
    }

    #[test]
    fn documento_escrito_ou_citado_vai_para_a_mensagem_com_a_pasta_do_transcript() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("proposta.pdf"), b"%PDF").unwrap();
        let cwd = dir.path().to_string_lossy().into_owned();
        let escrita = serde_json::json!({"type":"assistant","cwd":cwd,"message":{"role":"assistant","content":[
            {"type":"tool_use","name":"Write","input":{"file_path":"relatorio.docx","content":"x"}}]}});
        let itens = crate::feed::items_em(&escrita, "claude", Some(dir.path()));
        assert_eq!(itens[0].arquivos, vec![dir.path().join("relatorio.docx").to_string_lossy().into_owned()]);
        let fala = serde_json::json!({"type":"assistant","message":{"role":"assistant","content":[
            {"type":"text","text":"Gerei a proposta em `proposta.pdf`."}]}});
        let itens = crate::feed::items_em(&fala, "claude", Some(dir.path()));
        assert_eq!(itens[0].arquivos, vec![dir.path().join("proposta.pdf").to_string_lossy().into_owned()]);
    }

    #[test]
    fn sem_pasta_relativo_nao_vale_e_codigo_nunca_vira_cartao() {
        assert_eq!(citados("veja `saida/relatorio.pdf`", None), (vec![], vec![]));
        assert_eq!(criado("src/app.ts", Some(Path::new("/p"))), None);
        assert_eq!(criado("docs/plano.docx", Some(Path::new("/p"))), Some(PathBuf::from("/p/docs/plano.docx")));
    }
}
