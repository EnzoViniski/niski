//! Fuso sintético com transições conhecidas de NY em 2026. Sem TZ global nem relógio real.
use super::*;
use chrono::{FixedOffset, NaiveDate, NaiveDateTime};
#[derive(Clone)]
struct FusoDst;
impl TimeZone for FusoDst {
    type Offset = FixedOffset;
    fn from_offset(_: &Self::Offset) -> Self {
        Self
    }
    fn offset_from_local_date(&self, d: &NaiveDate) -> LocalResult<Self::Offset> {
        self.offset_from_local_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
    }
    fn offset_from_utc_date(&self, d: &NaiveDate) -> Self::Offset {
        self.offset_from_utc_datetime(&d.and_hms_opt(0, 0, 0).unwrap())
    }
    fn offset_from_utc_datetime(&self, d: &NaiveDateTime) -> Self::Offset {
        let inicio = NaiveDate::from_ymd_opt(2026, 3, 8)
            .unwrap()
            .and_hms_opt(7, 0, 0)
            .unwrap();
        let fim = NaiveDate::from_ymd_opt(2026, 11, 1)
            .unwrap()
            .and_hms_opt(6, 0, 0)
            .unwrap();
        FixedOffset::west_opt(if *d >= inicio && *d < fim {
            4 * 3600
        } else {
            5 * 3600
        })
        .unwrap()
    }
    fn offset_from_local_datetime(&self, d: &NaiveDateTime) -> LocalResult<Self::Offset> {
        let validos: Vec<_> = [4, 5]
            .into_iter()
            .filter_map(|h| {
                let offset = FixedOffset::west_opt(h * 3600).unwrap();
                let utc = *d + Duration::hours(h.into());
                (self.offset_from_utc_datetime(&utc) == offset).then_some(offset)
            })
            .collect();
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
#[test]
fn horario_de_verao_em_tabela() {
    for (horario, agora, esperado) in [
        ("02:30", "2026-03-08T05:00:00Z", "2026-03-08T07:00:00Z"), // salto para 03:00 local
        ("09:00", "2026-03-07T15:00:00Z", "2026-03-08T13:00:00Z"), // dia de 23 h
        ("01:30", "2026-11-01T04:00:00Z", "2026-11-01T05:30:00Z"), // primeira ocorrência
        ("01:30", "2026-11-01T05:45:00Z", "2026-11-02T06:30:00Z"), // não repete no mesmo dia
        ("09:00", "2026-10-31T14:00:00Z", "2026-11-01T14:00:00Z"), // dia de 25 h
    ] {
        assert_eq!(
            Regra::Diario {
                horario: horario.into()
            }
            .proxima(ms(agora), &FusoDst),
            Some(ms(esperado)),
            "{horario} {agora}"
        );
    }
}
#[test]
fn semanal_obedece_fuso_e_intervalo_mede_tempo_decorrido() {
    let regra = Regra::Semanal {
        horario: "02:30".into(),
        dias: vec![6],
    };
    assert_eq!(
        regra.proxima(ms("2026-03-08T05:00:00Z"), &FusoDst),
        Some(ms("2026-03-08T07:00:00Z"))
    );
    assert_eq!(
        Regra::Intervalo { minutos: 60 }.proxima(ms("2026-03-08T06:30:00Z"), &FusoDst),
        Some(ms("2026-03-08T07:30:00Z"))
    );
    let brasil = FixedOffset::west_opt(3 * 3600).unwrap();
    assert_eq!(
        Regra::Diario {
            horario: "09:00".into()
        }
        .proxima(ms("2026-10-20T11:00:00Z"), &brasil),
        Some(ms("2026-10-20T12:00:00Z"))
    );
}
