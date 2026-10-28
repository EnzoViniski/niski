//! Calendário puro. O fuso é injetado; o engine usa o fuso local do computador.
use chrono::{DateTime, Datelike, Duration, LocalResult, NaiveTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "tipo", rename_all = "snake_case", deny_unknown_fields)]
pub enum Regra {
    UmaVez {
        em_ms: i64,
    },
    Intervalo {
        minutos: u32,
    },
    Diario {
        horario: String,
    },
    /// Segunda = 0, domingo = 6.
    Semanal {
        horario: String,
        dias: Vec<u32>,
    },
}
impl Regra {
    pub fn validar(&self) -> Result<(), String> {
        match self {
            Self::UmaVez { em_ms }
                if DateTime::<Utc>::from_timestamp_millis(*em_ms).is_some() && *em_ms > 0 =>
            {
                Ok(())
            }
            Self::Intervalo { minutos } if (5..=525_600).contains(minutos) => Ok(()),
            Self::Diario { horario } | Self::Semanal { horario, .. } => {
                if horario.len() != 5 || NaiveTime::parse_from_str(horario, "%H:%M").is_err() {
                    return Err("Horário inválido; use HH:MM".into());
                }
                if let Self::Semanal { dias, .. } = self {
                    if dias.is_empty() || dias.len() > 7 || dias.iter().any(|d| *d > 6) {
                        return Err("Escolha dias da semana válidos".into());
                    }
                }
                Ok(())
            }
            _ => Err("Data inválida ou intervalo fora de 5 a 525600 minutos".into()),
        }
    }
    /// Sempre estritamente depois de `agora`. Após sono avança a partir de agora, sem empilhar.
    /// DST: hora inexistente avança até o primeiro minuto válido; hora repetida usa a primeira
    /// ocorrência, uma única vez por dia (mesmo se agora estiver entre as duas ocorrências).
    pub fn proxima<T: TimeZone>(&self, agora: i64, fuso: &T) -> Option<i64> {
        self.validar().ok()?;
        match self {
            Self::UmaVez { em_ms } => (*em_ms > agora).then_some(*em_ms),
            Self::Intervalo { minutos } => agora.checked_add(i64::from(*minutos) * 60_000),
            Self::Diario { horario } | Self::Semanal { horario, .. } => {
                let data = DateTime::<Utc>::from_timestamp_millis(agora)?
                    .with_timezone(fuso)
                    .date_naive();
                let hora = NaiveTime::parse_from_str(horario, "%H:%M").ok()?;
                for n in 0..=8 {
                    let dia = data.checked_add_signed(Duration::days(n))?;
                    if let Self::Semanal { dias, .. } = self {
                        if !dias.contains(&dia.weekday().num_days_from_monday()) {
                            continue;
                        }
                    }
                    let local = dia.and_time(hora);
                    for minuto in 0..=180 {
                        let instante =
                            match fuso.from_local_datetime(&(local + Duration::minutes(minuto))) {
                                LocalResult::Single(d) => d.timestamp_millis(),
                                LocalResult::Ambiguous(a, b) => {
                                    a.timestamp_millis().min(b.timestamp_millis())
                                }
                                LocalResult::None => continue,
                            };
                        if instante > agora {
                            return Some(instante);
                        }
                        break;
                    }
                }
                None
            }
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "tipo", rename_all = "snake_case", deny_unknown_fields)]
pub enum Destino {
    Conversa {
        conversa: String,
    },
    NovoAgente {
        projeto: String,
        plataforma: String,
        conta: Option<String>,
        modelo: Option<String>,
        pasta: String,
        #[serde(default)]
        worktree: bool,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pedido {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub titulo: String,
    pub prompt: String,
    pub regra: Regra,
    pub destino: Destino,
}
impl Pedido {
    pub fn validar(&self) -> Result<(), String> {
        self.regra.validar()?;
        if self.titulo.trim().is_empty()
            || self.titulo.len() > 120
            || self.titulo.chars().any(char::is_control)
        {
            return Err("Título inválido".into());
        }
        if self.prompt.trim().is_empty()
            || self.prompt.len() > 16_000
            || self.prompt.trim_start().starts_with('/')
            || self
                .prompt
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err("Envie prompt de até 16000 bytes, sem comandos slash".into());
        }
        match &self.destino {
            Destino::Conversa { conversa } if !conversa.is_empty() && conversa.len() <= 120 => {
                Ok(())
            }
            Destino::NovoAgente {
                projeto,
                plataforma,
                conta,
                modelo,
                pasta,
                ..
            } => {
                if projeto.is_empty()
                    || pasta.is_empty()
                    || pasta.len() > 4096
                    || !["claude", "codex", "antigravity", "cursor"].contains(&plataforma.as_str())
                    || conta
                        .as_ref()
                        .is_some_and(|s| s == "automatica" || s.len() > 120)
                    || modelo
                        .as_ref()
                        .is_some_and(|s| !crate::comandos_oficiais::valor_seguro(s))
                {
                    return Err("Destino inválido (automática aguarda a fila de contas)".into());
                }
                Ok(())
            }
            _ => Err("Conversa inválida".into()),
        }
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    fn ms(s: &str) -> i64 {
        DateTime::parse_from_rfc3339(s).unwrap().timestamp_millis()
    }
    #[test]
    fn calendario_em_tabela() {
        for (regra, agora, esperado) in [
            (
                Regra::Diario {
                    horario: "09:00".into(),
                },
                "2026-01-31T10:00:00Z",
                "2026-02-01T09:00:00Z",
            ),
            (
                Regra::Diario {
                    horario: "09:00".into(),
                },
                "2028-02-28T10:00:00Z",
                "2028-02-29T09:00:00Z",
            ),
            (
                Regra::Semanal {
                    horario: "09:00".into(),
                    dias: vec![0],
                },
                "2026-10-19T09:00:00Z",
                "2026-10-26T09:00:00Z",
            ),
            (
                Regra::Semanal {
                    horario: "09:00".into(),
                    dias: vec![6, 0],
                },
                "2026-10-24T10:00:00Z",
                "2026-10-25T09:00:00Z",
            ),
            (
                Regra::Intervalo { minutos: 60 },
                "2026-12-31T23:30:00Z",
                "2027-01-01T00:30:00Z",
            ),
        ] {
            assert_eq!(
                regra.proxima(ms(agora), &Utc),
                Some(ms(esperado)),
                "{regra:?}"
            );
        }
    }
    #[test]
    fn validacao_e_uma_vez() {
        assert!(Regra::Intervalo { minutos: 4 }.validar().is_err());
        assert!(Regra::Diario {
            horario: "25:00".into()
        }
        .validar()
        .is_err());
        assert!(Regra::Semanal {
            horario: "09:00".into(),
            dias: vec![]
        }
        .validar()
        .is_err());
        assert_eq!(Regra::UmaVez { em_ms: 10 }.proxima(10, &Utc), None);
    }
}

#[cfg(test)]
#[path = "agendamentos_dst_testes.rs"]
mod dst_testes;
