//! Fork (Rotina): treino da academia — fichas (A/B/C…), sessões com séries (carga × repetições),
//! histórico, evolução por exercício (carga máxima, volume, 1RM estimado, recordes) e frequência.

use super::{chave, data, lista, segunda_de, texto_da_data, Arquivo};
use chrono::{Duration, NaiveDate};
use serde::{Deserialize, Serialize};

/// Sessões guardadas (as mais velhas saem ao gravar). ~5 anos de 5 treinos por semana.
pub const SESSOES_MAXIMAS: usize = 1500;

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ExercicioDaFicha {
    pub nome: String,
    pub series: u32,
    /// Texto livre: `8-12`, `10`, `até a falha`.
    pub repeticoes: String,
    #[serde(deserialize_with = "super::numero_opcional")]
    pub carga_kg: Option<f64>,
    pub descanso_s: u32,
    pub obs: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Ficha {
    pub id: String,
    /// Nome curto: `A`, `B`, `C`.
    pub nome: String,
    /// `Peito e tríceps`.
    pub titulo: String,
    #[serde(deserialize_with = "lista")]
    pub exercicios: Vec<ExercicioDaFicha>,
    /// Dias sugeridos (0 = segunda … 6 = domingo).
    pub dias: Vec<u8>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Serie {
    pub exercicio: String,
    pub carga_kg: f64,
    pub repeticoes: u32,
    pub obs: String,
    /// `HH:MM` em que a série foi registrada.
    pub em: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Sessao {
    pub id: String,
    pub data: String,
    /// Id da ficha (vazio = treino livre).
    pub ficha: String,
    pub inicio: String,
    pub fim: String,
    #[serde(deserialize_with = "lista")]
    pub series: Vec<Serie>,
    pub obs: String,
    pub concluida: bool,
}

impl Sessao {
    pub fn volume(&self) -> f64 {
        self.series.iter().map(|s| s.carga_kg * f64::from(s.repeticoes)).sum()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Treino {
    pub versao: u32,
    #[serde(deserialize_with = "lista")]
    pub fichas: Vec<Ficha>,
    #[serde(deserialize_with = "lista")]
    pub sessoes: Vec<Sessao>,
    /// Treinos por semana que o usuário quer fazer.
    pub meta_semanal: u32,
}

impl Default for Treino {
    fn default() -> Self {
        Self { versao: super::VERSAO, fichas: Vec::new(), sessoes: Vec::new(), meta_semanal: 4 }
    }
}

impl Arquivo for Treino {
    const NOME: &'static str = "treino.json";
    fn versao(&self) -> u32 { self.versao }
    fn marcar_versao(&mut self) { self.versao = super::VERSAO; }
}

/// 1RM estimado (Epley). Repetição única é a própria carga.
pub fn um_rm(carga: f64, repeticoes: u32) -> f64 {
    if repeticoes <= 1 { carga } else { carga * (1.0 + f64::from(repeticoes) / 30.0) }
}

fn arredondar(n: f64) -> f64 {
    (n * 10.0).round() / 10.0
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Ponto {
    pub data: String,
    pub carga_max: f64,
    pub volume: f64,
    pub repeticoes_max: u32,
    pub um_rm: f64,
    pub series: usize,
    /// Esta sessão bateu o recorde de carga (ou de 1RM) que existia antes dela.
    pub recorde: bool,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Evolucao {
    pub exercicio: String,
    pub pontos: Vec<Ponto>,
    pub recorde_carga: f64,
    pub recorde_um_rm: f64,
    pub recorde_volume: f64,
    /// Séries da última vez (para preencher o registro rápido).
    pub ultima_vez: Vec<Serie>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct ResumoDoExercicio {
    pub nome: String,
    pub sessoes: usize,
    pub ultima_data: String,
    pub carga_max: f64,
    pub um_rm: f64,
}

impl Treino {
    pub fn ficha(&self, id: &str) -> Option<&Ficha> {
        self.fichas.iter().find(|f| f.id == id || chave(&f.nome) == chave(id))
    }

    /// Sessões em ordem cronológica.
    pub fn ordenadas(&self) -> Vec<&Sessao> {
        let mut s: Vec<&Sessao> = self.sessoes.iter().collect();
        s.sort_by(|a, b| (a.data.as_str(), a.inicio.as_str()).cmp(&(b.data.as_str(), b.inicio.as_str())));
        s
    }

    pub fn sessao_do_dia(&self, dia: NaiveDate) -> Option<&Sessao> {
        let texto = texto_da_data(dia);
        self.sessoes.iter().rev().find(|s| s.data == texto)
    }

    /// A ficha seguinte na rotação (depois da última sessão com ficha), ou a do dia da semana.
    pub fn proxima_ficha(&self, dia: NaiveDate) -> Option<&Ficha> {
        if self.fichas.is_empty() {
            return None;
        }
        let semana = super::dia_da_semana(dia);
        if let Some(f) = self.fichas.iter().find(|f| f.dias.contains(&semana)) {
            return Some(f);
        }
        let ultima = self.ordenadas().into_iter().rev().find(|s| !s.ficha.is_empty() && s.data.as_str() < texto_da_data(dia).as_str());
        let posicao = ultima.and_then(|s| self.fichas.iter().position(|f| f.id == s.ficha));
        Some(&self.fichas[posicao.map(|p| (p + 1) % self.fichas.len()).unwrap_or(0)])
    }

    pub fn evolucao(&self, exercicio: &str) -> Evolucao {
        let alvo = chave(exercicio);
        let mut pontos = Vec::new();
        let (mut carga, mut rm, mut volume) = (0.0f64, 0.0f64, 0.0f64);
        let mut ultima_vez = Vec::new();
        let mut nome = exercicio.trim().to_owned();
        for sessao in self.ordenadas() {
            let series: Vec<&Serie> = sessao.series.iter().filter(|s| chave(&s.exercicio) == alvo).collect();
            if series.is_empty() {
                continue;
            }
            nome = series[0].exercicio.clone();
            let carga_max = series.iter().map(|s| s.carga_kg).fold(0.0, f64::max);
            let um = series.iter().map(|s| um_rm(s.carga_kg, s.repeticoes)).fold(0.0, f64::max);
            let vol: f64 = series.iter().map(|s| s.carga_kg * f64::from(s.repeticoes)).sum();
            let recorde = !pontos.is_empty() && (carga_max > carga || um > rm + 0.05);
            carga = carga.max(carga_max);
            rm = rm.max(um);
            volume = volume.max(vol);
            pontos.push(Ponto { data: sessao.data.clone(), carga_max: arredondar(carga_max), volume: arredondar(vol),
                repeticoes_max: series.iter().map(|s| s.repeticoes).max().unwrap_or(0), um_rm: arredondar(um), series: series.len(), recorde });
            ultima_vez = series.into_iter().cloned().collect();
        }
        Evolucao { exercicio: nome, pontos, recorde_carga: arredondar(carga), recorde_um_rm: arredondar(rm), recorde_volume: arredondar(volume), ultima_vez }
    }

    /// Exercícios que já apareceram (em sessões ou fichas), com o resumo.
    pub fn exercicios(&self) -> Vec<ResumoDoExercicio> {
        let mut nomes: Vec<String> = Vec::new();
        for nome in self.sessoes.iter().flat_map(|s| s.series.iter().map(|x| &x.exercicio))
            .chain(self.fichas.iter().flat_map(|f| f.exercicios.iter().map(|e| &e.nome))) {
            if !nome.trim().is_empty() && !nomes.iter().any(|n| chave(n) == chave(nome)) {
                nomes.push(nome.trim().to_owned());
            }
        }
        let mut resumos: Vec<ResumoDoExercicio> = nomes.iter().map(|n| {
            let e = self.evolucao(n);
            ResumoDoExercicio { nome: e.exercicio.clone(), sessoes: e.pontos.len(), ultima_data: e.pontos.last().map(|p| p.data.clone()).unwrap_or_default(),
                carga_max: e.recorde_carga, um_rm: e.recorde_um_rm }
        }).collect();
        resumos.sort_by(|a, b| b.ultima_data.cmp(&a.ultima_data).then(a.nome.cmp(&b.nome)));
        resumos
    }

    /// Sessões por semana nas últimas `semanas` (mais antiga primeiro), terminando na semana de `hoje`.
    pub fn frequencia(&self, hoje: NaiveDate, semanas: u32) -> Vec<(String, u32)> {
        let atual = segunda_de(hoje);
        (0..semanas).rev().map(|n| {
            let segunda = atual - Duration::weeks(i64::from(n));
            let fim = segunda + Duration::days(7);
            let total = self.sessoes.iter().filter_map(|s| data(&s.data)).filter(|d| *d >= segunda && *d < fim).count() as u32;
            (texto_da_data(segunda), total)
        }).collect()
    }

    pub fn podar(&mut self) {
        if self.sessoes.len() > SESSOES_MAXIMAS {
            self.sessoes.sort_by(|a, b| a.data.cmp(&b.data));
            let excesso = self.sessoes.len() - SESSOES_MAXIMAS;
            self.sessoes.drain(..excesso);
        }
    }
}
