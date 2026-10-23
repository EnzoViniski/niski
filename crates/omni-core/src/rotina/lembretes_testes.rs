//! Testes dos lembretes: quando vencem, sem duplicar, adiar, janela acordado e horário de verão.
use super::alimentacao::Alimentacao;
use super::lembretes::*;
use super::plano::{Bloco, Plano};
use super::treino::Treino;
use super::*;
use chrono::{DateTime, FixedOffset, LocalResult, NaiveDateTime, TimeZone};
use serde_json::json;

/// Fuso sintético com as transições de NY em 2026 (mesmo do `agendamentos_dst_testes.rs`).
#[derive(Clone)]
struct FusoDst;
impl TimeZone for FusoDst {
    type Offset = FixedOffset;
    fn from_offset(_: &Self::Offset) -> Self { Self }
    fn offset_from_local_date(&self, d: &NaiveDate) -> LocalResult<Self::Offset> {
        self.offset_from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
    }
    fn offset_from_utc_date(&self, d: &NaiveDate) -> Self::Offset {
        self.offset_from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
    }
    fn offset_from_utc_datetime(&self, d: &NaiveDateTime) -> Self::Offset {
        let inicio = NaiveDate::from_ymd_opt(2026, 3, 8).unwrap().and_hms_opt(7, 0, 0).unwrap();
        let fim = NaiveDate::from_ymd_opt(2026, 11, 1).unwrap().and_hms_opt(6, 0, 0).unwrap();
        FixedOffset::west_opt(if *d >= inicio && *d < fim { 4 * 3600 } else { 5 * 3600 }).unwrap()
    }
    fn offset_from_local_datetime(&self, d: &NaiveDateTime) -> LocalResult<Self::Offset> {
        let validos: Vec<_> = [4, 5].into_iter().filter_map(|h| {
            let offset = FixedOffset::west_opt(h * 3600).unwrap();
            let utc = *d + Duration::hours(h.into());
            (self.offset_from_utc_datetime(&utc) == offset).then_some(offset)
        }).collect();
        match validos.as_slice() {
            [a] => LocalResult::Single(*a),
            [a, b] => LocalResult::Ambiguous(*a, *b),
            _ => LocalResult::None,
        }
    }
}

fn ms(s: &str) -> i64 {
    DateTime::parse_from_rfc3339(s).unwrap().timestamp_millis()
}

fn utc() -> FixedOffset {
    FixedOffset::east_opt(0).unwrap()
}

struct Cena {
    lembretes: Lembretes,
    plano: Plano,
    alimentacao: Alimentacao,
    treino: Treino,
    tarefas: Vec<serde_json::Value>,
}

impl Cena {
    fn nova() -> Self {
        let mut lembretes = Lembretes::default();
        lembretes.agua.ativo = false;
        lembretes.refeicoes = false;
        lembretes.planejar.ativo = false;
        Self { lembretes, plano: Plano::default(), alimentacao: Alimentacao::default(), treino: Treino::default(), tarefas: Vec::new() }
    }
    fn vencidos<T: TimeZone>(&self, estado: &Estado, agora: i64, fuso: &T) -> Vec<Disparo> {
        let f = Fontes { lembretes: &self.lembretes, plano: &self.plano, alimentacao: &self.alimentacao, treino: &self.treino, tarefas: &self.tarefas };
        vencidos(&f, estado, agora, fuso)
    }
}

#[test]
fn tarefa_lembra_x_minutos_antes_uma_vez_so() {
    let mut c = Cena::nova();
    c.tarefas.push(json!({"id": "t1", "titulo": "Dentista", "data": "2026-10-23", "hora": "15:00", "status": "a_fazer", "local": "Centro"}));
    c.tarefas.push(json!({"id": "t2", "titulo": "Feita", "data": "2026-10-23", "hora": "15:00", "status": "feito"}));
    let mut estado = Estado::default();
    assert!(c.vencidos(&estado, ms("2026-10-23T14:49:00Z"), &utc()).is_empty(), "antes da hora");
    let d = c.vencidos(&estado, ms("2026-10-23T14:50:30Z"), &utc());
    assert_eq!(d.len(), 1);
    assert_eq!((d[0].tipo.as_str(), d[0].titulo.as_str(), d[0].texto.as_str()), ("tarefa", "Dentista", "Às 15:00 · Centro"));
    estado.registrar(&d, ms("2026-10-23T14:50:30Z"));
    assert!(c.vencidos(&estado, ms("2026-10-23T14:51:00Z"), &utc()).is_empty(), "não duplica");
    // Remarcada para outra hora: lembra de novo.
    c.tarefas[0]["hora"] = json!("16:00");
    assert_eq!(c.vencidos(&estado, ms("2026-10-23T15:50:00Z"), &utc()).len(), 1);
}

