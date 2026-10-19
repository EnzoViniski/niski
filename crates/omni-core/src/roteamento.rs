//! Fila de contas (Fase 3): decide qual perfil roda o próximo agente.
//!
//! Lógica pura, sem disco nem rede. Quem lê o `routing.json`, a cota de cada conta e conta as
//! sessões abertas é o engine; aqui só entram valores e sai a escolha com a explicação. Assim a
//! regra inteira é testável em tabela e a tela pode mostrar "por que esta conta" sem repetir a regra.
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use crate::usage::{AccountUsage, UsageWindow};

/// Assinatura cobra por janela de uso; API cobra por token. Só a API tem orçamento "normal" —
/// na assinatura o orçamento só vale para o uso extra (ver [`Conta::uso_extra`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoConta { Assinatura, Api }

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Conta {
    /// Mesmo id do `Profile` (`profiles.json`): é por ele que a fila referencia a conta.
    pub perfil_id: String,
    pub apelido: String,
    /// `claude`, `codex`, ... — igual ao `Profile::provider`.
    pub provider: String,
    pub tipo: TipoConta,
    /// Nome do plano como o usuário digitou (Pro, Max, Team...). Só exibição: limites e preços
    /// mudam e não podem ficar no código.
    #[serde(default)] pub plano: Option<String>,
    /// A conta aceita passar do limite da janela cobrando à parte. Só é usada assim se houver
    /// orçamento definido e ainda não gasto, para nunca gerar conta sem teto.
    #[serde(default)] pub uso_extra: bool,
    #[serde(default)] pub orcamento_usd: Option<f64>,
    #[serde(default)] pub gasto_usd: f64,
    /// Assento temporário, teste gratuito, chave com validade.
    #[serde(default)] pub expira_em_ms: Option<u64>,
    /// Quantas sessões simultâneas a conta aguenta. `None` = sem limite próprio.
    #[serde(default)] pub max_concorrencia: Option<u32>,
    /// Pode receber projeto marcado como confidencial (ex.: conta da empresa com retenção zero).
    /// Falso por padrão: errar para o lado seguro.
    #[serde(default)] pub confidencial_ok: bool,
}

/// Uma janela de limite (5h, semanal...) já normalizada para milissegundos.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Janela {
    pub nome: String,
    pub percentual: f64,
    pub reset_em_ms: Option<u64>,
}

/// O que se sabe da cota de uma conta. Conta sem `Cota` (ou sem janelas) é "desconhecida": não é
/// bloqueada — o dado pode só não ter chegado ainda —, mas no modo auto perde para as conhecidas.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Cota {
    pub janelas: Vec<Janela>,
    /// De onde veio (`claude_cache`, `codex_rollout`...), para a explicação.
    pub fonte: String,
    pub observado_em_ms: Option<u64>,
}

impl Cota {
    /// Converte o uso que o OMNI já lê (`usage.rs`). `None` quando a leitura falhou: vira
    /// "desconhecido" em vez de um 0% inventado.
    pub fn de_uso(uso: &AccountUsage) -> Option<Self> {
        if uso.status == "unavailable" { return None; }
        let janelas: Vec<Janela> = [&uso.primary, &uso.secondary].into_iter().flatten().map(janela_de_uso).collect();
        (!janelas.is_empty()).then(|| Cota { janelas, fonte: uso.source.clone(), observado_em_ms: uso.observed_at_ms })
    }
}

fn janela_de_uso(janela: &UsageWindow) -> Janela {
    let nome = match janela.window_minutes { 300 => "5h".to_owned(), 10080 => "semanal".to_owned(), m => format!("{m}min") };
    // `resets_at` do usage.rs está em segundos; aqui tudo é ms, como o resto do engine.
    let reset_em_ms = janela.resets_at.and_then(|s| u64::try_from(s).ok()).map(|s| s * 1000);
    Janela { nome, percentual: janela.used_percent, reset_em_ms }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Modo {
    /// Respeita a ordem: a primeira etapa que passa nos filtros leva.
    #[default] Fila,
    /// Entre as que passam, a de menor uso; a ordem só desempata.
    Auto,
}

/// Formato do `routing.json`. Ler e gravar o arquivo é do engine; aqui só a (de)serialização,
/// para o formato ter um dono só.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Politica {
    #[serde(default)] pub modo: Modo,
    /// Ids de perfil, na ordem de preferência.
    #[serde(default)] pub etapas: Vec<String>,
}

