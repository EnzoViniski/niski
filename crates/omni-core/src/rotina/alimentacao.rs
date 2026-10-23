//! Fork (Rotina): alimentação — plano de refeições do dia, refeições registradas em texto livre
//! (kcal e proteína opcionais) e água com meta diária.

use super::{lista, mapa, texto_da_data, Arquivo};
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const DIAS_GUARDADOS: i64 = 400;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct RefeicaoPlanejada {
    pub id: String,
    pub nome: String,
    pub hora: String,
    /// O que comer (texto livre).
    pub sugestao: String,
    #[serde(deserialize_with = "super::numero_opcional")]
    pub kcal: Option<f64>,
    #[serde(deserialize_with = "super::numero_opcional")]
    pub proteina_g: Option<f64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Refeicao {
    pub id: String,
    pub hora: String,
    pub nome: String,
    pub texto: String,
    #[serde(deserialize_with = "super::numero_opcional")]
    pub kcal: Option<f64>,
    #[serde(deserialize_with = "super::numero_opcional")]
    pub proteina_g: Option<f64>,
    /// Id da refeição do plano que esta cumpre (vazio = extra).
    pub planejada: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DiaAlimentar {
    #[serde(deserialize_with = "lista")]
    pub refeicoes: Vec<Refeicao>,
    pub agua_ml: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Alimentacao {
    pub versao: u32,
    #[serde(deserialize_with = "lista")]
    pub plano: Vec<RefeicaoPlanejada>,
    pub meta_agua_ml: u32,
    pub copo_ml: u32,
    #[serde(deserialize_with = "super::numero_opcional")]
    pub meta_kcal: Option<f64>,
    #[serde(deserialize_with = "super::numero_opcional")]
    pub meta_proteina_g: Option<f64>,
    #[serde(deserialize_with = "mapa")]
    pub dias: BTreeMap<String, DiaAlimentar>,
}

pub fn plano_padrao() -> Vec<RefeicaoPlanejada> {
    let r = |id: &str, nome: &str, hora: &str| RefeicaoPlanejada { id: id.into(), nome: nome.into(), hora: hora.into(), ..Default::default() };
    vec![r("cafe", "Café da manhã", "07:30"), r("almoco", "Almoço", "12:30"), r("lanche", "Lanche", "16:00"), r("jantar", "Jantar", "20:00")]
}

impl Default for Alimentacao {
    fn default() -> Self {
        Self { versao: super::VERSAO, plano: plano_padrao(), meta_agua_ml: 2500, copo_ml: 250, meta_kcal: None, meta_proteina_g: None, dias: BTreeMap::new() }
    }
}

impl Arquivo for Alimentacao {
    const NOME: &'static str = "alimentacao.json";
    fn versao(&self) -> u32 { self.versao }
    fn marcar_versao(&mut self) { self.versao = super::VERSAO; }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Totais {
    pub kcal: f64,
    pub proteina_g: f64,
    pub refeicoes: usize,
    pub agua_ml: u32,
}

impl Alimentacao {
    pub fn dia(&self, d: NaiveDate) -> DiaAlimentar {
        self.dias.get(&texto_da_data(d)).cloned().unwrap_or_default()
    }

    pub fn dia_mut(&mut self, d: NaiveDate) -> &mut DiaAlimentar {
        self.dias.entry(texto_da_data(d)).or_default()
    }

    pub fn totais(&self, d: NaiveDate) -> Totais {
        let dia = self.dia(d);
        Totais {
            kcal: dia.refeicoes.iter().filter_map(|r| r.kcal).sum(),
            proteina_g: dia.refeicoes.iter().filter_map(|r| r.proteina_g).sum(),
            refeicoes: dia.refeicoes.len(),
            agua_ml: dia.agua_ml,
        }
    }

    /// Soma (ou tira, com `ml` negativo) água no dia. Nunca abaixo de zero nem acima de 20 L.
    pub fn somar_agua(&mut self, d: NaiveDate, ml: i64) -> u32 {
        let dia = self.dia_mut(d);
        dia.agua_ml = (i64::from(dia.agua_ml) + ml).clamp(0, 20_000) as u32;
        dia.agua_ml
    }

    pub fn podar(&mut self, hoje: NaiveDate) {
        let limite = texto_da_data(hoje - Duration::days(DIAS_GUARDADOS));
        self.dias.retain(|d, _| d.as_str() >= limite.as_str());
    }
}
