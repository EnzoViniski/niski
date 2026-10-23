//! Fork (desempenho, 2026-10-22): a entrada do histórico de um transcript do Claude lida aos pedaços.
//!
//! O transcript só cresce (o Claude acrescenta linhas no fim). Antes, cada mudança relia o arquivo
//! inteiro para achar o título: um agente trabalhando muda o arquivo a cada poucos segundos, e
//! transcripts de 20–45 MB eram relidos por inteiro a cada varredura (lateral ao voltar à janela,
//! lista do celular, timeline do histórico) — duas vezes, porque o arquivo de conversas guarda uma
//! cópia. Agora a varredura guarda até onde leu e o que já achou, e lê só o que entrou depois.
//!
//! Mesmo resultado de `claude_entry` (teste `igual_a_ler_tudo_a_cada_pedaco`). Arquivo que encolheu ou
//! trocou de começo (não é o mesmo transcript) é relido do zero. A última linha sem `\n` (o Claude
//! escrevendo) é relida na próxima leitura.
use super::{blank, message, preview, HistoryEntry};
use crate::{conversations::timestamp, Profile};
use serde_json::Value;
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    path::Path,
};

/// Bytes do começo guardados para saber se ainda é o mesmo arquivo.
const ASSINATURA_DO_INICIO: usize = 256;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct LeituraDoClaude {
    /// Bytes já lidos: sempre o fim de uma linha completa.
    lido_ate: u64,
    inicio: Vec<u8>,
    title: Option<String>,
    summary: Option<String>,
    first_prompt: Option<String>,
    cwd: Option<String>,
    started_at_ms: Option<u64>,
    /// Linha com UTF-8 inválido: `claude_entry` para de ler ali, então aqui também.
    parou: bool,
}

fn inicio_do_arquivo(file: &mut File) -> Option<Vec<u8>> {
    let mut inicio = Vec::with_capacity(ASSINATURA_DO_INICIO);
    file.by_ref().take(ASSINATURA_DO_INICIO as u64).read_to_end(&mut inicio).ok()?;
    Some(inicio)
}

/// Continua a leitura de `anterior` (ou começa do zero) e devolve a entrada e o estado novo.
pub fn ler_claude(path: &Path, profile: &Profile, anterior: Option<&LeituraDoClaude>) -> (Option<HistoryEntry>, Option<LeituraDoClaude>) {
    let Ok(mut file) = File::open(path) else { return (None, None) };
    let Some(inicio) = inicio_do_arquivo(&mut file) else { return (None, None) };
    let tamanho = file.metadata().map(|m| m.len()).unwrap_or(0);
    // O arquivo só cresce: o começo guardado tem de continuar sendo o começo dele.
    let mut leitura = anterior.filter(|a| a.lido_ate <= tamanho && inicio.starts_with(&a.inicio)).cloned().unwrap_or_default();
    leitura.inicio = inicio;
    if !leitura.parou && file.seek(SeekFrom::Start(leitura.lido_ate)).is_ok() {
        let mut leitor = BufReader::new(file);
        let mut linha = Vec::new();
        loop {
            linha.clear();
            let Ok(lidos) = leitor.read_until(b'\n', &mut linha) else { break };
            if lidos == 0 { break }
            let completa = linha.last() == Some(&b'\n');
            let corpo = if completa { &linha[..linha.len() - 1] } else { &linha[..] };
            let Ok(texto) = std::str::from_utf8(corpo) else { if completa { leitura.parou = true; } break };
            let texto = texto.strip_suffix('\r').unwrap_or(texto);
            observar(&mut leitura, texto);
            // Última linha sem `\n` conta como `claude_entry` conta, mas é relida na próxima vez
            // (pode estar pela metade). Reler a mesma linha não muda o resultado.
            if !completa { break }
            leitura.lido_ate += lidos as u64;
        }
    }
    let mut entry = blank("claude", profile, match path.file_stem() { Some(s) => s.to_string_lossy().into_owned(), None => return (None, None) }, path);
    entry.title = leitura.title.clone().or_else(|| leitura.summary.clone());
    entry.first_prompt = leitura.first_prompt.clone();
    entry.cwd = leitura.cwd.clone();
    entry.started_at_ms = leitura.started_at_ms;
    (entry.first_prompt.is_some().then_some(entry), Some(leitura))
}

