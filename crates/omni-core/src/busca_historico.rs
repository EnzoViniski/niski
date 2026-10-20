//! Busca limitada em fontes já conhecidas pelo índice. Nenhum conteúdo é registrado em log.
use crate::conversations::text_message;
use serde::Serialize;
use std::{collections::HashSet, fs::File, io::{BufRead, BufReader}, path::PathBuf, time::{Duration, Instant}};

const MAX_LINHA: usize = 1024 * 1024;
const MAX_LEITURA: usize = 64 * 1024 * 1024;

pub struct Fonte {
    pub conversa_id: String,
    pub titulo: String,
    pub caminho: PathBuf,
    pub provider: String,
}

#[derive(Debug, Serialize)]
pub struct Achado {
    pub conversa_id: String,
    pub titulo: String,
    pub papel: String,
    pub trecho: String,
    pub data: Option<String>,
}

#[derive(Default, Debug, Serialize)]
pub struct Resultado {
    pub achados: Vec<Achado>,
    /// O cliente pode refinar a busca quando atingimos tempo, bytes, tamanho de linha ou resultados.
    pub parcial: bool,
}

/// Mantém contexto antes/depois do acerto e nunca corta no meio de um caractere UTF-8.
fn trecho(texto: &str, consulta: &str) -> Option<String> {
    let minusculo = texto.to_lowercase();
    let inicio = minusculo.find(consulta)?;
    // Unicode pode expandir ao baixar a caixa (İ → i + ponto): mapeia de volta para o original.
    let mut bytes = 0;
    let mut posicao = 0;
    for (indice, caractere) in texto.chars().enumerate() {
        if bytes >= inicio { posicao = indice; break; }
        bytes += caractere.to_lowercase().map(char::len_utf8).sum::<usize>();
        posicao = indice + 1;
    }
    let antes = posicao.saturating_sub(90);
    let tamanho = 180 + consulta.chars().count();
    let mut recorte: String = texto.chars().skip(antes).take(tamanho).collect();
    let tem_depois = texto.chars().skip(antes + tamanho).next().is_some();
    recorte = recorte.split_whitespace().collect::<Vec<_>>().join(" ");
    if antes > 0 { recorte.insert(0, '…'); }
    if tem_depois { recorte.push('…'); }
    Some(recorte)
}

/// Tempo cooperativo checado a cada bloco de 8 KiB; memória limitada por linha e por resposta.
/// Fontes são do índice do engine, nunca caminhos recebidos do cliente.
pub fn buscar(fontes: &[Fonte], texto: &str, limite: usize, tempo: Duration) -> Resultado {
    let inicio = Instant::now();
    let consulta = texto.trim().to_lowercase();
    let mut resultado = Resultado::default();
    if consulta.is_empty() || consulta.chars().count() > 200 || limite == 0 { return resultado; }
    let limite = limite.min(100);
    let tempo = tempo.min(Duration::from_millis(1500));
    let mut lidos = 0;
    let mut vistos = HashSet::new();
    for fonte in fontes {
        if inicio.elapsed() >= tempo { resultado.parcial = true; break; }
        if !matches!(fonte.provider.as_str(), "claude" | "codex") { continue; }
        if !vistos.insert((&fonte.conversa_id, &fonte.caminho)) { continue; }
        // Não segue diretórios/FIFOs nem symlinks: o índice só aponta para transcripts regulares.
        if !std::fs::symlink_metadata(&fonte.caminho).is_ok_and(|m| m.file_type().is_file()) { continue; }
        let Ok(arquivo) = File::open(&fonte.caminho) else { continue };
        let mut leitor = BufReader::with_capacity(8192, arquivo);
        let mut linha = Vec::new();
        let mut longa = false;
        loop {
            if inicio.elapsed() >= tempo || lidos >= MAX_LEITURA {
                resultado.parcial = true;
                return resultado;
            }
            let Ok(bloco) = leitor.fill_buf() else { break };
            if bloco.is_empty() { break; }
            let tamanho = bloco.iter().position(|b| *b == b'\n').map_or(bloco.len(), |p| p + 1);
            let completa = bloco[tamanho - 1] == b'\n';
            if !longa && linha.len() + tamanho <= MAX_LINHA { linha.extend_from_slice(&bloco[..tamanho]); }
            else { longa = true; resultado.parcial = true; linha.clear(); }
            leitor.consume(tamanho);
            lidos += tamanho;
            if !completa { continue; }
            if !longa { procurar(&linha, fonte, &consulta, &mut resultado); }
            linha.clear(); longa = false;
            if resultado.achados.len() >= limite { resultado.parcial = true; return resultado; }
        }
        if !longa && !linha.is_empty() { procurar(&linha, fonte, &consulta, &mut resultado); }
        if resultado.achados.len() >= limite { resultado.parcial = true; break; }
    }
    resultado
}

