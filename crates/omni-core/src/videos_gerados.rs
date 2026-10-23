//! Fork (vídeo na conversa): o vídeo que o agente **renderizou** aparece na conversa mesmo que ele não
//! cite o caminho na resposta.
//!
//! O `hyperframes render` sem `--output` grava em `renders/<…>/…mp4` (skills do HyperFrames) e o agente
//! nem sempre repete o caminho. Duas fontes, além do texto (`entregaveis::citados`):
//! - o comando: o que vem depois de `-o`/`--output`/`--out` (ou o último argumento, como no `ffmpeg`);
//!   o arquivo ainda não existe quando a chamada é gravada, então aqui não se confere;
//! - a saída do comando (o resultado da ferramenta): qualquer caminho de vídeo que **existe**,
//!   absoluto, `~/` ou relativo à pasta da conversa.
//! Só vídeo (`video/*` em `entregaveis`): o resto continua como era.

use std::path::{Path, PathBuf};

fn eh_video(caminho: &Path) -> bool { crate::entregaveis::tipo(caminho).is_some_and(|t| t.starts_with("video/")) }

/// Pedaços de um comando ou texto, sem aspas e sem a pontuação colada no fim.
fn pedacos(texto: &str) -> Vec<String> {
    texto.split(|c: char| c.is_whitespace() || matches!(c, '"' | '\'' | '`' | '(' | ')' | '[' | ']' | '<' | '>' | ',' | ';'))
        .map(|p| p.trim_end_matches(['.', ':', '!', '?']).to_owned())
        .filter(|p| !p.is_empty())
        .collect()
}

/// Argumentos de um comando de shell, com o que está entre aspas junto (`"saida curta.mov"`).
fn argumentos(comando: &str) -> Vec<String> {
    let mut saida = Vec::new();
    let mut atual = String::new();
    let mut aspas: Option<char> = None;
    for c in comando.chars() {
        match (aspas, c) {
            (Some(a), c) if c == a => aspas = None,
            (Some(_), c) => atual.push(c),
            (None, '"' | '\'') => aspas = Some(c),
            (None, c) if c.is_whitespace() || c == ';' => { if !atual.is_empty() { saida.push(std::mem::take(&mut atual)); } }
            (None, c) => atual.push(c),
        }
    }
    if !atual.is_empty() { saida.push(atual); }
    saida
}

/// Vídeos que o comando vai escrever. `cwd`: a pasta da conversa, para caminho relativo.
pub fn do_comando(comando: &str, cwd: Option<&Path>) -> Vec<PathBuf> {
    let p = argumentos(comando);
    let mut saida = Vec::new();
    for (i, pedaco) in p.iter().enumerate() {
        let alvo = match pedaco.as_str() {
            "-o" | "--output" | "--out" => p.get(i + 1).cloned(),
            outro => ["--output=", "--out=", "-o="].iter().find_map(|f| outro.strip_prefix(f)).map(str::to_owned),
        };
        if let Some(alvo) = alvo { saida.push(alvo); }
    }
    // `ffmpeg -i entrada.mp4 … saida.mp4`: a saída é o último argumento.
    if saida.is_empty() && p.first().is_some_and(|b| b.ends_with("ffmpeg")) { saida.extend(p.last().cloned()); }
    let mut videos = Vec::new();
    for alvo in saida {
        let Some(caminho) = crate::entregaveis::resolver(&alvo, cwd) else { continue };
        if eh_video(&caminho) && !videos.contains(&caminho) { videos.push(caminho); }
    }
    videos
}

/// Vídeos que existem, citados na saída de uma ferramenta.
pub fn da_saida(texto: &str, cwd: Option<&Path>) -> Vec<PathBuf> {
    let mut videos = Vec::new();
    for pedaco in pedacos(texto) {
        if !eh_video(Path::new(&pedaco)) { continue; }
        let Some(caminho) = crate::entregaveis::resolver(&pedaco, cwd) else { continue };
        if !videos.contains(&caminho) && std::fs::metadata(&caminho).is_ok_and(|m| m.is_file()) { videos.push(caminho); }
    }
    videos
}

