//! Fork (Rotina): ajustes do agente da rotina (plataforma, conta, modelo, esforço), qual conversa é a
//! da semana e as preferências do Telegram da rotina. O token do bot **não** mora aqui: fica no
//! arquivo próprio do engine (`rotina/telegram-rotina.json`, modo 0600).

use super::{mapa, Arquivo};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const MODOS_DE_CONVERSA: &[&str] = &["semanal", "continua"];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Ajustes {
    pub versao: u32,
    /// `claude` ou `codex` (as que aceitam MCP só para a sessão). Vazio = a do copiloto.
    pub plataforma: String,
    /// Id da conta (perfil). Vazio = a do copiloto ou a padrão da plataforma.
    pub conta: String,
    pub modelo: String,
    pub esforco: String,
    /// `semanal` (padrão): uma conversa por semana, a continuidade vem dos dados e da memória.
    /// `continua`: a mesma conversa sempre (retomada com `--resume`).
    pub modo_conversa: String,
    /// Segunda-feira da semana → id da conversa do agente da rotina daquela semana.
    #[serde(deserialize_with = "mapa")]
    pub conversas: BTreeMap<String, String>,
    /// Lembretes com o texto vão pelo bot da rotina (só com o bot da rotina conectado).
    pub telegram_lembretes: bool,
    /// Texto mandado ao bot da rotina vai para o agente da rotina e a resposta volta pelo Telegram.
    pub telegram_conversa: bool,
}

impl Default for Ajustes {
    fn default() -> Self {
        Self { versao: super::VERSAO, plataforma: String::new(), conta: String::new(), modelo: String::new(), esforco: String::new(),
            modo_conversa: "semanal".into(), conversas: BTreeMap::new(), telegram_lembretes: true, telegram_conversa: false }
    }
}

impl Arquivo for Ajustes {
    const NOME: &'static str = "ajustes.json";
    fn versao(&self) -> u32 { self.versao }
    fn marcar_versao(&mut self) { self.versao = super::VERSAO; }
}

impl Ajustes {
    /// A conversa que vale para a semana de `segunda`: no modo contínuo, a mais recente de todas.
    pub fn conversa_da_semana(&self, segunda: &str) -> Option<String> {
        if self.modo_conversa == "continua" {
            return self.conversas.values().next_back().cloned();
        }
        self.conversas.get(segunda).cloned()
    }

    /// Guarda a conversa da semana (mantém as 60 mais recentes).
    pub fn definir_conversa(&mut self, segunda: &str, conversa: &str) {
        self.conversas.insert(segunda.to_owned(), conversa.to_owned());
        while self.conversas.len() > 60 {
            let primeira = self.conversas.keys().next().cloned().unwrap_or_default();
            self.conversas.remove(&primeira);
        }
    }
}