impl Politica {
    pub fn carregar(texto: &str) -> Result<Self, String> {
        serde_json::from_str(texto).map_err(|erro| format!("routing.json inválido: {erro}"))
    }
    pub fn de_valor(valor: serde_json::Value) -> Result<Self, String> {
        serde_json::from_value(valor).map_err(|erro| format!("routing.json inválido: {erro}"))
    }
    pub fn salvar(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| "{}".to_owned())
    }
}

/// O que o chamador quer rodar e o estado do momento.
#[derive(Clone, Debug, Default)]
pub struct Pedido {
    pub provider: String,
    pub confidencial: bool,
    /// Sessões abertas agora por `perfil_id`; ausente = 0.
    pub sessoes_em_uso: HashMap<String, u32>,
    pub agora_ms: u64,
    /// Execução sem tela (fluxo, agendamento). Só ela pode trocar de conta sozinha após falha.
    pub headless: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Escolha {
    pub perfil_id: Option<String>,
    /// Uma linha por conta avaliada: por que foi pulada, e por que a escolhida venceu.
    pub explicacao: Vec<String>,
}

/// Situação de uma conta que passou nos filtros, para ordenar e explicar.
struct Apta<'a> {
    conta: &'a Conta,
    /// Maior percentual entre as janelas vigentes. `None` = desconhecido.
    pior: Option<f64>,
    resumo: String,
}

/// Escolhe a conta para o pedido segundo a política.
pub fn escolher(politica: &Politica, contas: &[Conta], cotas: &HashMap<String, Cota>, pedido: &Pedido) -> Escolha {
    escolher_sem(politica, contas, cotas, pedido, &[])
}

/// Queda automática: a conta `falharam` (limite, erro de login...) sai da disputa e a fila segue.
/// Só em headless — numa sessão interativa trocar de conta no meio mudaria a conversa e o
/// histórico debaixo do usuário, então devolve `None` com o aviso para a interface mostrar.
pub fn proxima_apos_falha(politica: &Politica, contas: &[Conta], cotas: &HashMap<String, Cota>,
    pedido: &Pedido, falharam: &[&str]) -> Escolha {
    if !pedido.headless {
        return Escolha { perfil_id: None, explicacao: vec![
            "sessão interativa: sem troca automática de conta; avise o usuário para escolher outra".to_owned()] };
    }
    escolher_sem(politica, contas, cotas, pedido, falharam)
}

fn escolher_sem(politica: &Politica, contas: &[Conta], cotas: &HashMap<String, Cota>, pedido: &Pedido,
    falharam: &[&str]) -> Escolha {
    let mut explicacao = Vec::new();
    let mut vistas = HashSet::new();
    let mut aptas: Vec<Apta> = Vec::new();
    for id in &politica.etapas {
        // Conta repetida na fila não ganha segunda chance nem peso extra no desempate.
        if !vistas.insert(id.as_str()) { explicacao.push(format!("{id}: repetida na fila, ignorada")); continue; }
        let Some(conta) = contas.iter().find(|c| &c.perfil_id == id) else {
            explicacao.push(format!("{id}: não está cadastrada, pulada")); continue;
        };
        let nome = rotulo(conta);
        if falharam.contains(&id.as_str()) { explicacao.push(format!("{nome}: falhou nesta execução, pulada")); continue; }
        match avaliar(conta, cotas.get(id), pedido) {
            Err(motivo) => explicacao.push(format!("{nome}: pulada — {motivo}")),
            Ok(apta) => {
                // No modo fila a primeira apta encerra: as seguintes nem são consultadas.
                if politica.modo == Modo::Fila {
                    explicacao.push(format!("{nome}: escolhida — primeira da fila que passou nos filtros ({})", apta.resumo));
                    return Escolha { perfil_id: Some(conta.perfil_id.clone()), explicacao };
                }
                aptas.push(apta);
            }
        }
    }
    // Ordenação estável: em empate (ou entre desconhecidas) vale a ordem da fila.
    aptas.sort_by(|a, b| match (a.pior, b.pior) {
        (Some(x), Some(y)) => x.total_cmp(&y),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => std::cmp::Ordering::Equal,
    });
    let Some(vencedora) = aptas.first() else {
        explicacao.push("nenhuma conta da fila passou nos filtros".to_owned());
        return Escolha { perfil_id: None, explicacao };
    };
    let motivo = if vencedora.pior.is_some() { "menor uso entre as aptas" } else { "nenhuma apta tem cota conhecida; vale a ordem da fila" };
    explicacao.push(format!("{}: escolhida — {motivo} ({})", rotulo(vencedora.conta), vencedora.resumo));
    for outra in &aptas[1..] {
        explicacao.push(format!("{}: apta, mas perdeu no modo auto ({})", rotulo(outra.conta), outra.resumo));
    }
    Escolha { perfil_id: Some(vencedora.conta.perfil_id.clone()), explicacao }
}