/// O mesmo que o laço de `claude_entry` faz com cada linha.
fn observar(leitura: &mut LeituraDoClaude, line: &str) {
    if line.contains("\"type\":\"ai-title\"") {
        let title = serde_json::from_str::<Value>(line).ok().and_then(|value| value["aiTitle"].as_str().map(str::to_owned));
        if title.is_some() { leitura.title = title; }
    } else if line.contains("\"type\":\"summary\"") {
        leitura.summary = serde_json::from_str::<Value>(line).ok().and_then(|value| value["summary"].as_str().map(str::to_owned));
    } else if leitura.first_prompt.is_none() && line.contains("\"type\":\"user\"") {
        let Ok(value) = serde_json::from_str::<Value>(line) else { return };
        let Some(prompt) = message(&value, "claude").filter(|message| message.role == "user") else { return };
        leitura.first_prompt = Some(preview(&prompt.text));
        leitura.cwd = value["cwd"].as_str().map(str::to_owned);
        leitura.started_at_ms = timestamp(&value["timestamp"]);
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::history::claude_entry;
    use serde_json::json;
    use std::io::Write;

    fn perfil() -> Profile {
        serde_json::from_value(json!({"id":"p","provider":"claude","name":"Padrão","config_dir":"/tmp","created_at_ms":0})).unwrap()
    }

    fn linha(v: Value) -> String { format!("{v}\n") }

    fn acrescentar(path: &Path, texto: &str) {
        std::fs::OpenOptions::new().create(true).append(true).open(path).unwrap().write_all(texto.as_bytes()).unwrap();
    }

    #[test] fn igual_a_ler_tudo_a_cada_pedaco() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s1.jsonl");
        let pedacos = [
            linha(json!({"type":"system","content":"x"})),
            linha(json!({"type":"user","cwd":"/repo","timestamp":"2026-10-22T10:00:00Z","message":{"role":"user","content":"conserte o login"}})),
            linha(json!({"type":"summary","summary":"Resumo velho"})),
            linha(json!({"type":"ai-title","aiTitle":"Login quebrado"})),
            linha(json!({"type":"user","message":{"role":"user","content":"outra coisa"}})),
            linha(json!({"type":"ai-title","aiTitle":"Login consertado"})),
        ];
        let mut estado = None;
        for pedaco in &pedacos {
            acrescentar(&path, pedaco);
            let (entrada, novo) = ler_claude(&path, &perfil(), estado.as_ref());
            assert_eq!(entrada, claude_entry(&path, &perfil()), "depois de {pedaco}");
            estado = novo;
        }
        assert_eq!(estado.as_ref().unwrap().lido_ate, std::fs::metadata(&path).unwrap().len());
    }

    #[test] fn linha_pela_metade_fica_para_depois() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s2.jsonl");
        let usuario = linha(json!({"type":"user","message":{"role":"user","content":"oi"}}));
        acrescentar(&path, &usuario);
        let titulo = linha(json!({"type":"ai-title","aiTitle":"Saudação"}));
        let (metade, resto) = titulo.split_at(10);
        acrescentar(&path, metade);
        let (entrada, estado) = ler_claude(&path, &perfil(), None);
        assert_eq!(entrada, claude_entry(&path, &perfil()));
        assert_eq!(entrada.unwrap().title, None);
        assert_eq!(estado.as_ref().unwrap().lido_ate, usuario.len() as u64);
        acrescentar(&path, resto);
        let (entrada, _) = ler_claude(&path, &perfil(), estado.as_ref());
        assert_eq!(entrada.unwrap().title.as_deref(), Some("Saudação"));
    }

    #[test] fn arquivo_trocado_ou_menor_e_relido_do_zero() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s3.jsonl");
        acrescentar(&path, &linha(json!({"type":"user","message":{"role":"user","content":"primeiro arquivo, bem comprido para passar do outro"}})));
        let (_, estado) = ler_claude(&path, &perfil(), None);
        std::fs::write(&path, linha(json!({"type":"user","message":{"role":"user","content":"outro"}}))).unwrap();
        let (entrada, _) = ler_claude(&path, &perfil(), estado.as_ref());
        assert_eq!(entrada, claude_entry(&path, &perfil()));
        assert_eq!(entrada.unwrap().first_prompt.as_deref(), Some("outro"));
    }

    #[test] fn sem_prompt_digitado_fica_de_fora() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("s4.jsonl");
        acrescentar(&path, &linha(json!({"type":"ai-title","aiTitle":"Só título"})));
        assert_eq!(ler_claude(&path, &perfil(), None).0, None);
        assert_eq!(claude_entry(&path, &perfil()), None);
    }
}
