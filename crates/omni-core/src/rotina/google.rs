//! Fork (Google Agenda): os compromissos do Google Agenda pessoal dentro da Rotina.
//!
//! O engine busca os eventos (`omni-engine/src/mobile/google_agenda.rs`) e guarda aqui uma cópia
//! (`google-agenda.json`): a montagem do dia (`dia.rs`) mostra cada evento como item `compromisso`,
//! então Hoje, Semana, celular, Telegram e o agente da rotina enxergam os mesmos compromissos.
//! Nada deste arquivo fala com a rede nem guarda credencial.

use super::{data, lista, texto_da_data, Arquivo};
use chrono::{DateTime, Duration, NaiveDate, TimeZone};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Um evento já no fuso local, um por dia (evento de vários dias vira uma entrada por dia).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Evento {
    pub id: String,
    pub titulo: String,
    /// `AAAA-MM-DD` local.
    pub data: String,
    /// `HH:MM` local; vazio no evento de dia inteiro.
    pub inicio: String,
    pub fim: String,
    pub dia_todo: bool,
    pub local: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct AgendaDoGoogle {
    pub versao: u32,
    /// Nome da agenda principal (o e-mail da conta), para a tela dizer qual conta está ligada.
    pub conta: String,
    pub sincronizado_ms: i64,
    /// Última falha da sincronização (texto curto para a tela), vazio quando deu certo.
    pub erro: String,
    #[serde(deserialize_with = "lista")]
    pub eventos: Vec<Evento>,
}

impl Default for AgendaDoGoogle {
    fn default() -> Self {
        Self { versao: super::VERSAO, conta: String::new(), sincronizado_ms: 0, erro: String::new(), eventos: Vec::new() }
    }
}

impl Arquivo for AgendaDoGoogle {
    const NOME: &'static str = "google-agenda.json";
    fn versao(&self) -> u32 { self.versao }
    fn marcar_versao(&mut self) { self.versao = super::VERSAO; }
}

/// Evento de dia inteiro longo (férias, viagem) não vira centenas de entradas.
const DIAS_MAXIMOS_POR_EVENTO: i64 = 31;

impl AgendaDoGoogle {
    pub fn do_dia(&self, dia: NaiveDate) -> impl Iterator<Item = &Evento> {
        let texto = texto_da_data(dia);
        self.eventos.iter().filter(move |e| e.data == texto)
    }
}

/// Chave do item no dia (a mesma das marcas de feito/pulado), com até 120 caracteres.
pub fn chave_do_item(e: &Evento) -> String {
    if e.id.len() <= 100 {
        return format!("google:{}", e.id);
    }
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    e.id.hash(&mut h);
    format!("google:{:016x}", h.finish())
}

fn texto(v: &Value, campo: &str, max: usize) -> String {
    let t = v[campo].as_str().unwrap_or("").trim();
    t.chars().take(max).collect()
}

/// Converte os `items` da API (`events.list` com `singleEvents=true`) para o fuso local.
/// Cancelados e eventos sem início válido ficam de fora.
pub fn eventos_de<Tz: TimeZone>(itens: &[Value], fuso: &Tz) -> Vec<Evento> {
    let mut eventos = Vec::new();
    for item in itens {
        if item["status"].as_str() == Some("cancelled") {
            continue;
        }
        let id = texto(item, "id", 300);
        if id.is_empty() {
            continue;
        }
        let titulo = Some(texto(item, "summary", 200)).filter(|t| !t.is_empty()).unwrap_or_else(|| "(sem título)".into());
        let local = texto(item, "location", 200);
        let base = Evento { id, titulo, local, ..Default::default() };
        if let Some(inicio) = item["start"]["date"].as_str().and_then(data) {
            // Dia inteiro: `end.date` é exclusivo.
            let fim = item["end"]["date"].as_str().and_then(data).filter(|f| *f > inicio).unwrap_or(inicio + Duration::days(1));
            let dias = (fim - inicio).num_days().clamp(1, DIAS_MAXIMOS_POR_EVENTO);
            for n in 0..dias {
                eventos.push(Evento { data: texto_da_data(inicio + Duration::days(n)), dia_todo: true, ..base.clone() });
            }
            continue;
        }
        let Some(inicio) = item["start"]["dateTime"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok()) else { continue };
        let inicio = inicio.with_timezone(fuso);
        let fim = item["end"]["dateTime"].as_str().and_then(|t| DateTime::parse_from_rfc3339(t).ok()).map(|f| f.with_timezone(fuso));
        let dia = inicio.date_naive();
        let fim = match fim {
            Some(f) if f.date_naive() == dia => f.naive_local().format("%H:%M").to_string(),
            Some(f) if f.date_naive() > dia => "23:59".into(), // atravessa a meia-noite: fica no dia em que começa
            _ => String::new(),
        };
        eventos.push(Evento { data: texto_da_data(dia), inicio: inicio.naive_local().format("%H:%M").to_string(), fim, ..base });
    }
    eventos
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;

    fn brasilia() -> chrono::FixedOffset {
        chrono::FixedOffset::west_opt(3 * 3600).unwrap()
    }

    #[test]
    fn converte_hora_para_o_fuso_local_e_ignora_cancelados() {
        let itens = vec![
            json!({"id": "a", "summary": "Dentista", "location": "Centro", "start": {"dateTime": "2026-10-27T17:00:00Z"}, "end": {"dateTime": "2026-10-27T18:00:00Z"}}),
            json!({"id": "b", "status": "cancelled", "start": {"dateTime": "2026-10-27T17:00:00Z"}}),
            json!({"id": "c", "start": {"dateTime": "2026-10-28T23:00:00-03:00"}, "end": {"dateTime": "2026-10-29T01:00:00-03:00"}}),
            json!({"summary": "sem id", "start": {"dateTime": "2026-10-28T10:00:00-03:00"}}),
        ];
        let e = eventos_de(&itens, &brasilia());
        assert_eq!(e.len(), 2);
        assert_eq!((e[0].data.as_str(), e[0].inicio.as_str(), e[0].fim.as_str(), e[0].local.as_str()), ("2026-10-27", "14:00", "15:00", "Centro"));
        assert_eq!((e[1].titulo.as_str(), e[1].fim.as_str()), ("(sem título)", "23:59"));
    }

    #[test]
    fn dia_inteiro_vira_uma_entrada_por_dia_com_limite() {
        let itens = vec![
            json!({"id": "v", "summary": "Viagem", "start": {"date": "2026-10-30"}, "end": {"date": "2026-11-02"}}),
            json!({"id": "f", "summary": "Férias", "start": {"date": "2026-12-01"}, "end": {"date": "2027-03-01"}}),
        ];
        let e = eventos_de(&itens, &brasilia());
        let viagem: Vec<&str> = e.iter().filter(|x| x.id == "v").map(|x| x.data.as_str()).collect();
        assert_eq!(viagem, ["2026-10-30", "2026-10-31", "2026-11-01"]);
        assert!(e.iter().all(|x| x.dia_todo && x.inicio.is_empty()));
        assert_eq!(e.iter().filter(|x| x.id == "f").count(), DIAS_MAXIMOS_POR_EVENTO as usize);
    }

    #[test]
    fn chave_do_item_cabe_nas_marcas() {
        let curto = Evento { id: "abc_20261027T170000Z".into(), ..Default::default() };
        assert_eq!(chave_do_item(&curto), "google:abc_20261027T170000Z");
        let longo = Evento { id: "x".repeat(300), ..Default::default() };
        let chave = chave_do_item(&longo);
        assert!(chave.len() <= 120 && chave.starts_with("google:"));
        assert_eq!(chave, chave_do_item(&longo), "estável");
    }
}