fn rotulo(conta: &Conta) -> String { format!("{} ({})", conta.apelido, conta.perfil_id) }

fn orcamento_livre(conta: &Conta) -> bool { conta.orcamento_usd.is_some_and(|teto| conta.gasto_usd < teto) }

/// Filtros duros, na ordem do mais barato/definitivo para o que depende da cota.
fn avaliar<'a>(conta: &'a Conta, cota: Option<&Cota>, pedido: &Pedido) -> Result<Apta<'a>, String> {
    if conta.provider != pedido.provider {
        return Err(format!("provider {} ≠ {}", conta.provider, pedido.provider));
    }
    if pedido.confidencial && !conta.confidencial_ok {
        return Err("projeto confidencial e a conta não está liberada para ele".to_owned());
    }
    if conta.expira_em_ms.is_some_and(|fim| fim <= pedido.agora_ms) {
        return Err("conta expirada".to_owned());
    }
    if conta.tipo == TipoConta::Api && conta.orcamento_usd.is_some() && !orcamento_livre(conta) {
        return Err(format!("orçamento estourado (US$ {:.2} de {:.2})", conta.gasto_usd, conta.orcamento_usd.unwrap_or_default()));
    }
    let em_uso = pedido.sessoes_em_uso.get(&conta.perfil_id).copied().unwrap_or(0);
    if conta.max_concorrencia.is_some_and(|max| em_uso >= max) {
        return Err(format!("concorrência no máximo ({em_uso} de {})", conta.max_concorrencia.unwrap_or_default()));
    }
    let Some(cota) = cota.filter(|c| !c.janelas.is_empty()) else {
        return Ok(Apta { conta, pior: None, resumo: "cota desconhecida".to_owned() });
    };
    let mut pior = 0.0_f64;
    let mut partes = Vec::new();
    let mut estouradas = Vec::new();
    for janela in &cota.janelas {
        // Reset já passou: a janela recomeçou, o percentual guardado é de antes. Conta volta.
        if janela.reset_em_ms.is_some_and(|reset| reset <= pedido.agora_ms) {
            partes.push(format!("{} reiniciada", janela.nome));
            continue;
        }
        // Estourada sem horário de reset conhecido: tratar como ainda estourada (lado seguro).
        if janela.percentual >= 100.0 { estouradas.push(janela.nome.clone()); }
        pior = pior.max(janela.percentual);
        partes.push(format!("{} {:.0}%", janela.nome, janela.percentual));
    }
    if !estouradas.is_empty() {
        let quais = estouradas.join(", ");
        if !conta.uso_extra { return Err(format!("limite estourado ({quais})")); }
        if !orcamento_livre(conta) { return Err(format!("limite estourado ({quais}) e uso extra sem orçamento disponível")); }
        partes.push("em uso extra".to_owned());
    }
    partes.push(idade(cota, pedido.agora_ms));
    Ok(Apta { conta, pior: Some(pior), resumo: partes.join(", ") })
}

