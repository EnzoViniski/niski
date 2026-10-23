//! Fork (Rotina): o plano — áreas da vida (Faculdade, Medza, Aircook…), blocos fixos da semana
//! (aulas, trabalho, sono, academia, refeições), exceções de um dia (imprevistos), metas e foco de
//! cada semana, marcas de feito/pulado por dia e a memória que o agente da rotina guarda entre as
//! semanas. Compromissos com data e hora **não** moram aqui: são Tarefas.

use super::{chave, data, dia_da_semana, hora, lista, mapa, minutos, texto_da_data, Arquivo};
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const TIPOS_DE_BLOCO: &[&str] = &["aula", "trabalho", "estudo", "sono", "academia", "refeicao", "deslocamento", "pessoal", "outro"];
/// Quanto tempo de marcas e exceções o arquivo guarda (o resto é podado ao gravar).
pub const DIAS_GUARDADOS: i64 = 120;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Area {
    pub id: String,
    pub nome: String,
    /// Cor da área na grade (hex `#rrggbb`).
    pub cor: String,
    /// Categoria das Tarefas que cai nesta área (`trabalho`, `faculdade`…).
    pub categoria: String,
    /// Etiqueta das Tarefas que cai nesta área (vence a categoria): `medza`, `aircook`.
    pub etiqueta: String,
}

