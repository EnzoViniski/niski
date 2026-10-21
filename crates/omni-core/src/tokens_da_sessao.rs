//! Fork: total de tokens gastos na conversa, para o topo da aba (pedido do Enzo, 21/10/2026).
//!
//! - **Claude Code**: cada resposta grava `message.usage` (`input_tokens`, `cache_creation_input_tokens`,
//!   `cache_read_input_tokens`, `output_tokens`). A mesma resposta aparece em várias linhas (uma por
//!   bloco de conteúdo) com o mesmo `message.id` e o mesmo uso: conta uma vez por id.
//! - **Codex**: o rollout grava eventos `token_count` com o acumulado da sessão em
//!   `info.total_token_usage`; vale o último.
//!
//! Entrada = tudo que o modelo leu (inclusive o que veio do cache), saída = o que ele escreveu — a mesma
//! conta do `total_tokens` do Codex. Leitura incremental com cache por arquivo: só o que foi
//! acrescentado desde a última vez (transcript passa de 5 MB e a lista é consultada a cada segundo).
use crate::conversations::Conversation;
use serde::Serialize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
pub struct Tokens {
    /// Entrada + saída.
    pub total: u64,
    /// Tudo que o modelo leu, inclusive do cache.
    pub entrada: u64,
    /// Parte da entrada que veio do cache (mais barata).
    pub cache: u64,
    pub saida: u64,
}

impl Tokens {
    fn somar(&mut self, outro: Tokens) {
        self.total += outro.total;
        self.entrada += outro.entrada;
        self.cache += outro.cache;
        self.saida += outro.saida;
    }
}

#[derive(Default, Clone)]
struct Leitura {
    lido_ate: u64,
    claude: Tokens,
    vistos: HashSet<String>,
    codex: Option<Tokens>,
}

static CACHE: OnceLock<Mutex<HashMap<PathBuf, Leitura>>> = OnceLock::new();

fn numero(valor: &Value, campo: &str) -> u64 { valor[campo].as_u64().unwrap_or(0) }

/// Uso de uma linha do transcript do Claude (com o id da mensagem, para não contar duas vezes).
fn uso_do_claude(linha: &str) -> Option<(String, Tokens)> {
    if !linha.contains("\"usage\"") || !linha.contains("\"assistant\"") { return None; }
    let valor: Value = serde_json::from_str(linha).ok()?;
    if valor["type"] != "assistant" { return None; }
    let mensagem = &valor["message"];
    let uso = &mensagem["usage"];
    if !uso.is_object() { return None; }
    let id = mensagem["id"].as_str().map(str::to_owned).or_else(|| valor["uuid"].as_str().map(str::to_owned))?;
    let cache = numero(uso, "cache_read_input_tokens");
    let entrada = numero(uso, "input_tokens") + numero(uso, "cache_creation_input_tokens") + cache;
    let saida = numero(uso, "output_tokens");
    Some((id, Tokens { total: entrada + saida, entrada, cache, saida }))
}

/// Acumulado de um evento `token_count` do rollout do Codex.
fn uso_do_codex(linha: &str) -> Option<Tokens> {
    if !linha.contains("\"token_count\"") { return None; }
    let valor: Value = serde_json::from_str(linha).ok()?;
    if valor["payload"]["type"] != "token_count" { return None; }
    let uso = &valor["payload"]["info"]["total_token_usage"];
    if !uso.is_object() { return None; }
    let entrada = numero(uso, "input_tokens");
    let saida = numero(uso, "output_tokens");
    let total = uso["total_tokens"].as_u64().unwrap_or(entrada + saida);
    Some(Tokens { total, entrada, cache: numero(uso, "cached_input_tokens"), saida })
}

fn ler(caminho: &Path, provider: &str) -> Option<Tokens> {
    let tamanho = std::fs::metadata(caminho).ok()?.len();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let mut leitura = cache.lock().ok()?.get(caminho).cloned().unwrap_or_default();
    // Arquivo encolheu (reescrito): começa de novo.
    if tamanho < leitura.lido_ate { leitura = Leitura::default(); }
    if tamanho > leitura.lido_ate {
        let mut arquivo = File::open(caminho).ok()?;
        arquivo.seek(SeekFrom::Start(leitura.lido_ate)).ok()?;
        let mut leitor = BufReader::new(arquivo);
        let mut linha = String::new();
        loop {
            linha.clear();
            let Ok(lidos) = leitor.read_line(&mut linha) else { break };
            // Linha sem "\n" no fim ainda está sendo escrita: fica para a próxima leitura.
            if lidos == 0 || !linha.ends_with('\n') { break; }
            leitura.lido_ate += lidos as u64;
            match provider {
                "claude" => if let Some((id, uso)) = uso_do_claude(linha.trim_end()) {
                    if leitura.vistos.insert(id) { leitura.claude.somar(uso); }
                },
                "codex" => if let Some(uso) = uso_do_codex(linha.trim_end()) { leitura.codex = Some(uso); },
                _ => {}
            }
        }
    }
    let resultado = match provider { "claude" => Some(leitura.claude), "codex" => leitura.codex, _ => None };
    if let Ok(mut mapa) = cache.lock() {
        if mapa.len() > 512 { mapa.clear(); }
        mapa.insert(caminho.to_path_buf(), leitura);
    }
    resultado.filter(|tokens| tokens.total > 0)
}