#[test]
fn atrasado_alem_da_tolerancia_nao_sai_depois_de_dormir() {
    let mut c = Cena::nova();
    c.tarefas.push(json!({"id": "t1", "titulo": "Reunião", "data": "2026-10-23", "hora": "09:00", "status": "a_fazer"}));
    let estado = Estado::default();
    assert_eq!(c.vencidos(&estado, ms("2026-10-23T09:00:00Z"), &utc()).len(), 1);
    assert!(c.vencidos(&estado, ms("2026-10-23T11:00:00Z"), &utc()).is_empty());
}

#[test]
fn adiar_lembra_de_novo_e_tambem_uma_vez_so() {
    let mut c = Cena::nova();
    c.tarefas.push(json!({"id": "t1", "titulo": "Remédio", "data": "2026-10-23", "hora": "10:10", "status": "a_fazer"}));
    let mut estado = Estado::default();
    let agora = ms("2026-10-23T10:00:00Z");
    let d = c.vencidos(&estado, agora, &utc());
    estado.registrar(&d, agora);
    let novo = estado.adiar(&d[0].chave, 15, agora + 60_000).unwrap();
    assert_eq!(estado.aviso(&d[0].chave).unwrap().resposta, "adiado");
    assert!(c.vencidos(&estado, agora + 10 * 60_000, &utc()).is_empty());
    let de_novo = c.vencidos(&estado, agora + 16 * 60_000, &utc());
    assert_eq!(de_novo.len(), 1);
    assert_eq!(de_novo[0].chave, novo.chave);
    estado.registrar(&de_novo, agora + 16 * 60_000);
    assert!(estado.adiados.is_empty());
    assert!(c.vencidos(&estado, agora + 17 * 60_000, &utc()).is_empty());
    // Adiar o adiado gera outra chave.
    let outro = estado.adiar(&novo.chave, 15, agora + 17 * 60_000).unwrap();
    assert_ne!(outro.chave, novo.chave);
    assert!(estado.adiar("nao-existe", 15, agora).is_err());
}

#[test]
fn agua_so_na_janela_acordado_e_para_ao_bater_a_meta() {
    let mut c = Cena::nova();
    c.lembretes.agua.ativo = true;
    c.lembretes.agua.intervalo_min = 120;
    c.lembretes.acordar = "07:00".into();
    c.lembretes.dormir = "22:00".into();
    let dia = data("2026-10-23").unwrap();
    let f = Fontes { lembretes: &c.lembretes, plano: &c.plano, alimentacao: &c.alimentacao, treino: &c.treino, tarefas: &c.tarefas };
    let horas: Vec<String> = candidatos(&f, dia, &utc()).iter().filter(|d| d.tipo == "agua").map(|d| d.chave.rsplit(':').next().unwrap().to_owned()).collect();
    // 09:00, 11:00, … 21:00 (a chave termina em minutos; conferimos a quantidade e o primeiro/último).
    let chaves: Vec<String> = candidatos(&f, dia, &utc()).iter().filter(|d| d.tipo == "agua").map(|d| d.chave.clone()).collect();
    assert_eq!(chaves.len(), 7, "{horas:?}");
    assert_eq!(chaves.first().unwrap(), "agua:2026-10-23:09:00");
    assert_eq!(chaves.last().unwrap(), "agua:2026-10-23:21:00");
    c.alimentacao.somar_agua(dia, 2500);
    let f = Fontes { lembretes: &c.lembretes, plano: &c.plano, alimentacao: &c.alimentacao, treino: &c.treino, tarefas: &c.tarefas };
    assert!(candidatos(&f, dia, &utc()).iter().all(|d| d.tipo != "agua"));
}