pub fn areas_padrao() -> Vec<Area> {
    let a = |id: &str, nome: &str, cor: &str, categoria: &str, etiqueta: &str| Area {
        id: id.into(), nome: nome.into(), cor: cor.into(), categoria: categoria.into(), etiqueta: etiqueta.into(),
    };
    vec![
        a("faculdade", "Faculdade", "#6366f1", "faculdade", ""),
        a("medza", "Medza", "#0ea5e9", "trabalho", "medza"),
        a("aircook", "Aircook", "#f97316", "trabalho", "aircook"),
        a("saude", "Saúde e treino", "#10b981", "saude", ""),
        a("pessoal", "Pessoal", "#a855f7", "pessoal", ""),
    ]
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Bloco {
    pub id: String,
    pub titulo: String,
    pub area: String,
    pub tipo: String,
    /// Dias da semana (0 = segunda … 6 = domingo). Ignorado quando `data` está preenchida.
    pub dias: Vec<u8>,
    pub inicio: String,
    pub fim: String,
    /// `AAAA-MM-DD`: bloco de um dia só (criado por imprevisto ou pelo plano da semana).
    pub data: String,
    pub lembrar: bool,
    pub local: String,
    pub notas: String,
}

/// Mudança num bloco fixo só numa data: cancelar ou mudar o horário.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Excecao {
    pub data: String,
    pub bloco: String,
    pub cancelado: bool,
    pub inicio: String,
    pub fim: String,
    pub nota: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Meta {
    pub id: String,
    pub texto: String,
    pub area: String,
    /// Alvo numérico opcional (ex.: 4 treinos, 10 páginas). Sem alvo: meta de sim/não.
    #[serde(deserialize_with = "super::numero_opcional")]
    pub alvo: Option<f64>,
    pub progresso: f64,
    pub unidade: String,
    pub concluida: bool,
}

impl Meta {
    /// 0 a 1.
    pub fn fracao(&self) -> f64 {
        if self.concluida {
            return 1.0;
        }
        match self.alvo {
            Some(alvo) if alvo > 0.0 => (self.progresso / alvo).clamp(0.0, 1.0),
            _ => 0.0,
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Imprevisto {
    pub em: String,
    pub data: String,
    pub texto: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Semana {
    pub foco: String,
    #[serde(deserialize_with = "lista")]
    pub metas: Vec<Meta>,
    pub notas: String,
    /// ISO de quando o planejamento da semana foi gravado (vazio = ainda não planejada).
    pub planejada_em: String,
    #[serde(deserialize_with = "lista")]
    pub imprevistos: Vec<Imprevisto>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Marcas {
    pub feitos: Vec<String>,
    pub pulados: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Plano {
    pub versao: u32,
    #[serde(deserialize_with = "lista")]
    pub areas: Vec<Area>,
    #[serde(deserialize_with = "lista")]
    pub blocos: Vec<Bloco>,
    #[serde(deserialize_with = "lista")]
    pub excecoes: Vec<Excecao>,
    /// Chave: a segunda-feira da semana (`AAAA-MM-DD`).
    #[serde(deserialize_with = "mapa")]
    pub semanas: BTreeMap<String, Semana>,
    /// Chave: a data. Itens: `bloco:<id>`, `refeicao:<id>`, `treino`, `agua`.
    #[serde(deserialize_with = "mapa")]
    pub marcas: BTreeMap<String, Marcas>,
    /// O que o agente da rotina aprendeu e quer lembrar nas próximas semanas (preferências, padrões).
    pub memoria: String,
}

impl Default for Plano {
    fn default() -> Self {
        Self { versao: super::VERSAO, areas: areas_padrao(), blocos: Vec::new(), excecoes: Vec::new(), semanas: BTreeMap::new(),
            marcas: BTreeMap::new(), memoria: String::new() }
    }
}

impl Arquivo for Plano {
    const NOME: &'static str = "plano.json";
    fn versao(&self) -> u32 { self.versao }
    fn marcar_versao(&mut self) { self.versao = super::VERSAO; }
}

/// Um bloco já resolvido para uma data (exceção aplicada).
#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct BlocoDoDia {
    pub id: String,
    pub titulo: String,
    pub area: String,
    pub tipo: String,
    pub inicio: String,
    pub fim: String,
    pub local: String,
    pub notas: String,
    pub lembrar: bool,
    /// Bloco só desta data (não é fixo da semana).
    pub avulso: bool,
    /// Horário mudado só hoje, com a nota do imprevisto.
    pub mudado: Option<String>,
}

impl Plano {
    pub fn area(&self, id: &str) -> Option<&Area> {
        self.areas.iter().find(|a| a.id == id)
    }

    pub fn semana(&self, segunda: NaiveDate) -> Semana {
        self.semanas.get(&texto_da_data(segunda)).cloned().unwrap_or_default()
    }

    pub fn semana_mut(&mut self, segunda: NaiveDate) -> &mut Semana {
        self.semanas.entry(texto_da_data(segunda)).or_default()
    }

    /// Blocos da data, em ordem de início. Cancelados ficam de fora.
    pub fn blocos_do_dia(&self, dia: NaiveDate) -> Vec<BlocoDoDia> {
        let texto = texto_da_data(dia);
        let semana = dia_da_semana(dia);
        let mut blocos: Vec<BlocoDoDia> = self.blocos.iter()
            .filter(|b| if b.data.is_empty() { b.dias.contains(&semana) } else { b.data == texto })
            .filter_map(|b| {
                let excecao = self.excecoes.iter().rev().find(|e| e.data == texto && e.bloco == b.id);
                if excecao.is_some_and(|e| e.cancelado) {
                    return None;
                }
                let (inicio, fim, mudado) = match excecao {
                    Some(e) => (
                        if e.inicio.is_empty() { b.inicio.clone() } else { e.inicio.clone() },
                        if e.fim.is_empty() { b.fim.clone() } else { e.fim.clone() },
                        Some(e.nota.clone()),
                    ),
                    None => (b.inicio.clone(), b.fim.clone(), None),
                };
                Some(BlocoDoDia { id: b.id.clone(), titulo: b.titulo.clone(), area: b.area.clone(), tipo: b.tipo.clone(), inicio, fim,
                    local: b.local.clone(), notas: b.notas.clone(), lembrar: b.lembrar, avulso: !b.data.is_empty(), mudado })
            })
            .collect();
        blocos.sort_by_key(|b| hora(&b.inicio).map(minutos).unwrap_or(u32::MAX));
        blocos
    }

    /// Cancelados da data (para a tela mostrar riscados, se quiser).
    pub fn cancelados_do_dia(&self, dia: NaiveDate) -> Vec<String> {
        let texto = texto_da_data(dia);
        self.excecoes.iter().filter(|e| e.data == texto && e.cancelado).map(|e| e.bloco.clone()).collect()
    }

    pub fn marcas_do_dia(&self, dia: NaiveDate) -> Marcas {
        self.marcas.get(&texto_da_data(dia)).cloned().unwrap_or_default()
    }

    /// `estado`: `feito`, `pulado` ou `nada` (desfaz).
    pub fn marcar(&mut self, dia: NaiveDate, item: &str, estado: &str) -> Result<(), String> {
        if item.is_empty() || item.len() > 120 {
            return Err("Item inválido para marcar".into());
        }
        let marcas = self.marcas.entry(texto_da_data(dia)).or_default();
        marcas.feitos.retain(|i| i != item);
        marcas.pulados.retain(|i| i != item);
        match estado {
            "feito" => marcas.feitos.push(item.to_owned()),
            "pulado" => marcas.pulados.push(item.to_owned()),
            "nada" => {}
            _ => return Err("`estado` é feito, pulado ou nada".into()),
        }
        Ok(())
    }

    /// Área de uma tarefa (JSON das Tarefas): etiqueta vence categoria.
    pub fn area_da_tarefa(&self, tarefa: &serde_json::Value) -> Option<String> {
        let etiquetas: Vec<String> = tarefa.get("etiquetas").and_then(|e| e.as_array())
            .map(|l| l.iter().filter_map(|e| e.as_str()).map(chave).collect()).unwrap_or_default();
        if let Some(a) = self.areas.iter().find(|a| !a.etiqueta.is_empty() && etiquetas.contains(&chave(&a.etiqueta))) {
            return Some(a.id.clone());
        }
        let categoria = tarefa.get("categoria").and_then(|c| c.as_str()).unwrap_or("");
        self.areas.iter().find(|a| a.etiqueta.is_empty() && !a.categoria.is_empty() && a.categoria == categoria).map(|a| a.id.clone())
    }

    /// Tira marcas e exceções mais velhas que [`DIAS_GUARDADOS`] e blocos avulsos vencidos.
    pub fn podar(&mut self, hoje: NaiveDate) {
        let limite = texto_da_data(hoje - Duration::days(DIAS_GUARDADOS));
        self.marcas.retain(|d, _| d.as_str() >= limite.as_str());
        self.excecoes.retain(|e| e.data.as_str() >= limite.as_str());
        self.blocos.retain(|b| b.data.is_empty() || b.data.as_str() >= limite.as_str());
    }
}

/// A semana já foi planejada?
pub fn planejada(plano: &Plano, segunda: NaiveDate) -> bool {
    !plano.semana(segunda).planejada_em.is_empty()
}

pub fn data_valida(texto: &str) -> bool {
    data(texto).is_some()
}
