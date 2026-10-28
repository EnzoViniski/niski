//! Teto de sessões **ativas** neste PC (celular, retomar, bifurcar, terminal da web e
//! subagentes do orquestrador passam por ele; o engine conta em `omni-engine/src/limite_de_sessoes.rs`).
//!
//! Só contam sessões de agente e terminais com um comando rodando em primeiro plano: um shell parado
//! no prompt não gasta nada e não deve impedir o orquestrador de abrir subagentes.
//!
//! O número fica em `sessoes.json` na pasta do engine (`{"limite": 32}`), editável em
//! Configurações → Geral → Sessões. O engine relê o arquivo a cada conferência: mudar vale na hora.

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const ARQUIVO: &str = "sessoes.json";
/// Sem arquivo (ou valor inválido): 32. O antigo teto fixo era 16 e contava até shell parado.
pub const PADRAO: usize = 32;
pub const MINIMO: usize = 4;
pub const MAXIMO: usize = 128;

#[derive(Debug, Default, Serialize, Deserialize)]
struct Config {
    #[serde(default)]
    limite: Option<usize>,
}

/// Limite em vigor (o padrão se não houver arquivo; valor fora da faixa é trazido para dentro dela).
pub fn ler(dir: &Path) -> usize {
    std::fs::read(dir.join(ARQUIVO)).ok()
        .and_then(|bytes| serde_json::from_slice::<Config>(&bytes).ok())
        .and_then(|config| config.limite)
        .map(|limite| limite.clamp(MINIMO, MAXIMO))
        .unwrap_or(PADRAO)
}

/// Grava o limite escolhido. Fora da faixa é erro (a tela mostra a faixa), não ajuste silencioso.
pub fn gravar(dir: &Path, limite: usize) -> Result<usize, String> {
    if !(MINIMO..=MAXIMO).contains(&limite) {
        return Err(format!("O limite de sessões vai de {MINIMO} a {MAXIMO}"));
    }
    let caminho = dir.join(ARQUIVO);
    let temporario = caminho.with_extension("json.tmp");
    let corpo = serde_json::to_vec_pretty(&Config { limite: Some(limite) }).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    std::fs::write(&temporario, corpo).and_then(|_| std::fs::rename(&temporario, &caminho)).map_err(|e| e.to_string())?;
    Ok(limite)
}

/// O que está ocupando vaga agora.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Contagem {
    /// Sessões com uma CLI de agente (Claude, Codex…).
    pub agentes: usize,
    /// Terminais sem agente com um comando em primeiro plano (`npm run dev`, um build…).
    pub terminais_ocupados: usize,
}

impl Contagem {
    pub fn total(&self) -> usize { self.agentes + self.terminais_ocupados }
}

/// `Err(mensagem)` quando abrir mais uma passaria do limite. A mensagem diz quantas há e como liberar.
pub fn conferir(contagem: Contagem, limite: usize) -> Result<(), String> {
    if contagem.total() < limite { return Ok(()); }
    Err(mensagem(contagem, limite))
}

pub fn mensagem(contagem: Contagem, limite: usize) -> String {
    let agentes = match contagem.agentes { 1 => "1 agente".to_owned(), n => format!("{n} agentes") };
    let terminais = match contagem.terminais_ocupados {
        0 => String::new(),
        1 => " e 1 terminal rodando um comando".to_owned(),
        n => format!(" e {n} terminais rodando comandos"),
    };
    format!("Limite de {limite} sessões ativas neste PC atingido ({agentes}{terminais}). Para liberar, encerre agentes \
        que já terminaram (× na lateral, ou parar_agente no orquestrador) ou pare o comando de algum terminal. \
        Terminal parado no prompt não conta. O limite muda em Configurações → Geral → Sessões.")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn padrao_sem_arquivo_e_faixa_ao_gravar() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(ler(dir.path()), PADRAO);
        assert_eq!(gravar(dir.path(), 48).unwrap(), 48);
        assert_eq!(ler(dir.path()), 48);
        assert!(gravar(dir.path(), 2).is_err());
        assert!(gravar(dir.path(), MAXIMO + 1).is_err());
        assert_eq!(ler(dir.path()), 48, "valor inválido não sobrescreve");
        // Arquivo editado à mão fora da faixa: trazido para dentro, nunca zero.
        std::fs::write(dir.path().join(ARQUIVO), r#"{"limite": 0}"#).unwrap();
        assert_eq!(ler(dir.path()), MINIMO);
        std::fs::write(dir.path().join(ARQUIVO), "lixo").unwrap();
        assert_eq!(ler(dir.path()), PADRAO);
    }

    #[test]
    fn so_conta_agente_e_terminal_ocupado_e_mensagem_diz_como_liberar() {
        assert!(conferir(Contagem { agentes: 3, terminais_ocupados: 0 }, 4).is_ok());
        let erro = conferir(Contagem { agentes: 3, terminais_ocupados: 1 }, 4).unwrap_err();
        assert!(erro.contains("Limite de 4"), "{erro}");
        assert!(erro.contains("3 agentes e 1 terminal rodando um comando"), "{erro}");
        assert!(erro.contains("Configurações"), "{erro}");
        let erro = conferir(Contagem { agentes: 1, terminais_ocupados: 0 }, 1).unwrap_err();
        assert!(erro.contains("(1 agente)"), "{erro}");
    }
}
