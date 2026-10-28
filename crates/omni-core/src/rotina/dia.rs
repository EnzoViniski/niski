//! Fork (Rotina): o dia e a semana montados para a tela e para o agente — blocos do plano, tarefas
//! com data (as do app), refeições, treino e água numa linha do tempo, com "agora" e "próximo".

use super::alimentacao::{Alimentacao, Totais};
use super::lembretes::Lembretes;
use super::plano::{Area, Imprevisto, Meta, Plano};
use super::treino::Treino;
use super::{dia_da_semana, hora, minutos, segunda_de, texto_da_data, NOMES_DOS_DIAS};
use chrono::{Duration, NaiveDate, NaiveTime};
use serde::Serialize;
use serde_json::Value;

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Item {
    /// Chave do item no dia (`bloco:<id>`, `tarefa:<id>`, `refeicao:<id>`): a mesma das marcas.
    pub id: String,
    /// `bloco`, `treino`, `tarefa`, `refeicao`.
    pub tipo: String,
    pub titulo: String,
    pub detalhe: String,
    pub inicio: String,
    pub fim: String,
    pub area: String,
    pub cor: String,
    /// `feito`, `pulado` ou vazio.
    pub estado: String,
    /// Id do item de origem (tarefa, bloco, refeição do plano).
    pub alvo: String,
    /// Subtipo do bloco (`aula`, `trabalho`, `sono`…).
    pub categoria: String,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Agua {
    pub ml: u32,
    pub meta_ml: u32,
    pub copo_ml: u32,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct TreinoDoDia {
    pub ficha: Option<Value>,
    pub sessao: Option<Value>,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Dia {
    pub data: String,
    pub dia_da_semana: String,
    pub itens: Vec<Item>,
    /// Tarefas do dia sem hora (e as atrasadas, quando o dia é hoje).
    pub sem_hora: Vec<Item>,
    pub cancelados: Vec<String>,
    pub agua: Agua,
    pub alimentacao: Totais,
    pub treino: TreinoDoDia,
    pub agora: Option<String>,
    pub proximo: Option<String>,
    pub foco: String,
    pub metas: Vec<Meta>,
}

/// O que a montagem lê.
pub struct Fontes<'a> {
    pub plano: &'a Plano,
    pub alimentacao: &'a Alimentacao,
    pub treino: &'a Treino,
    pub lembretes: &'a Lembretes,
    pub tarefas: &'a [Value],
    /// Fork (Google Agenda): compromissos copiados do Google pelo engine.
    pub google: &'a super::google::AgendaDoGoogle,
}

fn somar(h: &str, minutos_a_mais: u64) -> String {
    match hora(h) {
        Some(t) => {
            let total = (u64::from(minutos(t)) + minutos_a_mais).min(23 * 60 + 59) as u32;
            format!("{:02}:{:02}", total / 60, total % 60)
        }
        None => String::new(),
    }
}

fn cor(plano: &Plano, area: &str) -> String {
    plano.area(area).map(|a| a.cor.clone()).unwrap_or_else(|| "#64748b".into())
}

fn item_da_tarefa(plano: &Plano, t: &Value) -> Option<Item> {
    let id = t["id"].as_str()?;
    let area = plano.area_da_tarefa(t).unwrap_or_default();
    let inicio = t["hora"].as_str().unwrap_or("").to_owned();
    let duracao = t["duracaoMin"].as_u64().unwrap_or(0);
    let fim = if inicio.is_empty() { String::new() } else { somar(&inicio, if duracao == 0 { 30 } else { duracao }) };
    let local = t["local"].as_str().unwrap_or("");
    Some(Item {
        id: format!("tarefa:{id}"), tipo: "tarefa".into(), titulo: t["titulo"].as_str().unwrap_or("Tarefa").into(),
        detalhe: local.into(), inicio, fim, cor: cor(plano, &area), area,
        estado: if t["status"].as_str() == Some("feito") { "feito".into() } else { String::new() },
        alvo: id.into(), categoria: t["categoria"].as_str().unwrap_or("").into(),
    })
}

fn resumo_da_ficha(f: &super::treino::Ficha) -> Value {
    serde_json::json!({"id": f.id, "nome": f.nome, "titulo": f.titulo, "exercicios": f.exercicios.len()})
}

pub fn montar_dia(f: &Fontes, dia: NaiveDate, hoje: NaiveDate, agora: Option<NaiveTime>) -> Dia {
    let texto = texto_da_data(dia);
    let marcas = f.plano.marcas_do_dia(dia);
    let estado_de = |id: &str| {
        if marcas.feitos.iter().any(|m| m == id) { "feito".to_owned() }
        else if marcas.pulados.iter().any(|m| m == id) { "pulado".to_owned() }
        else { String::new() }
    };
    let sessao = f.treino.sessao_do_dia(dia);
    let mut itens: Vec<Item> = f.plano.blocos_do_dia(dia).into_iter().map(|b| {
        let id = format!("bloco:{}", b.id);
        let academia = b.tipo == "academia";
        let mut estado = estado_de(&id);
        if academia && estado.is_empty() && sessao.is_some_and(|s| s.concluida) {
            estado = "feito".into();
        }
        let mut detalhe = b.local.clone();
        if let Some(nota) = b.mudado.as_ref().filter(|n| !n.is_empty()) {
            detalhe = if detalhe.is_empty() { format!("Mudou hoje: {nota}") } else { format!("{detalhe} · mudou hoje: {nota}") };
        }
        Item { id, tipo: if academia { "treino".into() } else { "bloco".into() }, titulo: b.titulo.clone(), detalhe, inicio: b.inicio.clone(),
            fim: b.fim.clone(), cor: cor(f.plano, &b.area), area: b.area.clone(), estado, alvo: b.id.clone(), categoria: b.tipo.clone() }
    }).collect();

    let mut sem_hora = Vec::new();
    for t in f.tarefas {
        let data_t = t["data"].as_str().unwrap_or("");
        let aberta = t["status"].as_str() != Some("feito");
        let atrasada = dia == hoje && aberta && !data_t.is_empty() && data_t < texto.as_str();
        if data_t != texto && !atrasada {
            continue;
        }
        let Some(item) = item_da_tarefa(f.plano, t) else { continue };
        if item.inicio.is_empty() || atrasada { sem_hora.push(item) } else { itens.push(item) }
    }

    // Fork (Google Agenda): compromissos do Google; os de dia inteiro vão para "sem hora".
    for e in f.google.do_dia(dia) {
        let id = super::google::chave_do_item(e);
        let detalhe = [e.local.as_str(), e.agenda.as_str()].into_iter().filter(|x| !x.is_empty()).collect::<Vec<_>>().join(" · ");
        let cor = if e.cor.is_empty() { "#1a73e8".to_owned() } else { e.cor.clone() };
        let item = Item { estado: estado_de(&id), id, tipo: "compromisso".into(), titulo: e.titulo.clone(), detalhe,
            inicio: e.inicio.clone(), fim: e.fim.clone(), area: String::new(), cor, alvo: e.id.clone(),
            categoria: if e.dia_todo { "dia_todo".into() } else { "google".into() } };
        if e.dia_todo { sem_hora.push(item) } else { itens.push(item) }
    }

    let registradas = f.alimentacao.dia(dia);
    for r in &f.alimentacao.plano {
        let id = format!("refeicao:{}", r.id);
        let feita = registradas.refeicoes.iter().find(|x| x.planejada == r.id);
        let mut estado = estado_de(&id);
        if feita.is_some() { estado = "feito".into(); }
        itens.push(Item { id, tipo: "refeicao".into(), titulo: r.nome.clone(),
            detalhe: feita.map(|x| x.texto.clone()).unwrap_or_else(|| r.sugestao.clone()), inicio: r.hora.clone(), fim: String::new(),
            area: "saude".into(), cor: cor(f.plano, "saude"), estado, alvo: r.id.clone(), categoria: "refeicao".into() });
    }
    itens.sort_by_key(|i| hora(&i.inicio).map(minutos).unwrap_or(u32::MAX));

    let (mut atual, mut proximo) = (None, None);
    if let (Some(agora), true) = (agora, dia == hoje) {
        let m = minutos(agora);
        for i in itens.iter().filter(|i| i.estado.is_empty()) {
            let (Some(ini), fim) = (hora(&i.inicio).map(minutos), hora(&i.fim).map(minutos)) else { continue };
            let fim = fim.filter(|f| *f > ini).unwrap_or(ini + 30);
            if atual.is_none() && ini <= m && m < fim { atual = Some(i.id.clone()); }
            if proximo.is_none() && ini > m { proximo = Some(i.id.clone()); }
        }
    }
    let semana = f.plano.semana(segunda_de(dia));
    Dia {
        data: texto, dia_da_semana: NOMES_DOS_DIAS[dia_da_semana(dia) as usize].into(), itens, sem_hora,
        cancelados: f.plano.cancelados_do_dia(dia),
        agua: Agua { ml: registradas.agua_ml, meta_ml: f.alimentacao.meta_agua_ml, copo_ml: f.alimentacao.copo_ml.max(50) },
        alimentacao: f.alimentacao.totais(dia),
        treino: TreinoDoDia {
            ficha: sessao.and_then(|s| f.treino.ficha(&s.ficha)).or_else(|| f.treino.proxima_ficha(dia)).map(resumo_da_ficha),
            sessao: sessao.map(|s| serde_json::json!({"id": s.id, "ficha": s.ficha, "series": s.series.len(), "volume": s.volume(), "concluida": s.concluida})),
        },
        agora: atual, proximo, foco: semana.foco, metas: semana.metas,
    }
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Progresso {
    pub itens_feitos: usize,
    pub itens_total: usize,
    pub treinos: u32,
    pub meta_treinos: u32,
    pub dias_com_agua_na_meta: usize,
    pub metas_concluidas: usize,
    pub metas_total: usize,
}

#[derive(Clone, Debug, Serialize, PartialEq)]
pub struct Semana {
    pub segunda: String,
    pub dias: Vec<Dia>,
    pub foco: String,
    pub metas: Vec<Meta>,
    pub notas: String,
    pub planejada_em: String,
    pub imprevistos: Vec<Imprevisto>,
    pub areas: Vec<Area>,
    pub progresso: Progresso,
    pub memoria: String,
}

pub fn montar_semana(f: &Fontes, qualquer_dia: NaiveDate, hoje: NaiveDate, agora: Option<NaiveTime>) -> Semana {
    let segunda = segunda_de(qualquer_dia);
    let dias: Vec<Dia> = (0..7).map(|n| montar_dia(f, segunda + Duration::days(n), hoje, agora)).collect();
    let semana = f.plano.semana(segunda);
    // Progresso: só itens até hoje contam (o que ainda vai acontecer não é "não feito").
    let passados: Vec<&Dia> = dias.iter().filter(|d| super::data(&d.data).is_some_and(|x| x <= hoje)).collect();
    let contaveis = passados.iter().flat_map(|d| d.itens.iter()).filter(|i| i.tipo != "refeicao" && i.categoria != "sono" && i.tipo != "compromisso"); // Compromisso do Google não é "a fazer"
    let (mut feitos, mut total) = (0, 0);
    for i in contaveis {
        total += 1;
        if i.estado == "feito" { feitos += 1; }
    }
    let treinos = f.treino.frequencia(segunda + Duration::days(6), 1).first().map(|(_, n)| *n).unwrap_or(0);
    Semana {
        segunda: texto_da_data(segunda),
        progresso: Progresso {
            itens_feitos: feitos, itens_total: total, treinos, meta_treinos: f.treino.meta_semanal,
            dias_com_agua_na_meta: passados.iter().filter(|d| d.agua.meta_ml > 0 && d.agua.ml >= d.agua.meta_ml).count(),
            metas_concluidas: semana.metas.iter().filter(|m| m.concluida || m.fracao() >= 1.0).count(), metas_total: semana.metas.len(),
        },
        dias, foco: semana.foco, metas: semana.metas, notas: semana.notas, planejada_em: semana.planejada_em, imprevistos: semana.imprevistos,
        areas: f.plano.areas.clone(), memoria: f.plano.memoria.clone(),
    }
}