#[test]
fn bloco_treino_refeicao_e_marca_feito() {
    let mut c = Cena::nova();
    c.lembretes.refeicoes = true;
    c.plano.blocos.push(Bloco { id: "b1".into(), titulo: "Academia".into(), tipo: "academia".into(), dias: vec![4], inicio: "06:30".into(), fim: "07:30".into(), lembrar: true, ..Default::default() });
    c.plano.blocos.push(Bloco { id: "b2".into(), titulo: "Aula".into(), tipo: "aula".into(), dias: vec![4], inicio: "19:00".into(), fim: "22:00".into(), lembrar: false, ..Default::default() });
    let dia = data("2026-10-23").unwrap(); // sexta
    let f = Fontes { lembretes: &c.lembretes, plano: &c.plano, alimentacao: &c.alimentacao, treino: &c.treino, tarefas: &c.tarefas };
    let tipos: Vec<String> = candidatos(&f, dia, &utc()).iter().map(|d| d.tipo.clone()).collect();
    assert_eq!(tipos.iter().filter(|t| *t == "treino").count(), 1);
    assert_eq!(tipos.iter().filter(|t| *t == "bloco").count(), 0, "bloco sem lembrar");
    assert_eq!(tipos.iter().filter(|t| *t == "refeicao").count(), 4);
    c.plano.marcar(dia, "bloco:b1", "feito").unwrap();
    let f = Fontes { lembretes: &c.lembretes, plano: &c.plano, alimentacao: &c.alimentacao, treino: &c.treino, tarefas: &c.tarefas };
    assert!(candidatos(&f, dia, &utc()).iter().all(|d| d.tipo != "treino"));
    c.lembretes.ativo = false;
    let f = Fontes { lembretes: &c.lembretes, plano: &c.plano, alimentacao: &c.alimentacao, treino: &c.treino, tarefas: &c.tarefas };
    assert!(candidatos(&f, dia, &utc()).is_empty(), "chave geral desliga tudo");
}

#[test]
fn domingo_a_noite_planejar_so_se_a_semana_seguinte_nao_foi_planejada() {
    let mut c = Cena::nova();
    c.lembretes.planejar.ativo = true;
    let estado = Estado::default();
    let d = c.vencidos(&estado, ms("2026-10-25T20:00:00Z"), &utc()); // domingo
    assert_eq!(d.len(), 1);
    assert_eq!((d[0].chave.as_str(), d[0].alvo.as_str()), ("planejar:2026-10-26", "2026-10-26"));
    assert_eq!(d[0].botoes, ["planejar", "adiar"]);
    c.plano.semana_mut(data("2026-10-26").unwrap()).planejada_em = "x".into();
    assert!(c.vencidos(&estado, ms("2026-10-25T20:00:00Z"), &utc()).is_empty());
    assert!(c.vencidos(&Estado::default(), ms("2026-10-24T20:00:00Z"), &utc()).is_empty(), "sábado não");
}

#[test]
fn lembrete_antes_da_meia_noite_cai_no_dia_anterior() {
    let mut c = Cena::nova();
    c.tarefas.push(json!({"id": "t1", "titulo": "Voo", "data": "2026-10-24", "hora": "00:05", "status": "a_fazer"}));
    let d = c.vencidos(&Estado::default(), ms("2026-10-23T23:55:00Z"), &utc());
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].data, "2026-10-24");
}

#[test]
fn horario_de_verao_hora_inexistente_e_repetida() {
    let mut c = Cena::nova();
    c.lembretes.antecedencia_min = 0;
    // 08/03/2026: 02:00 local vira 03:00. Tarefa às 02:30 lembra às 03:00 local (07:00Z).
    c.tarefas.push(json!({"id": "t1", "titulo": "Inexistente", "data": "2026-03-08", "hora": "02:30", "status": "a_fazer"}));
    let d = c.vencidos(&Estado::default(), ms("2026-03-08T07:00:00Z"), &FusoDst);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].em_ms, ms("2026-03-08T07:00:00Z"));
    // 01/11/2026: 01:30 acontece duas vezes; vale a primeira (05:30Z) e só uma vez.
    c.tarefas[0] = json!({"id": "t2", "titulo": "Repetida", "data": "2026-11-01", "hora": "01:30", "status": "a_fazer"});
    let mut estado = Estado::default();
    let d = c.vencidos(&estado, ms("2026-11-01T05:31:00Z"), &FusoDst);
    assert_eq!(d.len(), 1);
    assert_eq!(d[0].em_ms, ms("2026-11-01T05:30:00Z"));
    estado.registrar(&d, ms("2026-11-01T05:31:00Z"));
    assert!(c.vencidos(&estado, ms("2026-11-01T06:31:00Z"), &FusoDst).is_empty(), "a segunda 01:30 não repete");
}

