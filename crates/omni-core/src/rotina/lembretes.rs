//! Fork (Rotina): lembretes — o que lembrar (ajustes em `lembretes.json`) e a conta de quando cada
//! lembrete vence (pura: recebe o instante e o fuso; o engine chama a cada 30 s com o fuso local).
//!
//! Cada ocorrência tem uma **chave** estável (`tarefa:<id>:<data>:<hora>`, `agua:<data>:<HH:MM>`…).
//! Ela entra em `lembretes-estado.json` ao ser enviada: o mesmo lembrete nunca sai duas vezes, nem
//! depois de o engine reiniciar. Ocorrência que passou há mais de [`TOLERANCIA_MS`] (PC dormindo,
//! engine fechado) não sai atrasada: some, sem empilhar avisos velhos.
//!
//! Horário de verão: hora que não existe no dia (o relógio pula) vale no primeiro minuto válido
//! depois dela; hora que acontece duas vezes vale na primeira, uma vez só (a chave é por data local).

use super::alimentacao::Alimentacao;
use super::plano::{planejada, Plano};
use super::treino::Treino;
use super::{dia_da_semana, hora, lista, mapa, minutos, segunda_de, texto_da_data, texto_da_hora, Arquivo};
use chrono::{DateTime, Duration, LocalResult, NaiveDate, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;

/// Até quanto tempo depois da hora um lembrete ainda sai.
pub const TOLERANCIA_MS: i64 = 15 * 60_000;
/// Adiado pode sair com mais atraso (o usuário pediu para lembrar de novo).
pub const TOLERANCIA_ADIADO_MS: i64 = 6 * 3_600_000;
pub const AVISOS_GUARDADOS: usize = 60;
pub const TIPOS_PERSONALIZADOS: &[&str] = &["meta", "livre", "remedio"];

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Agua {
    pub ativo: bool,
    pub intervalo_min: u32,
}

impl Default for Agua {
    fn default() -> Self { Self { ativo: true, intervalo_min: 90 } }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Planejar {
    pub ativo: bool,
    /// 0 = segunda … 6 = domingo.
    pub dia: u8,
    pub hora: String,
}

impl Default for Planejar {
    fn default() -> Self { Self { ativo: true, dia: 6, hora: "20:00".into() } }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Personalizado {
    pub id: String,
    pub texto: String,
    pub hora: String,
    /// Vazio com `data` vazia = todo dia.
    pub dias: Vec<u8>,
    /// Um dia só.
    pub data: String,
    /// `meta`, `livre`, `remedio`.
    pub tipo: String,
    pub ativo: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Lembretes {
    pub versao: u32,
    /// Chave geral.
    pub ativo: bool,
    /// Minutos antes de tarefa com hora e de bloco.
    pub antecedencia_min: u32,
    pub tarefas: bool,
    pub blocos: bool,
    pub refeicoes: bool,
    pub treino: bool,
    pub agua: Agua,
    /// Janela acordado: água só entre estas horas.
    pub acordar: String,
    pub dormir: String,
    pub planejar: Planejar,
    #[serde(deserialize_with = "lista")]
    pub personalizados: Vec<Personalizado>,
}

impl Default for Lembretes {
    fn default() -> Self {
        Self { versao: super::VERSAO, ativo: true, antecedencia_min: 10, tarefas: true, blocos: true, refeicoes: true, treino: true,
            agua: Agua::default(), acordar: "07:00".into(), dormir: "23:00".into(), planejar: Planejar::default(), personalizados: Vec::new() }
    }
}

impl Arquivo for Lembretes {
    const NOME: &'static str = "lembretes.json";
    fn versao(&self) -> u32 { self.versao }
    fn marcar_versao(&mut self) { self.versao = super::VERSAO; }
}

/// Um lembrete que vence agora.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Disparo {
    pub chave: String,
    /// `tarefa`, `bloco`, `treino`, `refeicao`, `agua`, `planejar`, `lembrete`.
    pub tipo: String,
    pub titulo: String,
    pub texto: String,
    pub em_ms: i64,
    /// Id do item (tarefa, bloco, refeição do plano, lembrete).
    pub alvo: String,
    /// Data local do item.
    pub data: String,
    /// Botões que fazem sentido: `feito`, `adiar`, `pular`, `copo`, `planejar`.
    pub botoes: Vec<String>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Aviso {
    #[serde(flatten)]
    pub disparo: Disparo,
    pub enviado_ms: i64,
    pub visto: bool,
    /// O que o usuário respondeu: `feito`, `adiado`, `pulado`, `copo`, `planejar` (vazio = nada).
    pub resposta: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Adiado {
    pub disparo: Disparo,
    pub em_ms: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Estado {
    pub versao: u32,
    /// Chave → quando saiu (ms).
    #[serde(deserialize_with = "mapa")]
    pub enviados: BTreeMap<String, i64>,
    #[serde(deserialize_with = "lista")]
    pub adiados: Vec<Adiado>,
    /// Os mais recentes, para a tela e para os botões do Telegram.
    #[serde(deserialize_with = "lista")]
    pub avisos: Vec<Aviso>,
}

impl Default for Estado {
    fn default() -> Self { Self { versao: super::VERSAO, enviados: BTreeMap::new(), adiados: Vec::new(), avisos: Vec::new() } }
}

impl Arquivo for Estado {
    const NOME: &'static str = "lembretes-estado.json";
    fn versao(&self) -> u32 { self.versao }
    fn marcar_versao(&mut self) { self.versao = super::VERSAO; }
}

/// O que os lembretes leem.
pub struct Fontes<'a> {
    pub lembretes: &'a Lembretes,
    pub plano: &'a Plano,
    pub alimentacao: &'a Alimentacao,
    pub treino: &'a Treino,
    /// Tarefas do app (JSON de `crate::tarefas::todas`).
    pub tarefas: &'a [Value],
}

/// Instante (ms) de uma hora local numa data, com as regras de horário de verão do módulo.
pub fn instante<T: TimeZone>(fuso: &T, dia: NaiveDate, h: NaiveTime) -> Option<i64> {
    let local = dia.and_time(h);
    for minuto in 0..=180 {
        match fuso.from_local_datetime(&(local + Duration::minutes(minuto))) {
            LocalResult::Single(d) => return Some(d.timestamp_millis()),
            LocalResult::Ambiguous(a, b) => return Some(a.timestamp_millis().min(b.timestamp_millis())),
            LocalResult::None => continue,
        }
    }
    None
}

/// Data local de um instante.
pub fn data_local<T: TimeZone>(fuso: &T, ms: i64) -> Option<NaiveDate> {
    Some(DateTime::<Utc>::from_timestamp_millis(ms)?.with_timezone(fuso).date_naive())
}

fn menos(h: NaiveTime, min: u32) -> (NaiveTime, bool) {
    let total = minutos(h) as i64 - i64::from(min);
    if total < 0 {
        (NaiveTime::from_hms_opt(((total + 1440) / 60) as u32, ((total + 1440) % 60) as u32, 0).unwrap_or(h), true)
    } else {
        (NaiveTime::from_hms_opt((total / 60) as u32, (total % 60) as u32, 0).unwrap_or(h), false)
    }
}

fn botoes(lista: &[&str]) -> Vec<String> {
    lista.iter().map(|b| (*b).to_owned()).collect()
}

fn feito_ou_pulado(plano: &Plano, dia: NaiveDate, item: &str) -> bool {
    let m = plano.marcas_do_dia(dia);
    m.feitos.iter().chain(m.pulados.iter()).any(|i| i == item)
}

/// Ocorrências de lembrete cujo item é da data `dia` (com instante). Não filtra por horário.
pub fn candidatos<T: TimeZone>(f: &Fontes, dia: NaiveDate, fuso: &T) -> Vec<Disparo> {
    let l = f.lembretes;
    let mut saida = Vec::new();
    if !l.ativo {
        return saida;
    }
    let texto_dia = texto_da_data(dia);
    // Lembrete marcado X min antes: se a conta cair no dia anterior, vale o instante anterior mesmo.
    let mut empurrar = |chave: String, tipo: &str, titulo: String, texto: String, h: NaiveTime, antes: u32, alvo: &str, b: &[&str]| {
        let (hora_do_aviso, dia_antes) = menos(h, antes);
        let data_do_aviso = if dia_antes { dia - Duration::days(1) } else { dia };
        if let Some(em_ms) = instante(fuso, data_do_aviso, hora_do_aviso) {
            saida.push(Disparo { chave, tipo: tipo.into(), titulo, texto, em_ms, alvo: alvo.into(), data: texto_dia.clone(), botoes: botoes(b) });
        }
    };
    let antes = l.antecedencia_min.min(240);

    if l.tarefas {
        for t in f.tarefas {
            let (Some(id), Some(data_t), Some(h)) = (t["id"].as_str(), t["data"].as_str(), t["hora"].as_str().and_then(hora)) else { continue };
            if data_t != texto_dia || t["status"].as_str() == Some("feito") || t["apagada"] == Value::Bool(true) {
                continue;
            }
            let titulo = t["titulo"].as_str().unwrap_or("Tarefa").chars().take(200).collect::<String>();
            let local = t["local"].as_str().filter(|s| !s.is_empty()).map(|s| format!(" · {s}")).unwrap_or_default();
            empurrar(format!("tarefa:{id}:{texto_dia}:{}", texto_da_hora(h)), "tarefa", titulo,
                format!("Às {}{local}", texto_da_hora(h)), h, antes, id, &["feito", "adiar", "pular"]);
        }
    }

    for b in f.plano.blocos_do_dia(dia) {
        let Some(h) = hora(&b.inicio) else { continue };
        let academia = b.tipo == "academia";
        let ligado = if academia { l.treino } else { l.blocos && b.lembrar };
        if !ligado || feito_ou_pulado(f.plano, dia, &format!("bloco:{}", b.id)) {
            continue;
        }
        let mut texto = format!("{}–{}", b.inicio, b.fim);
        if !b.local.is_empty() {
            texto.push_str(&format!(" · {}", b.local));
        }
        if academia {
            if let Some(ficha) = f.treino.proxima_ficha(dia) {
                texto.push_str(&format!(" · Ficha {}{}", ficha.nome, if ficha.titulo.is_empty() { String::new() } else { format!(" ({})", ficha.titulo) }));
            }
        }
        empurrar(format!("{}:{}:{texto_dia}:{}", if academia { "treino" } else { "bloco" }, b.id, b.inicio),
            if academia { "treino" } else { "bloco" }, b.titulo.clone(), texto, h, antes, &b.id, &["feito", "adiar", "pular"]);
    }

    if l.refeicoes {
        let registradas = f.alimentacao.dia(dia);
        for r in &f.alimentacao.plano {
            let Some(h) = hora(&r.hora) else { continue };
            if registradas.refeicoes.iter().any(|x| x.planejada == r.id) || feito_ou_pulado(f.plano, dia, &format!("refeicao:{}", r.id)) {
                continue;
            }
            let texto = if r.sugestao.is_empty() { format!("{} às {}", r.nome, r.hora) } else { r.sugestao.chars().take(300).collect() };
            empurrar(format!("refeicao:{}:{texto_dia}", r.id), "refeicao", format!("Hora de comer: {}", r.nome), texto, h, 0, &r.id,
                &["feito", "adiar", "pular"]);
        }
    }

    if l.agua.ativo && l.agua.intervalo_min >= 15 {
        let a = f.alimentacao;
        let bebido = a.dia(dia).agua_ml;
        if let (Some(acordar), Some(dormir)) = (hora(&l.acordar), hora(&l.dormir)) {
            if bebido < a.meta_agua_ml {
                let fim = if minutos(dormir) > minutos(acordar) { minutos(dormir) } else { minutos(dormir) + 1440 };
                let mut m = minutos(acordar) + l.agua.intervalo_min;
                while m < fim && m < 1440 {
                    let h = NaiveTime::from_hms_opt(m / 60, m % 60, 0).unwrap_or(acordar);
                    let texto = format!("{:.1} L de {:.1} L hoje", f64::from(bebido) / 1000.0, f64::from(a.meta_agua_ml) / 1000.0).replace('.', ",");
                    empurrar(format!("agua:{texto_dia}:{}", texto_da_hora(h)), "agua", "Beba água".into(), texto, h, 0, "agua",
                        &["copo", "adiar", "pular"]);
                    m += l.agua.intervalo_min;
                }
            }
        }
    }

    if l.planejar.ativo && dia_da_semana(dia) == l.planejar.dia {
        let proxima = segunda_de(dia) + Duration::days(7);
        if let Some(h) = hora(&l.planejar.hora) {
            if !planejada(f.plano, proxima) {
                empurrar(format!("planejar:{}", texto_da_data(proxima)), "planejar", "Hora de planejar a semana".into(),
                    "Abra a conversa da rotina: compromissos, faculdade, Medza, Aircook, treino, alimentação e metas.".into(), h, 0,
                    &texto_da_data(proxima), &["planejar", "adiar"]);
            }
        }
    }

    let semana = dia_da_semana(dia);
    for p in l.personalizados.iter().filter(|p| p.ativo) {
        let vale = if !p.data.is_empty() { p.data == texto_dia } else { p.dias.is_empty() || p.dias.contains(&semana) };
        let Some(h) = hora(&p.hora) else { continue };
        if !vale || feito_ou_pulado(f.plano, dia, &format!("lembrete:{}", p.id)) {
            continue;
        }
        let titulo = match p.tipo.as_str() { "meta" => "Meta", "remedio" => "Remédio", _ => "Lembrete" };
        empurrar(format!("lembrete:{}:{texto_dia}", p.id), "lembrete", titulo.into(), p.texto.chars().take(500).collect(), h, 0, &p.id,
            &["feito", "adiar"]);
    }
    saida
}

/// Lembretes que vencem agora e ainda não saíram (inclui os adiados que chegaram na hora).
pub fn vencidos<T: TimeZone>(f: &Fontes, estado: &Estado, agora_ms: i64, fuso: &T) -> Vec<Disparo> {
    let Some(hoje) = data_local(fuso, agora_ms) else { return Vec::new() };
    let mut saida: Vec<Disparo> = Vec::new();
    // Ontem e amanhã: X min antes de um item de 00:05 cai no dia anterior.
    for dia in [hoje - Duration::days(1), hoje, hoje + Duration::days(1)] {
        for d in candidatos(f, dia, fuso) {
            if d.em_ms <= agora_ms && agora_ms - d.em_ms <= TOLERANCIA_MS && !estado.enviados.contains_key(&d.chave)
                && !saida.iter().any(|x| x.chave == d.chave) {
                saida.push(d);
            }
        }
    }
    for a in &estado.adiados {
        if a.em_ms <= agora_ms && agora_ms - a.em_ms <= TOLERANCIA_ADIADO_MS && !estado.enviados.contains_key(&a.disparo.chave) {
            let mut d = a.disparo.clone();
            d.em_ms = a.em_ms;
            saida.push(d);
        }
    }
    saida.sort_by_key(|d| d.em_ms);
    saida
}

impl Estado {
    /// Marca como enviados e guarda o aviso. Poda chaves com mais de 3 dias e adiados vencidos.
    pub fn registrar(&mut self, disparos: &[Disparo], agora_ms: i64) {
        for d in disparos {
            self.enviados.insert(d.chave.clone(), agora_ms);
            self.avisos.push(Aviso { disparo: d.clone(), enviado_ms: agora_ms, visto: false, resposta: String::new() });
        }
        let limite = agora_ms - 3 * 86_400_000;
        self.enviados.retain(|_, ms| *ms >= limite);
        self.adiados.retain(|a| !self.enviados.contains_key(&a.disparo.chave) && a.em_ms + TOLERANCIA_ADIADO_MS >= agora_ms);
        if self.avisos.len() > AVISOS_GUARDADOS {
            let excesso = self.avisos.len() - AVISOS_GUARDADOS;
            self.avisos.drain(..excesso);
        }
    }

    pub fn aviso(&self, chave: &str) -> Option<&Aviso> {
        self.avisos.iter().rev().find(|a| a.disparo.chave == chave)
    }

    /// Lembra de novo daqui a `minutos`. A chave nova (`<chave>~<n>`) também sai uma vez só.
    pub fn adiar(&mut self, chave: &str, minutos: u32, agora_ms: i64) -> Result<Disparo, String> {
        let aviso = self.aviso(chave).cloned().ok_or_else(|| "Lembrete não encontrado (já saiu da lista)".to_owned())?;
        let base = chave.split('~').next().unwrap_or(chave);
        let n = self.enviados.keys().filter(|k| k.starts_with(&format!("{base}~"))).count()
            + self.adiados.iter().filter(|a| a.disparo.chave.starts_with(&format!("{base}~"))).count() + 1;
        let mut disparo = aviso.disparo.clone();
        disparo.chave = format!("{base}~{n}");
        let em_ms = agora_ms + i64::from(minutos.clamp(1, 24 * 60)) * 60_000;
        disparo.em_ms = em_ms;
        self.adiados.push(Adiado { disparo: disparo.clone(), em_ms });
        self.responder(chave, "adiado");
        Ok(disparo)
    }

    pub fn responder(&mut self, chave: &str, resposta: &str) {
        if let Some(a) = self.avisos.iter_mut().rev().find(|a| a.disparo.chave == chave) {
            a.resposta = resposta.to_owned();
            a.visto = true;
        }
    }

    pub fn marcar_vistos(&mut self) {
        for a in &mut self.avisos {
            a.visto = true;
        }
    }
}