/// Dado velho decide mal; a explicação diz de onde veio e há quanto tempo.
fn idade(cota: &Cota, agora_ms: u64) -> String {
    match cota.observado_em_ms {
        Some(quando) => format!("fonte {}, há {} min", cota.fonte, agora_ms.saturating_sub(quando) / 60_000),
        None => format!("fonte {}, idade desconhecida", cota.fonte),
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    const AGORA: u64 = 1_000_000_000_000;
    const HORA: u64 = 3_600_000;

    struct Cenario { politica: Politica, contas: Vec<Conta>, cotas: HashMap<String, Cota>, pedido: Pedido }

    fn conta(id: &str, provider: &str) -> Conta {
        Conta { perfil_id: id.into(), apelido: format!("conta {id}"), provider: provider.into(), tipo: TipoConta::Assinatura,
            plano: None, uso_extra: false, orcamento_usd: None, gasto_usd: 0.0, expira_em_ms: None,
            max_concorrencia: None, confidencial_ok: false }
    }

    fn cota(percentual: f64, reset_em_ms: Option<u64>) -> Cota {
        Cota { janelas: vec![Janela { nome: "5h".into(), percentual, reset_em_ms }], fonte: "teste".into(),
            observado_em_ms: Some(AGORA - 10 * 60_000) }
    }

    /// a (50%), b (20%), c (cota desconhecida, liberada para confidencial), d (codex). Fila a, b, c.
    fn base() -> Cenario {
        let mut c = conta("c", "claude");
        c.confidencial_ok = true;
        let cotas = HashMap::from([
            ("a".to_owned(), cota(50.0, Some(AGORA + HORA))),
            ("b".to_owned(), cota(20.0, Some(AGORA + HORA))),
            ("d".to_owned(), cota(0.0, Some(AGORA + HORA))),
        ]);
        Cenario {
            politica: Politica { modo: Modo::Fila, etapas: fila(&["a", "b", "c"]) },
            contas: vec![conta("a", "claude"), conta("b", "claude"), c, conta("d", "codex")],
            cotas,
            pedido: Pedido { provider: "claude".into(), agora_ms: AGORA, headless: true, ..Default::default() },
        }
    }

    fn mexer(cenario: &mut Cenario, id: &str, ajuste: impl FnOnce(&mut Conta)) {
        ajuste(cenario.contas.iter_mut().find(|c| c.perfil_id == id).unwrap());
    }

    fn fila(ids: &[&str]) -> Vec<String> { ids.iter().map(|s| s.to_string()).collect() }

    /// Cada caso: nome, ajuste sobre a base, conta esperada e um trecho que a explicação precisa ter.
    #[test]
    fn tabela_de_escolha() {
        type Ajuste = fn(&mut Cenario);
        let casos: Vec<(&str, Ajuste, Option<&str>, &str)> = vec![
            ("fila pega a primeira apta", |_| {}, Some("a"), "primeira da fila"),
            ("auto pega o menor uso", |c| c.politica.modo = Modo::Auto, Some("b"), "menor uso"),
            ("auto deixa desconhecida por último", |c| {
                c.politica = Politica { modo: Modo::Auto, etapas: fila(&["c", "a"]) };
            }, Some("a"), "cota desconhecida"),
            ("auto só com desconhecidas segue a fila", |c| {
                c.politica = Politica { modo: Modo::Auto, etapas: fila(&["c"]) };
            }, Some("c"), "nenhuma apta tem cota conhecida"),
            ("auto empata pela ordem da fila", |c| {
                c.cotas.insert("a".into(), cota(20.0, Some(AGORA + HORA)));
                c.politica = Politica { modo: Modo::Auto, etapas: fila(&["b", "a"]) };
            }, Some("b"), "perdeu no modo auto"),
            ("provider diferente", |c| c.politica.etapas = fila(&["d", "a"]), Some("a"), "provider codex ≠ claude"),
            ("confidencial só em conta liberada", |c| c.pedido.confidencial = true, Some("c"), "projeto confidencial"),
            ("conta expirada", |c| mexer(c, "a", |a| a.expira_em_ms = Some(AGORA)), Some("b"), "conta expirada"),
            ("conta que expira no futuro passa", |c| mexer(c, "a", |a| a.expira_em_ms = Some(AGORA + 1)), Some("a"), "primeira"),
            ("api com orçamento estourado", |c| mexer(c, "a", |a| {
                a.tipo = TipoConta::Api; a.orcamento_usd = Some(10.0); a.gasto_usd = 10.0;
            }), Some("b"), "orçamento estourado"),
            ("api sem teto passa", |c| mexer(c, "a", |a| { a.tipo = TipoConta::Api; a.gasto_usd = 999.0; }), Some("a"), "primeira"),
            ("assinatura ignora orçamento fora do uso extra", |c| mexer(c, "a", |a| {
                a.orcamento_usd = Some(1.0); a.gasto_usd = 5.0;
            }), Some("a"), "primeira"),
            ("concorrência no máximo", |c| {
                mexer(c, "a", |a| a.max_concorrencia = Some(2));
                c.pedido.sessoes_em_uso.insert("a".into(), 2);
            }, Some("b"), "concorrência no máximo (2 de 2)"),
            ("concorrência abaixo do máximo passa", |c| {
                mexer(c, "a", |a| a.max_concorrencia = Some(2));
                c.pedido.sessoes_em_uso.insert("a".into(), 1);
            }, Some("a"), "primeira"),
            ("janela estourada com reset no futuro", |c| {
                c.cotas.insert("a".into(), cota(100.0, Some(AGORA + 1)));
            }, Some("b"), "limite estourado (5h)"),
            ("janela estourada sem reset conhecido", |c| { c.cotas.insert("a".into(), cota(100.0, None)); }, Some("b"), "limite estourado"),
            ("reset vencido devolve a conta", |c| {
                c.cotas.insert("a".into(), cota(100.0, Some(AGORA)));
            }, Some("a"), "5h reiniciada"),
            ("reset vencido conta como zero no auto", |c| {
                c.cotas.insert("a".into(), cota(100.0, Some(AGORA - 1)));
                c.politica.modo = Modo::Auto;
            }, Some("a"), "menor uso"),
            ("uso extra sem orçamento não salva", |c| {
                c.cotas.insert("a".into(), cota(100.0, Some(AGORA + HORA)));
                mexer(c, "a", |a| a.uso_extra = true);
            }, Some("b"), "uso extra sem orçamento"),
            ("uso extra com orçamento livre é aceito", |c| {
                c.cotas.insert("a".into(), cota(100.0, Some(AGORA + HORA)));
                mexer(c, "a", |a| { a.uso_extra = true; a.orcamento_usd = Some(20.0); a.gasto_usd = 5.0; });
            }, Some("a"), "em uso extra"),
            ("uso extra com orçamento gasto", |c| {
                c.cotas.insert("a".into(), cota(100.0, Some(AGORA + HORA)));
                mexer(c, "a", |a| { a.uso_extra = true; a.orcamento_usd = Some(20.0); a.gasto_usd = 20.0; });
            }, Some("b"), "uso extra sem orçamento"),
            ("uso extra perde para conta com folga no auto", |c| {
                c.cotas.insert("a".into(), cota(100.0, Some(AGORA + HORA)));
                mexer(c, "a", |a| { a.uso_extra = true; a.orcamento_usd = Some(20.0); });
                c.politica.modo = Modo::Auto;
            }, Some("b"), "em uso extra"),
            ("conta repetida é ignorada", |c| {
                c.cotas.insert("a".into(), cota(100.0, Some(AGORA + HORA)));
                c.politica.etapas = fila(&["a", "a", "b"]);
            }, Some("b"), "a: repetida na fila"),
            ("perfil fora do cadastro", |c| c.politica.etapas = fila(&["x", "a"]), Some("a"), "x: não está cadastrada"),
            ("ninguém passa", |c| c.politica.etapas = fila(&["d"]), None, "nenhuma conta da fila passou"),
            ("fila vazia", |c| c.politica.etapas.clear(), None, "nenhuma conta da fila passou"),
            ("explicação traz fonte e idade", |_| {}, Some("a"), "fonte teste, há 10 min"),
        ];
        for (nome, ajuste, esperado, trecho) in casos {
            let mut cenario = base();
            ajuste(&mut cenario);
            let escolha = escolher(&cenario.politica, &cenario.contas, &cenario.cotas, &cenario.pedido);
            assert_eq!(escolha.perfil_id.as_deref(), esperado, "{nome}: {:?}", escolha.explicacao);
            assert!(escolha.explicacao.iter().any(|linha| linha.contains(trecho)),
                "{nome}: faltou «{trecho}» em {:?}", escolha.explicacao);
        }
    }

    /// Repetição não pode dar peso extra: com a mesma conta duas vezes, ela aparece uma vez só
    /// entre as aptas do modo auto.
    #[test]
    fn repeticao_nao_duplica_no_auto() {
        let mut cenario = base();
        cenario.politica = Politica { modo: Modo::Auto, etapas: fila(&["b", "a", "b"]) };
        let escolha = escolher(&cenario.politica, &cenario.contas, &cenario.cotas, &cenario.pedido);
        assert_eq!(escolha.perfil_id.as_deref(), Some("b"));
        assert_eq!(escolha.explicacao.iter().filter(|l| l.starts_with("conta b")).count(), 1, "{:?}", escolha.explicacao);
    }

    #[test]
    fn queda_automatica_so_em_headless() {
        let mut cenario = base();
        let escolha = proxima_apos_falha(&cenario.politica, &cenario.contas, &cenario.cotas, &cenario.pedido, &["a"]);
        assert_eq!(escolha.perfil_id.as_deref(), Some("b"));
        assert!(escolha.explicacao.iter().any(|l| l.contains("falhou nesta execução")));

        let escolha = proxima_apos_falha(&cenario.politica, &cenario.contas, &cenario.cotas, &cenario.pedido, &["a", "b"]);
        assert_eq!(escolha.perfil_id.as_deref(), Some("c"));

        cenario.pedido.headless = false;
        let escolha = proxima_apos_falha(&cenario.politica, &cenario.contas, &cenario.cotas, &cenario.pedido, &["a"]);
        assert_eq!(escolha.perfil_id, None);
        assert!(escolha.explicacao[0].contains("sessão interativa"));
        // A escolha inicial continua valendo no interativo; só a troca sozinha é que não.
        assert_eq!(escolher(&cenario.politica, &cenario.contas, &cenario.cotas, &cenario.pedido).perfil_id.as_deref(), Some("a"));
    }

    #[test]
    fn routing_json_ida_e_volta_e_padroes() {
        let politica = Politica { modo: Modo::Auto, etapas: fila(&["claude-a", "codex-b"]) };
        let texto = politica.salvar();
        assert!(texto.contains("\"auto\""));
        assert_eq!(Politica::carregar(&texto).unwrap(), politica);
        assert_eq!(Politica::carregar("{}").unwrap(), Politica { modo: Modo::Fila, etapas: vec![] });
        assert_eq!(Politica::de_valor(serde_json::json!({ "modo": "fila", "etapas": ["x"] })).unwrap().etapas, fila(&["x"]));
        assert!(Politica::carregar("{\"modo\":\"aleatorio\"}").is_err());
    }

    #[test]
    fn cota_vem_do_usage_existente() {
        let janela = |pct, minutos, reset| UsageWindow { used_percent: pct, window_minutes: minutos, resets_at: Some(reset), reset_label: None };
        let mut uso = AccountUsage::unavailable("a", "codex", "sem dado");
        assert_eq!(Cota::de_uso(&uso), None);
        uso.status = "available".into();
        uso.reason = None;
        uso.observed_at_ms = Some(AGORA);
        uso.primary = Some(janela(40.0, 300, 2_000));
        uso.secondary = Some(janela(70.0, 10080, 3_000));
        let cota = Cota::de_uso(&uso).unwrap();
        assert_eq!(cota.fonte, "codex_rollout");
        assert_eq!(cota.janelas, vec![
            Janela { nome: "5h".into(), percentual: 40.0, reset_em_ms: Some(2_000_000) },
            Janela { nome: "semanal".into(), percentual: 70.0, reset_em_ms: Some(3_000_000) },
        ]);
    }
}