#[test]
fn estado_poda_e_guarda_poucos_avisos() {
    let mut estado = Estado::default();
    let base = ms("2026-10-01T00:00:00Z");
    for n in 0..100 {
        let d = Disparo { chave: format!("x:{n}"), em_ms: base, ..Default::default() };
        estado.registrar(&[d], base + n * 3_600_000);
    }
    assert_eq!(estado.avisos.len(), AVISOS_GUARDADOS);
    assert!(estado.enviados.len() <= 73, "só 3 dias de chaves");
}

#[test]
fn responder_aviso_pela_operacao_conclui_marca_e_soma_agua() {
    use super::operacoes::{executar, Agora};
    let dir = tempfile::tempdir().unwrap();
    let hoje = data("2026-10-23").unwrap();
    let agora = Agora { ms: ms("2026-10-23T12:00:00Z"), hoje, hora: hora("12:00").unwrap() };
    let ctx = crate::tarefas::contexto(dir.path()).unwrap();
    let t = crate::tarefas::criar(&ctx.raiz, &json!({"titulo": "Pagar boleto", "data": "2026-10-23", "hora": "12:10"}), "", 1, 1).unwrap();
    let id = t["id"].as_str().unwrap();
    let mut estado = Estado::default();
    let disparos = vec![
        Disparo { chave: format!("tarefa:{id}:2026-10-23:12:10"), tipo: "tarefa".into(), alvo: id.into(), data: "2026-10-23".into(), ..Default::default() },
        Disparo { chave: "agua:2026-10-23:12:00".into(), tipo: "agua".into(), alvo: "agua".into(), data: "2026-10-23".into(), ..Default::default() },
        Disparo { chave: "refeicao:almoco:2026-10-23".into(), tipo: "refeicao".into(), alvo: "almoco".into(), data: "2026-10-23".into(), ..Default::default() },
        Disparo { chave: "refeicao:jantar:2026-10-23".into(), tipo: "refeicao".into(), alvo: "jantar".into(), data: "2026-10-23".into(), ..Default::default() },
    ];
    estado.registrar(&disparos, agora.ms);
    gravar(dir.path(), &estado).unwrap();
    executar(dir.path(), "responder_aviso", &json!({"chave": disparos[0].chave, "acao": "feito"}), &agora).unwrap();
    assert_eq!(crate::tarefas::ler(&ctx.raiz, id).unwrap()["status"], "feito");
    let r = executar(dir.path(), "responder_aviso", &json!({"chave": disparos[1].chave, "acao": "copo"}), &agora).unwrap();
    assert_eq!(r["agua_ml"], 250);
    executar(dir.path(), "responder_aviso", &json!({"chave": disparos[2].chave, "acao": "pular"}), &agora).unwrap();
    let plano: Plano = ler(dir.path());
    assert!(plano.marcas_do_dia(hoje).pulados.contains(&"refeicao:almoco".to_owned()));
    // Fork (Rotina): já respondido (pulado) não vira adiado depois.
    let r = executar(dir.path(), "responder_aviso", &json!({"chave": disparos[2].chave, "acao": "adiar", "minutos": 30}), &agora).unwrap();
    assert_eq!(r["ja_respondido"], "pulado");
    let r = executar(dir.path(), "responder_aviso", &json!({"chave": disparos[3].chave, "acao": "adiar", "minutos": 30}), &agora).unwrap();
    assert_eq!(r["novo_em_ms"], agora.ms + 30 * 60_000);
    let avisos = executar(dir.path(), "ler_avisos", &json!({}), &agora).unwrap();
    assert_eq!(avisos["nao_vistos"], 0);
    assert!(executar(dir.path(), "responder_aviso", &json!({"chave": "x", "acao": "feito"}), &agora).is_err());
}