/// Texto da saída de uma ferramenta numa linha do transcript (Claude: `tool_result`; Codex:
/// `function_call_output`/`custom_tool_call_output`). Só os primeiros 64 KB: é onde o render diz o arquivo.
pub fn texto_de_saida(linha: &serde_json::Value, provider: &str) -> Option<String> {
    const MAXIMO: usize = 64 * 1024;
    let texto = match provider {
        "claude" if linha["type"] == "user" => linha["message"]["content"].as_array()?.iter()
            .filter(|b| b["type"] == "tool_result")
            .flat_map(|b| match &b["content"] {
                serde_json::Value::String(s) => vec![s.clone()],
                serde_json::Value::Array(partes) => partes.iter().filter_map(|p| p["text"].as_str().map(str::to_owned)).collect(),
                _ => Vec::new(),
            })
            .collect::<Vec<_>>().join("\n"),
        "codex" if linha["type"] == "response_item"
            && matches!(linha["payload"]["type"].as_str(), Some("function_call_output" | "custom_tool_call_output")) =>
            linha["payload"]["output"].as_str().map(str::to_owned)?,
        _ => return None,
    };
    if texto.is_empty() { return None; }
    let mut fim = texto.len().min(MAXIMO);
    while !texto.is_char_boundary(fim) { fim -= 1; }
    Some(texto[..fim].to_owned())
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn saida_do_comando_de_render() {
        let cwd = Path::new("/proj");
        assert_eq!(do_comando("npx hyperframes render --output final.mp4", Some(cwd)), [PathBuf::from("/proj/final.mp4")]);
        assert_eq!(do_comando("npx hyperframes render --format webm --output=overlay.webm", Some(cwd)), [PathBuf::from("/proj/overlay.webm")]);
        assert_eq!(do_comando("ffmpeg -i entrada.mp4 -vf scale=720:-2 \"saida curta.mov\"", Some(cwd)), [PathBuf::from("/proj/saida curta.mov")]);
        assert_eq!(do_comando("ffmpeg -y -i a.mp4 /tmp/b.mp4", Some(cwd)), [PathBuf::from("/tmp/b.mp4")]);
        assert!(do_comando("npx hyperframes render", Some(cwd)).is_empty());
        assert!(do_comando("cargo build -o saida.txt", Some(cwd)).is_empty(), "só vídeo");
    }

    #[test]
    fn texto_da_saida_do_claude_e_do_codex() {
        use serde_json::json;
        let claude = json!({"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":[{"type":"text","text":"Rendered a.mp4"}]}]}});
        assert_eq!(texto_de_saida(&claude, "claude").as_deref(), Some("Rendered a.mp4"));
        let codex = json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"c","output":"ok b.mov"}});
        assert_eq!(texto_de_saida(&codex, "codex").as_deref(), Some("ok b.mov"));
        assert!(texto_de_saida(&json!({"type":"assistant"}), "claude").is_none());
    }

    #[test]
    fn render_do_hyperframes_vira_cartao_do_video_na_conversa() {
        use serde_json::json;
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("renders/r1")).unwrap();
        std::fs::write(dir.path().join("renders/r1/output.mp4"), b"x").unwrap();
        let resultado = json!({"type":"user","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"t","content":"✓ Rendered renders/r1/output.mp4"}]}});
        let itens = crate::feed::items_em(&resultado, "claude", Some(dir.path()));
        assert_eq!(itens.len(), 1);
        assert_eq!(itens[0].tool.as_deref(), Some("Vídeo"));
        assert_eq!(itens[0].arquivos, [dir.path().join("renders/r1/output.mp4").to_string_lossy().into_owned()]);
        let chamada = json!({"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","id":"t","name":"Bash",
            "input":{"command":"npx hyperframes render --output final.mp4"}}]}});
        let itens = crate::feed::items_em(&chamada, "claude", Some(dir.path()));
        assert_eq!(itens[0].arquivos, [dir.path().join("final.mp4").to_string_lossy().into_owned()]);
        // Resposta gravada como texto simples (não em blocos) também mostra o vídeo citado.
        let simples = json!({"type":"assistant","message":{"role":"assistant","content":"Pronto: `renders/r1/output.mp4`"}});
        assert_eq!(crate::feed::items_em(&simples, "claude", Some(dir.path()))[0].arquivos.len(), 1);
        let sem_video = json!({"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"ok"}]}});
        assert!(crate::feed::items_em(&sem_video, "claude", Some(dir.path())).is_empty());
    }

    #[test]
    fn saida_da_ferramenta_so_com_video_que_existe() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("renders/r1")).unwrap();
        std::fs::write(dir.path().join("renders/r1/output.mp4"), b"x").unwrap();
        let texto = "✓ Rendered renders/r1/output.mp4 (12.3s). Preview: renders/r1/inexistente.mp4, log.txt";
        assert_eq!(da_saida(texto, Some(dir.path())), [dir.path().join("renders/r1/output.mp4")]);
        assert!(da_saida(texto, None).is_empty(), "relativo sem pasta não vale");
    }
}