/// Soma de todos os segmentos (trocar de conta no meio continua sendo a mesma conversa).
pub fn da_conversa(conversa: &Conversation) -> Option<Tokens> {
    let mut soma = Tokens::default();
    let mut achou = false;
    for segmento in &conversa.segments {
        let Some(caminho) = segmento.transcript_path.as_deref() else { continue };
        if let Some(uso) = ler(Path::new(caminho), &segmento.provider) { soma.somar(uso); achou = true; }
    }
    achou.then_some(soma)
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::conversations::{Segment, TitleSource};
    use std::fs::{self, OpenOptions};
    use std::io::Write;

    fn conversa(provider: &str, caminho: &Path) -> Conversation {
        Conversation { id: "c".into(), project_id: "p".into(), cwd: ".".into(), title: "Conversa".into(), title_source: TitleSource::Auto,
            created_at_ms: 0, segments: vec![Segment { provider: provider.into(), profile_id: None, external_session_id: None,
                transcript_path: Some(caminho.to_string_lossy().into_owned()), terminal_session_id: None, started_at_ms: 0, ended_at_ms: None }] }
    }
    fn resposta(id: &str, entrada: u64, criacao: u64, cache: u64, saida: u64) -> String {
        format!("{{\"type\":\"assistant\",\"message\":{{\"id\":\"{id}\",\"role\":\"assistant\",\"usage\":{{\"input_tokens\":{entrada},\"cache_creation_input_tokens\":{criacao},\"cache_read_input_tokens\":{cache},\"output_tokens\":{saida}}}}}}}\n")
    }

    #[test]
    fn claude_conta_cada_resposta_uma_vez_e_soma_o_que_chega_depois() {
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join("s.jsonl");
        // A mesma resposta em duas linhas (dois blocos de conteúdo) conta uma vez só.
        fs::write(&t, format!("{{\"type\":\"user\"}}\n{}{}", resposta("m1", 10, 100, 1000, 50), resposta("m1", 10, 100, 1000, 50))).unwrap();
        let c = conversa("claude", &t);
        assert_eq!(da_conversa(&c), Some(Tokens { total: 1160, entrada: 1110, cache: 1000, saida: 50 }));

        OpenOptions::new().append(true).open(&t).unwrap().write_all(resposta("m2", 5, 0, 1100, 20).as_bytes()).unwrap();
        assert_eq!(da_conversa(&c).unwrap().total, 1160 + 1125);
    }

    #[test]
    fn linha_pela_metade_espera_a_proxima_leitura() {
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join("s.jsonl");
        let linha = resposta("m1", 10, 0, 0, 5);
        fs::write(&t, &linha[..linha.len() - 20]).unwrap();
        assert_eq!(da_conversa(&conversa("claude", &t)), None);
        OpenOptions::new().append(true).open(&t).unwrap().write_all(linha[linha.len() - 20..].as_bytes()).unwrap();
        assert_eq!(da_conversa(&conversa("claude", &t)).unwrap().total, 15);
    }

    #[test]
    fn codex_usa_o_ultimo_acumulado() {
        let dir = tempfile::tempdir().unwrap();
        let r = dir.path().join("rollout.jsonl");
        let evento = |info: &str| format!("{{\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\",\"info\":{info}}}}}\n");
        fs::write(&r, format!("{}{}{}", evento("null"),
            evento(r#"{"total_token_usage":{"input_tokens":1000,"cached_input_tokens":600,"output_tokens":200,"reasoning_output_tokens":50,"total_tokens":1200}}"#),
            evento(r#"{"total_token_usage":{"input_tokens":3000,"cached_input_tokens":2000,"output_tokens":500,"reasoning_output_tokens":80,"total_tokens":3500}}"#))).unwrap();
        assert_eq!(da_conversa(&conversa("codex", &r)), Some(Tokens { total: 3500, entrada: 3000, cache: 2000, saida: 500 }));
    }

    #[test]
    fn sem_uso_ainda_nao_mostra_nada() {
        let dir = tempfile::tempdir().unwrap();
        let t = dir.path().join("s.jsonl");
        fs::write(&t, "{\"type\":\"user\"}\n").unwrap();
        assert_eq!(da_conversa(&conversa("claude", &t)), None);
    }
}