fn procurar(linha: &[u8], fonte: &Fonte, consulta: &str, resultado: &mut Resultado) {
    let Ok(valor) = serde_json::from_slice::<serde_json::Value>(linha) else { return };
    if valor["isMeta"] == true { return; }
    let Some((papel, texto)) = text_message(&valor, &fonte.provider) else { return };
    let texto = texto.trim();
    if papel == "user" && (texto.starts_with('<') || texto.starts_with("# AGENTS.md") || texto.starts_with("Caveat:")) { return; }
    let Some(trecho) = trecho(texto, consulta) else { return };
    resultado.achados.push(Achado {
        conversa_id: fonte.conversa_id.clone(), titulo: fonte.titulo.clone(), papel, trecho,
        data: valor["timestamp"].as_str().map(str::to_owned),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn fonte(dir: &std::path::Path, provider: &str, texto: &str) -> Fonte {
        let caminho = dir.join(format!("{provider}.jsonl"));
        std::fs::write(&caminho, texto).unwrap();
        Fonte { conversa_id: provider.into(), titulo: "Conversa sintética".into(), caminho, provider: provider.into() }
    }
    #[test]
    fn busca_claude_e_codex_com_papel_contexto_e_data() {
        let dir = tempfile::tempdir().unwrap();
        let claude = fonte(dir.path(), "claude", &format!("{}\n{}\n{}\n", 
            json!({"type":"user","timestamp":"2026-10-19T12:00:00Z","message":{"role":"user","content":"Antes, revisão de NAVEGAÇÃO, depois."}}),
            json!({"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","input":{"text":"navegação"}}]}}),
            json!({"type":"user","message":{"role":"user","content":"# AGENTS.md navegação"}})));
        let codex = fonte(dir.path(), "codex", &json!({"type":"response_item","timestamp":"2026-10-19T13:00:00Z","payload":{"type":"message","role":"assistant","content":[{"type":"output_text","text":"Minha navegação está pronta."}]}}).to_string());
        let r = buscar(&[claude, codex], "navegação", 10, Duration::from_secs(1));
        assert_eq!(r.achados.len(), 2);
        assert_eq!(r.achados[0].papel, "user");
        assert_eq!(r.achados[1].papel, "assistant");
        assert_eq!(r.achados[0].data.as_deref(), Some("2026-10-19T12:00:00Z"));
        assert!(r.achados[0].trecho.contains("Antes, revisão"));
        assert!(!r.parcial);
    }
    #[test]
    fn limites_de_tempo_resultados_e_consulta_vazia() {
        let dir = tempfile::tempdir().unwrap();
        let linha = json!({"type":"user","message":{"role":"user","content":"agulha"}}).to_string();
        let fontes = [fonte(dir.path(), "claude", &format!("{linha}\n{linha}\n"))];
        let r = buscar(&fontes, "agulha", 1, Duration::from_secs(1));
        assert_eq!(r.achados.len(), 1); assert!(r.parcial);
        let r = buscar(&fontes, "agulha", 10, Duration::ZERO);
        assert!(r.achados.is_empty()); assert!(r.parcial);
        assert!(buscar(&fontes, " ", 10, Duration::from_secs(1)).achados.is_empty());
        assert!(buscar(&fontes, "agulha", 0, Duration::from_secs(1)).achados.is_empty());
    }
    #[test]
    fn ignora_json_invalido_e_linha_gigante_sem_perder_a_proxima() {
        let dir = tempfile::tempdir().unwrap();
        let linha = json!({"type":"user","message":{"role":"user","content":"agulha"}}).to_string();
        let fontes = [fonte(dir.path(), "claude", &format!("inválido\n{}\n{linha}", "x".repeat(MAX_LINHA + 1)))];
        let r = buscar(&fontes, "agulha", 10, Duration::from_secs(1));
        assert_eq!(r.achados.len(), 1); assert!(r.parcial);
    }
    #[test]
    fn trecho_unicode_longo_fica_limitado() {
        let texto = format!("{}İÁGULHA{}", "á".repeat(500), "ç".repeat(500));
        let t = trecho(&texto, "i\u{307}águlha").unwrap();
        assert!(t.contains("İÁGULHA")); assert!(t.starts_with('…')); assert!(t.ends_with('…'));
        assert!(t.chars().count() < 400);
    }
}
