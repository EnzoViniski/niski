//! Fork (Rotina): todas as operações da rotina num lugar só, por nome e argumentos JSON. A tela
//! (rotas do engine), as ferramentas MCP do agente da rotina e os botões do Telegram chamam
//! [`executar`] — a mesma validação e a mesma gravação para os três.
//!
//! Erros são textos curtos para o agente/usuário corrigir o pedido. Nunca citam conteúdo pessoal
//! além do campo errado.

use super::ajustes::{Ajustes, MODOS_DE_CONVERSA};
use super::alimentacao::{Alimentacao, Refeicao, RefeicaoPlanejada};
use super::dia;
use super::lembretes::{Estado, Lembretes, Personalizado, TIPOS_PERSONALIZADOS};
use super::plano::{Bloco, Excecao, Imprevisto, Meta, Plano, TIPOS_DE_BLOCO};
use super::treino::{ExercicioDaFicha, Ficha, Serie, Sessao, Treino};
use super::*;
use chrono::{NaiveDate, NaiveTime};
use serde_json::{json, Map, Value};

/// O relógio de quem chama (o engine passa o local; os testes, um fixo).
#[derive(Clone, Copy, Debug)]
pub struct Agora {
    pub ms: i64,
    pub hoje: NaiveDate,
    pub hora: NaiveTime,
}

/// Nomes de todas as operações (as rotas e o MCP conferem contra esta lista).
pub const OPERACOES: &[&str] = &[
    "ler_dia", "ler_semana", "planejar_semana", "salvar_bloco", "remover_bloco", "ajustar_dia", "marcar", "salvar_meta",
    "remover_meta", "anotar_memoria", "salvar_area", "ler_treino", "salvar_ficha", "remover_ficha", "registrar_serie",
    "registrar_treino", "remover_serie", "concluir_treino", "ajustar_treino", "ler_alimentacao", "registrar_refeicao",
    "remover_refeicao", "registrar_agua", "salvar_plano_alimentar", "ler_lembretes", "salvar_lembretes", "salvar_lembrete",
    "remover_lembrete", "ler_ajustes", "salvar_ajustes", "ler_avisos", "responder_aviso", "ver_avisos",
];

/// Operações que só leem (o resto grava).
pub fn so_le(nome: &str) -> bool {
    nome.starts_with("ler_")
}

pub fn existe(nome: &str) -> bool {
    OPERACOES.contains(&nome)
}

/* ── Leitura dos argumentos ───────────────────────────────────────────────────────────────── */

fn txt(args: &Value, campo: &str, max: usize) -> Option<String> {
    texto_de(args, campo).map(|t| limpar(&t, max))
}

fn obrigatorio(args: &Value, campo: &str, max: usize) -> Result<String, String> {
    txt(args, campo, max).filter(|t| !t.is_empty()).ok_or_else(|| format!("Falta o campo `{campo}`"))
}

fn data_ou_hoje(args: &Value, campo: &str, agora: &Agora) -> Result<NaiveDate, String> {
    match txt(args, campo, 20).filter(|t| !t.is_empty()) {
        Some(t) => normalizar_data(&t, campo),
        None => Ok(agora.hoje),
    }
}

fn id_ou_novo(args: &Value, prefixo: &str, agora: &Agora) -> Result<String, String> {
    match txt(args, "id", 80).filter(|t| !t.is_empty()) {
        Some(id) if id_valido(&id) => Ok(id),
        Some(_) => Err("`id` inválido".into()),
        None => Ok(novo_id(prefixo, agora.ms)),
    }
}

fn tarefas(dados: &Path) -> Vec<Value> {
    crate::tarefas::contexto(dados).map(|c| crate::tarefas::todas(&c.raiz)).unwrap_or_default()
}

/* ── Execução ─────────────────────────────────────────────────────────────────────────────── */

/// Roda a operação `nome`. Segura a trava da rotina do começo ao fim.
pub fn executar(dados: &Path, nome: &str, args: &Value, agora: &Agora) -> Result<Value, String> {
    let vazio = json!({});
    let args = if args.is_object() { args } else { &vazio };
    let _trava = travar();
    match nome {
        "ler_dia" => ler_dia(dados, args, agora),
        "ler_semana" => ler_semana(dados, args, agora),
        "planejar_semana" => planejar_semana(dados, args, agora),
        "salvar_bloco" => mudar::<Plano, _>(dados, |p| salvar_bloco(p, args, agora)),
        "remover_bloco" => mudar::<Plano, _>(dados, |p| {
            let id = obrigatorio(args, "id", 80)?;
            let antes = p.blocos.len();
            p.blocos.retain(|b| b.id != id);
            if antes == p.blocos.len() { return Err("Bloco não encontrado (veja os ids em ler_semana)".into()); }
            p.excecoes.retain(|e| e.bloco != id);
            Ok(json!({"ok": true, "removido": id}))
        }),
        "ajustar_dia" => ajustar_dia(dados, args, agora),
        "marcar" => marcar(dados, args, agora),
        "salvar_meta" => mudar::<Plano, _>(dados, |p| salvar_meta(p, args, agora)),
        "remover_meta" => mudar::<Plano, _>(dados, |p| {
            let id = obrigatorio(args, "id", 80)?;
            let segunda = segunda_de(data_ou_hoje(args, "semana", agora)?);
            let semana = p.semana_mut(segunda);
            let antes = semana.metas.len();
            semana.metas.retain(|m| m.id != id);
            if antes == semana.metas.len() { return Err("Meta não encontrada nesta semana".into()); }
            Ok(json!({"ok": true}))
        }),
        "anotar_memoria" => mudar::<Plano, _>(dados, |p| {
            let texto = txt(args, "texto", 8000).unwrap_or_default();
            if txt(args, "modo", 20).as_deref() == Some("acrescentar") {
                if !texto.is_empty() {
                    if !p.memoria.is_empty() { p.memoria.push('\n'); }
                    p.memoria.push_str(&texto);
                }
            } else {
                p.memoria = texto;
            }
            p.memoria = limpar(&p.memoria, 12_000);
            Ok(json!({"ok": true, "tamanho": p.memoria.chars().count()}))
        }),
        "salvar_area" => mudar::<Plano, _>(dados, |p| {
            let id = obrigatorio(args, "id", 40)?;
            if !id_valido(&id) { return Err("`id` da área: letras, números e hífen".into()); }
            let mut area = p.area(&id).cloned().unwrap_or(super::plano::Area { id: id.clone(), cor: "#64748b".into(), ..Default::default() });
            if let Some(n) = txt(args, "nome", 60) { area.nome = n; }
            if let Some(c) = txt(args, "cor", 9) {
                if !(c.len() == 7 && c.starts_with('#') && c[1..].chars().all(|x| x.is_ascii_hexdigit())) { return Err("`cor` é #rrggbb".into()); }
                area.cor = c;
            }
            if let Some(c) = txt(args, "categoria", 40) {
                if !c.is_empty() && !crate::tarefas::CATEGORIAS.contains(&c.as_str()) {
                    return Err(format!("`categoria` é uma de: {}", crate::tarefas::CATEGORIAS.join(", ")));
                }
                area.categoria = c;
            }
            if let Some(e) = txt(args, "etiqueta", 40) { area.etiqueta = e; }
            if area.nome.is_empty() { area.nome = id.clone(); }
            match p.areas.iter_mut().find(|a| a.id == id) { Some(a) => *a = area, None => p.areas.push(area) }
            Ok(json!({"ok": true, "area": id}))
        }),
        "ler_treino" => ler_treino(dados, args, agora),
        "salvar_ficha" => mudar::<Treino, _>(dados, |t| salvar_ficha(t, args, agora)),
        "remover_ficha" => mudar::<Treino, _>(dados, |t| {
            let id = obrigatorio(args, "id", 80)?;
            let antes = t.fichas.len();
            t.fichas.retain(|f| f.id != id);
            if antes == t.fichas.len() { return Err("Ficha não encontrada".into()); }
            Ok(json!({"ok": true}))
        }),
        "registrar_serie" => mudar::<Treino, _>(dados, |t| registrar_serie(t, args, agora)),
        "registrar_treino" => mudar::<Treino, _>(dados, |t| registrar_treino(t, args, agora)),
        "remover_serie" => mudar::<Treino, _>(dados, |t| {
            let id = obrigatorio(args, "sessao", 80)?;
            let indice = inteiro_de(args, "indice").ok_or("Falta o `indice` da série")?;
            let sessao = t.sessoes.iter_mut().find(|s| s.id == id).ok_or("Sessão não encontrada")?;
            if indice < 0 || indice as usize >= sessao.series.len() { return Err("Série não encontrada".into()); }
            sessao.series.remove(indice as usize);
            Ok(json!({"ok": true, "series": sessao.series.len()}))
        }),
        "concluir_treino" => mudar::<Treino, _>(dados, |t| {
            let id = match txt(args, "sessao", 80).filter(|s| !s.is_empty()) {
                Some(id) => id,
                None => t.sessao_do_dia(data_ou_hoje(args, "data", agora)?).map(|s| s.id.clone()).ok_or("Nenhum treino registrado hoje")?,
            };
            let sessao = t.sessoes.iter_mut().find(|s| s.id == id).ok_or("Sessão não encontrada")?;
            sessao.concluida = booleano_de(args, "concluida").unwrap_or(true);
            if sessao.concluida && sessao.fim.is_empty() { sessao.fim = texto_da_hora(agora.hora); }
            if let Some(obs) = txt(args, "obs", 1000) { sessao.obs = obs; }
            Ok(json!({"ok": true, "sessao": id, "series": sessao.series.len(), "volume": sessao.volume()}))
        }),
        "ajustar_treino" => mudar::<Treino, _>(dados, |t| {
            if let Some(n) = inteiro_de(args, "meta_semanal") { t.meta_semanal = n.clamp(0, 14) as u32; }
            Ok(json!({"ok": true, "meta_semanal": t.meta_semanal}))
        }),
        "ler_alimentacao" => ler_alimentacao(dados, args, agora),
        "registrar_refeicao" => mudar::<Alimentacao, _>(dados, |a| registrar_refeicao(a, args, agora)),
        "remover_refeicao" => mudar::<Alimentacao, _>(dados, |a| {
            let dia = data_ou_hoje(args, "data", agora)?;
            let id = obrigatorio(args, "id", 80)?;
            let d = a.dia_mut(dia);
            let antes = d.refeicoes.len();
            d.refeicoes.retain(|r| r.id != id);
            if antes == d.refeicoes.len() { return Err("Refeição não encontrada nesse dia".into()); }
            Ok(json!({"ok": true}))
        }),
        "registrar_agua" => mudar::<Alimentacao, _>(dados, |a| {
            let dia = data_ou_hoje(args, "data", agora)?;
            let ml = match (inteiro_de(args, "ml"), inteiro_de(args, "copos")) {
                (Some(ml), _) => ml,
                (None, Some(c)) => c * i64::from(a.copo_ml.max(50)),
                (None, None) => i64::from(a.copo_ml.max(50)),
            };
            let total = a.somar_agua(dia, ml.clamp(-5000, 5000));
            Ok(json!({"ok": true, "agua_ml": total, "meta_ml": a.meta_agua_ml}))
        }),
        "salvar_plano_alimentar" => mudar::<Alimentacao, _>(dados, |a| salvar_plano_alimentar(a, args, agora)),
        "ler_lembretes" => Ok(serde_json::to_value(ler::<Lembretes>(dados)).unwrap_or_default()),
        "salvar_lembretes" => mudar::<Lembretes, _>(dados, |l| salvar_lembretes(l, args)),
        "salvar_lembrete" => mudar::<Lembretes, _>(dados, |l| salvar_lembrete(l, args, agora)),
        "remover_lembrete" => mudar::<Lembretes, _>(dados, |l| {
            let id = obrigatorio(args, "id", 80)?;
            let antes = l.personalizados.len();
            l.personalizados.retain(|p| p.id != id);
            if antes == l.personalizados.len() { return Err("Lembrete não encontrado".into()); }
            Ok(json!({"ok": true}))
        }),
        "ler_ajustes" => Ok(serde_json::to_value(ler::<Ajustes>(dados)).unwrap_or_default()),
        "salvar_ajustes" => mudar::<Ajustes, _>(dados, |a| salvar_ajustes(a, args)),
        "ler_avisos" => {
            let estado: Estado = ler(dados);
            let nao_vistos = estado.avisos.iter().filter(|a| !a.visto).count();
            Ok(json!({"nao_vistos": nao_vistos, "avisos": estado.avisos.iter().rev().take(30).collect::<Vec<_>>()}))
        }
        "ver_avisos" => mudar::<Estado, _>(dados, |e| { e.marcar_vistos(); Ok(json!({"ok": true})) }),
        "responder_aviso" => responder_aviso(dados, args, agora),
        _ => Err(format!("Operação desconhecida: {}", limpar(nome, 40))),
    }
}

/* ── Plano ────────────────────────────────────────────────────────────────────────────────── */

fn fontes_e<R>(dados: &Path, f: impl FnOnce(&dia::Fontes) -> R) -> R {
    let (plano, alimentacao, treino, lembretes) = (ler::<Plano>(dados), ler::<Alimentacao>(dados), ler::<Treino>(dados), ler::<Lembretes>(dados));
    let tarefas = tarefas(dados);
    f(&dia::Fontes { plano: &plano, alimentacao: &alimentacao, treino: &treino, lembretes: &lembretes, tarefas: &tarefas })
}

fn ler_dia(dados: &Path, args: &Value, agora: &Agora) -> Result<Value, String> {
    let d = data_ou_hoje(args, "data", agora)?;
    Ok(fontes_e(dados, |f| serde_json::to_value(dia::montar_dia(f, d, agora.hoje, Some(agora.hora))).unwrap_or_default()))
}

fn ler_semana(dados: &Path, args: &Value, agora: &Agora) -> Result<Value, String> {
    let d = data_ou_hoje(args, "semana", agora)?;
    let mut v = fontes_e(dados, |f| serde_json::to_value(dia::montar_semana(f, d, agora.hoje, Some(agora.hora))).unwrap_or_default());
    // Os blocos fixos (com dias) para o agente editar pelo id.
    let plano: Plano = ler(dados);
    v["blocos_fixos"] = serde_json::to_value(&plano.blocos).unwrap_or_default();
    v["hoje"] = json!(texto_da_data(agora.hoje));
    Ok(v)
}

fn bloco_de(args: &Value, mut b: Bloco) -> Result<Bloco, String> {
    if let Some(t) = txt(args, "titulo", 120) { b.titulo = t; }
    if let Some(a) = txt(args, "area", 40) { b.area = a; }
    if let Some(t) = txt(args, "tipo", 30) {
        if !TIPOS_DE_BLOCO.contains(&t.as_str()) { return Err(format!("`tipo` é um de: {}", TIPOS_DE_BLOCO.join(", "))); }
        b.tipo = t;
    }
    if args.get("dias").is_some() { b.dias = dias_de(args.get("dias"))?; }
    if let Some(h) = txt(args, "inicio", 8) { b.inicio = normalizar_hora(&h, "inicio")?; }
    if let Some(h) = txt(args, "fim", 8) { b.fim = normalizar_hora(&h, "fim")?; }
    if let Some(d) = txt(args, "data", 20) {
        b.data = if d.is_empty() { d } else { texto_da_data(normalizar_data(&d, "data")?) };
    }
    if let Some(l) = booleano_de(args, "lembrar") { b.lembrar = l; }
    if let Some(l) = txt(args, "local", 120) { b.local = l; }
    if let Some(n) = txt(args, "notas", 1000) { b.notas = n; }
    if b.titulo.is_empty() { return Err("Falta o `titulo` do bloco".into()); }
    if b.inicio.is_empty() { return Err("Falta o `inicio` (HH:MM) do bloco".into()); }
    if b.tipo.is_empty() { b.tipo = "outro".into(); }
    if b.fim.is_empty() { b.fim = b.inicio.clone(); }
    if b.data.is_empty() && b.dias.is_empty() { return Err("Bloco fixo precisa de `dias` (0 = segunda … 6 = domingo) ou de uma `data`".into()); }
    Ok(b)
}

fn salvar_bloco(p: &mut Plano, args: &Value, agora: &Agora) -> Result<Value, String> {
    let id = id_ou_novo(args, "b", agora)?;
    let existente = p.blocos.iter().find(|b| b.id == id).cloned();
    let novo = existente.is_none();
    let base = existente.unwrap_or_else(|| Bloco { id: id.clone(), lembrar: true, ..Default::default() });
    let mut bloco = bloco_de(args, base)?;
    // Sono não lembra por padrão (ninguém quer aviso para dormir, a não ser que peça).
    if novo && bloco.tipo == "sono" && booleano_de(args, "lembrar").is_none() { bloco.lembrar = false; }
    match p.blocos.iter_mut().find(|b| b.id == id) { Some(b) => *b = bloco, None => p.blocos.push(bloco) }
    Ok(json!({"ok": true, "bloco": id, "novo": novo}))
}

fn planejar_semana(dados: &Path, args: &Value, agora: &Agora) -> Result<Value, String> {
    let segunda = segunda_de(data_ou_hoje(args, "semana", agora)?);
    let resultado = mudar::<Plano, _>(dados, |p| {
        if let Some(Value::Array(blocos)) = args.get("blocos_fixos") {
            let mut novos = Vec::new();
            for (n, b) in blocos.iter().enumerate() {
                let id = txt(b, "id", 80).filter(|i| id_valido(i)).unwrap_or_else(|| novo_id("b", agora.ms + n as i64));
                let base = p.blocos.iter().find(|x| x.id == id).cloned().unwrap_or(Bloco { id, lembrar: true, ..Default::default() });
                let mut bloco = bloco_de(b, base).map_err(|e| format!("blocos_fixos[{n}]: {e}"))?;
                bloco.data.clear();
                if bloco.tipo == "sono" && booleano_de(b, "lembrar").is_none() { bloco.lembrar = false; }
                novos.push(bloco);
            }
            // Os fixos são trocados pela lista; os avulsos (com data) ficam.
            p.blocos.retain(|b| !b.data.is_empty());
            p.blocos.extend(novos);
        }
        if let Some(Value::Array(blocos)) = args.get("blocos_da_semana") {
            for (n, b) in blocos.iter().enumerate() {
                let base = Bloco { id: novo_id("b", agora.ms + 1000 + n as i64), lembrar: true, ..Default::default() };
                let bloco = bloco_de(b, base).map_err(|e| format!("blocos_da_semana[{n}]: {e}"))?;
                if bloco.data.is_empty() { return Err(format!("blocos_da_semana[{n}]: precisa de `data`")); }
                p.blocos.push(bloco);
            }
        }
        let semana = p.semana_mut(segunda);
        if let Some(f) = txt(args, "foco", 500) { semana.foco = f; }
        if let Some(n) = txt(args, "notas", 8000) { semana.notas = n; }
        if let Some(Value::Array(metas)) = args.get("metas") {
            let mut novas = Vec::new();
            for (n, m) in metas.iter().enumerate() {
                let m = if let Value::String(s) = m { json!({"texto": s}) } else { m.clone() };
                let id = txt(&m, "id", 80).filter(|i| id_valido(i)).unwrap_or_else(|| novo_id("m", agora.ms + n as i64));
                let base = semana.metas.iter().find(|x| x.id == id).cloned().unwrap_or(Meta { id, ..Default::default() });
                novas.push(meta_de(&m, base).map_err(|e| format!("metas[{n}]: {e}"))?);
            }
            semana.metas = novas;
        }
        if booleano_de(args, "concluir_planejamento").unwrap_or(true) {
            semana.planejada_em = iso(agora.ms);
        }
        let resposta = json!({"ok": true, "semana": texto_da_data(segunda), "metas": semana.metas.len(), "planejada": !semana.planejada_em.is_empty()});
        p.podar(agora.hoje);
        Ok(resposta)
    })?;
    let mut v = resultado;
    v["blocos_fixos"] = json!(ler::<Plano>(dados).blocos.iter().filter(|b| b.data.is_empty()).count());
    Ok(v)
}

fn meta_de(args: &Value, mut m: Meta) -> Result<Meta, String> {
    if let Some(t) = txt(args, "texto", 300) { m.texto = t; }
    if let Some(a) = txt(args, "area", 40) { m.area = a; }
    if args.get("alvo").is_some() { m.alvo = numero_de(args, "alvo").filter(|n| *n > 0.0); }
    if let Some(p) = numero_de(args, "progresso") { m.progresso = p.max(0.0); }
    if let Some(s) = numero_de(args, "somar") { m.progresso = (m.progresso + s).max(0.0); }
    if let Some(u) = txt(args, "unidade", 30) { m.unidade = u; }
    if let Some(c) = booleano_de(args, "concluida") { m.concluida = c; }
    if m.texto.is_empty() { return Err("Falta o `texto` da meta".into()); }
    Ok(m)
}

fn salvar_meta(p: &mut Plano, args: &Value, agora: &Agora) -> Result<Value, String> {
    let segunda = segunda_de(data_ou_hoje(args, "semana", agora)?);
    let id = id_ou_novo(args, "m", agora)?;
    let semana = p.semana_mut(segunda);
    let base = semana.metas.iter().find(|m| m.id == id).cloned().unwrap_or(Meta { id: id.clone(), ..Default::default() });
    let meta = meta_de(args, base)?;
    let fracao = meta.fracao();
    match semana.metas.iter_mut().find(|m| m.id == id) { Some(m) => *m = meta, None => semana.metas.push(meta) }
    Ok(json!({"ok": true, "meta": id, "progresso": fracao}))
}

fn ajustar_dia(dados: &Path, args: &Value, agora: &Agora) -> Result<Value, String> {
    let dia = data_ou_hoje(args, "data", agora)?;
    let texto_dia = texto_da_data(dia);
    let nota = txt(args, "nota", 1000).unwrap_or_default();
    mudar::<Plano, _>(dados, |p| {
        let mut feitas = 0;
        if let Some(Value::Array(mudancas)) = args.get("mudancas") {
            for (n, m) in mudancas.iter().enumerate() {
                let bloco = obrigatorio(m, "bloco", 80).map_err(|e| format!("mudancas[{n}]: {e}"))?;
                let original = p.blocos.iter().find(|b| b.id == bloco).cloned()
                    .ok_or_else(|| format!("mudancas[{n}]: bloco não encontrado (ids em ler_dia/ler_semana)"))?;
                let acao = txt(m, "acao", 20).unwrap_or_else(|| "mover".into());
                p.excecoes.retain(|e| !(e.data == texto_dia && e.bloco == bloco));
                match acao.as_str() {
                    "restaurar" => {}
                    "cancelar" => p.excecoes.push(Excecao { data: texto_dia.clone(), bloco, cancelado: true, nota: nota.clone(), ..Default::default() }),
                    "mover" => {
                        let inicio = normalizar_hora(&txt(m, "inicio", 8).unwrap_or_default(), "inicio").map_err(|e| format!("mudancas[{n}]: {e}"))?;
                        let fim = normalizar_hora(&txt(m, "fim", 8).unwrap_or_default(), "fim").map_err(|e| format!("mudancas[{n}]: {e}"))?;
                        if inicio.is_empty() && fim.is_empty() { return Err(format!("mudancas[{n}]: mover precisa de `inicio` e/ou `fim`")); }
                        // Bloco avulso (só deste dia) muda direto.
                        if !original.data.is_empty() {
                            if let Some(b) = p.blocos.iter_mut().find(|b| b.id == original.id) {
                                if !inicio.is_empty() { b.inicio = inicio; }
                                if !fim.is_empty() { b.fim = fim; }
                            }
                        } else {
                            p.excecoes.push(Excecao { data: texto_dia.clone(), bloco, cancelado: false, inicio, fim, nota: nota.clone() });
                        }
                    }
                    _ => return Err(format!("mudancas[{n}]: `acao` é cancelar, mover ou restaurar")),
                }
                feitas += 1;
            }
        }
        if let Some(Value::Array(novos)) = args.get("novos_blocos") {
            for (n, b) in novos.iter().enumerate() {
                let base = Bloco { id: novo_id("b", agora.ms + n as i64), data: texto_dia.clone(), lembrar: true, ..Default::default() };
                let mut bloco = bloco_de(b, base).map_err(|e| format!("novos_blocos[{n}]: {e}"))?;
                if bloco.data.is_empty() { bloco.data = texto_dia.clone(); }
                p.blocos.push(bloco);
                feitas += 1;
            }
        }
        if !nota.is_empty() {
            let semana = p.semana_mut(segunda_de(dia));
            semana.imprevistos.push(Imprevisto { em: iso(agora.ms), data: texto_dia.clone(), texto: nota.clone() });
            if semana.imprevistos.len() > 100 { semana.imprevistos.remove(0); }
        }
        p.podar(agora.hoje);
        Ok(json!({"ok": true, "data": texto_dia, "mudancas": feitas, "nota": "Tarefas do dia se remarcam com editar_tarefa (data/hora)."}))
    })
}

fn marcar(dados: &Path, args: &Value, agora: &Agora) -> Result<Value, String> {
    let dia = data_ou_hoje(args, "data", agora)?;
    let item = obrigatorio(args, "item", 120)?;
    let estado = txt(args, "estado", 10).unwrap_or_else(|| "feito".into());
    if let Some(id) = item.strip_prefix("tarefa:") {
        if estado != "pulado" {
            let ctx = crate::tarefas::contexto(dados).map_err(|e| e.to_string())?;
            crate::tarefas::concluir(&ctx.raiz, id, estado == "feito", agora.ms.max(0) as u128, agora.hoje).map_err(|e| e.to_string())?;
            return Ok(json!({"ok": true, "item": item, "estado": estado}));
        }
    }
    mudar::<Plano, _>(dados, |p| {
        p.marcar(dia, &item, &estado)?;
        Ok(json!({"ok": true, "item": item, "estado": estado}))
    })
}

/* ── Treino ───────────────────────────────────────────────────────────────────────────────── */

fn ler_treino(dados: &Path, args: &Value, agora: &Agora) -> Result<Value, String> {
    let t: Treino = ler(dados);
    let semanas = inteiro_de(args, "semanas").unwrap_or(12).clamp(1, 104) as u32;
    let mut recentes: Vec<&Sessao> = t.ordenadas();
    recentes.reverse();
    let mut v = json!({
        "hoje": texto_da_data(agora.hoje),
        "fichas": t.fichas,
        "meta_semanal": t.meta_semanal,
        "proxima_ficha": t.proxima_ficha(agora.hoje).map(|f| f.id.clone()),
        "sessao_de_hoje": t.sessao_do_dia(agora.hoje),
        "sessoes": recentes.iter().take(inteiro_de(args, "sessoes").unwrap_or(20).clamp(1, 200) as usize)
            .map(|s| json!({"id": s.id, "data": s.data, "ficha": s.ficha, "series": s.series, "volume": (s.volume() * 10.0).round() / 10.0,
                "concluida": s.concluida, "obs": s.obs, "inicio": s.inicio, "fim": s.fim})).collect::<Vec<_>>(),
        "frequencia": t.frequencia(agora.hoje, semanas).into_iter().map(|(s, n)| json!({"semana": s, "treinos": n})).collect::<Vec<_>>(),
        "exercicios": t.exercicios(),
    });
    if let Some(e) = txt(args, "exercicio", 80).filter(|e| !e.is_empty()) {
        v["evolucao"] = serde_json::to_value(t.evolucao(&e)).unwrap_or_default();
    }
    Ok(v)
}

fn salvar_ficha(t: &mut Treino, args: &Value, agora: &Agora) -> Result<Value, String> {
    let id = id_ou_novo(args, "f", agora)?;
    let mut ficha = t.fichas.iter().find(|f| f.id == id).cloned().unwrap_or(Ficha { id: id.clone(), ..Default::default() });
    if let Some(n) = txt(args, "nome", 20) { ficha.nome = n; }
    if let Some(n) = txt(args, "titulo", 120) { ficha.titulo = n; }
    if args.get("dias").is_some() { ficha.dias = dias_de(args.get("dias"))?; }
    if let Some(Value::Array(exercicios)) = args.get("exercicios") {
        ficha.exercicios = exercicios.iter().enumerate().map(|(n, e)| {
            let e = if let Value::String(s) = e { json!({"nome": s}) } else { e.clone() };
            let nome = obrigatorio(&e, "nome", 80).map_err(|x| format!("exercicios[{n}]: {x}"))?;
            Ok(ExercicioDaFicha { nome, series: inteiro_de(&e, "series").unwrap_or(3).clamp(1, 20) as u32,
                repeticoes: txt(&e, "repeticoes", 20).unwrap_or_else(|| "10".into()),
                carga_kg: numero_de(&e, "carga_kg").filter(|c| *c >= 0.0), descanso_s: inteiro_de(&e, "descanso_s").unwrap_or(90).clamp(0, 900) as u32,
                obs: txt(&e, "obs", 300).unwrap_or_default() })
        }).collect::<Result<_, String>>()?;
    }
    if ficha.nome.is_empty() { return Err("Falta o `nome` da ficha (ex.: A)".into()); }
    match t.fichas.iter_mut().find(|f| f.id == id) { Some(f) => *f = ficha, None => t.fichas.push(ficha) }
    Ok(json!({"ok": true, "ficha": id}))
}

/// A sessão do dia (a última aberta) ou uma nova com a ficha pedida/sugerida.
fn sessao_para(t: &mut Treino, dia: NaiveDate, ficha: Option<String>, agora: &Agora) -> Result<usize, String> {
    let texto = texto_da_data(dia);
    let ficha = match ficha.filter(|f| !f.is_empty()) {
        Some(f) => Some(t.ficha(&f).map(|x| x.id.clone()).ok_or("Ficha não encontrada (ids em ler_treino)")?),
        None => None,
    };
    if let Some(i) = t.sessoes.iter().rposition(|s| s.data == texto && ficha.as_ref().is_none_or(|f| &s.ficha == f || s.ficha.is_empty())) {
        if let (Some(f), true) = (&ficha, t.sessoes[i].ficha.is_empty()) { t.sessoes[i].ficha = f.clone(); }
        return Ok(i);
    }
    let ficha = ficha.or_else(|| t.proxima_ficha(dia).map(|f| f.id.clone())).unwrap_or_default();
    t.sessoes.push(Sessao { id: novo_id("s", agora.ms), data: texto, ficha, inicio: texto_da_hora(agora.hora), ..Default::default() });
    Ok(t.sessoes.len() - 1)
}

fn serie_de(args: &Value, agora: &Agora) -> Result<Serie, String> {
    let exercicio = obrigatorio(args, "exercicio", 80)?;
    let carga = numero_de(args, "carga_kg").unwrap_or(0.0);
    if !(0.0..=1000.0).contains(&carga) { return Err("`carga_kg` entre 0 e 1000".into()); }
    let reps = inteiro_de(args, "repeticoes").ok_or("Falta `repeticoes`")?;
    if !(0..=500).contains(&reps) { return Err("`repeticoes` entre 0 e 500".into()); }
    Ok(Serie { exercicio, carga_kg: (carga * 100.0).round() / 100.0, repeticoes: reps as u32, obs: txt(args, "obs", 300).unwrap_or_default(),
        em: txt(args, "em", 8).and_then(|h| hora(&h)).map(texto_da_hora).unwrap_or_else(|| texto_da_hora(agora.hora)) })
}

fn registrar_serie(t: &mut Treino, args: &Value, agora: &Agora) -> Result<Value, String> {
    let dia = data_ou_hoje(args, "data", agora)?;
    let serie = serie_de(args, agora)?;
    let antes = t.evolucao(&serie.exercicio);
    let recorde = !antes.pontos.is_empty()
        && (serie.carga_kg > antes.recorde_carga || treino::um_rm(serie.carga_kg, serie.repeticoes) > antes.recorde_um_rm + 0.05);
    let i = sessao_para(t, dia, txt(args, "ficha", 80), agora)?;
    t.sessoes[i].series.push(serie.clone());
    t.podar();
    let sessao = &t.sessoes[i];
    let do_exercicio = sessao.series.iter().filter(|s| chave(&s.exercicio) == chave(&serie.exercicio)).count();
    Ok(json!({"ok": true, "sessao": sessao.id, "ficha": sessao.ficha, "series": sessao.series.len(), "serie_do_exercicio": do_exercicio,
        "recorde": recorde, "recorde_anterior_kg": antes.recorde_carga}))
}

fn registrar_treino(t: &mut Treino, args: &Value, agora: &Agora) -> Result<Value, String> {
    let dia = data_ou_hoje(args, "data", agora)?;
    let series: Vec<Serie> = match args.get("series") {
        Some(Value::Array(lista)) => lista.iter().enumerate().map(|(n, s)| serie_de(s, agora).map_err(|e| format!("series[{n}]: {e}"))).collect::<Result<_, _>>()?,
        _ => Vec::new(),
    };
    let i = sessao_para(t, dia, txt(args, "ficha", 80), agora)?;
    let sessao = &mut t.sessoes[i];
    sessao.series.extend(series);
    if let Some(h) = txt(args, "inicio", 8) { sessao.inicio = normalizar_hora(&h, "inicio")?; }
    if let Some(h) = txt(args, "fim", 8) { sessao.fim = normalizar_hora(&h, "fim")?; }
    if let Some(o) = txt(args, "obs", 1000) { sessao.obs = o; }
    if let Some(c) = booleano_de(args, "concluir") { sessao.concluida = c; }
    let (id, n, vol) = (sessao.id.clone(), sessao.series.len(), sessao.volume());
    t.podar();
    Ok(json!({"ok": true, "sessao": id, "series": n, "volume": vol}))
}

/* ── Alimentação ──────────────────────────────────────────────────────────────────────────── */

fn ler_alimentacao(dados: &Path, args: &Value, agora: &Agora) -> Result<Value, String> {
    let a: Alimentacao = ler(dados);
    let dia = data_ou_hoje(args, "data", agora)?;
    let dias = inteiro_de(args, "dias").unwrap_or(14).clamp(1, 120);
    let historico: Vec<Value> = (0..dias).map(|n| {
        let d = dia - chrono::Duration::days(n);
        let t = a.totais(d);
        json!({"data": texto_da_data(d), "kcal": t.kcal, "proteina_g": t.proteina_g, "refeicoes": t.refeicoes, "agua_ml": t.agua_ml})
    }).collect();
    Ok(json!({
        "data": texto_da_data(dia), "plano": a.plano, "meta_agua_ml": a.meta_agua_ml, "copo_ml": a.copo_ml,
        "meta_kcal": a.meta_kcal, "meta_proteina_g": a.meta_proteina_g, "dia": a.dia(dia), "totais": a.totais(dia), "historico": historico,
    }))
}

fn registrar_refeicao(a: &mut Alimentacao, args: &Value, agora: &Agora) -> Result<Value, String> {
    let dia = data_ou_hoje(args, "data", agora)?;
    let id = id_ou_novo(args, "r", agora)?;
    let plano = a.plano.clone();
    let mut planejada = txt(args, "planejada", 80).unwrap_or_default();
    let nome = txt(args, "nome", 80).unwrap_or_default();
    if planejada.is_empty() && !nome.is_empty() {
        planejada = plano.iter().find(|p| chave(&p.nome) == chave(&nome) || chave(&p.id) == chave(&nome)).map(|p| p.id.clone()).unwrap_or_default();
    }
    let do_plano = plano.iter().find(|p| p.id == planejada);
    let texto = txt(args, "texto", 1000).unwrap_or_default();
    if texto.is_empty() && do_plano.is_none() && nome.is_empty() { return Err("Diga o que comeu em `texto`".into()); }
    let hora_t = match txt(args, "hora", 8).filter(|h| !h.is_empty()) { Some(h) => normalizar_hora(&h, "hora")?, None => texto_da_hora(agora.hora) };
    let refeicao = Refeicao {
        id: id.clone(), hora: hora_t,
        nome: if nome.is_empty() { do_plano.map(|p| p.nome.clone()).unwrap_or_else(|| "Refeição".into()) } else { nome },
        texto: if texto.is_empty() { do_plano.map(|p| p.sugestao.clone()).unwrap_or_default() } else { texto },
        kcal: numero_de(args, "kcal").filter(|n| *n >= 0.0), proteina_g: numero_de(args, "proteina_g").filter(|n| *n >= 0.0),
        planejada: do_plano.map(|p| p.id.clone()).unwrap_or_default(),
    };
    let d = a.dia_mut(dia);
    match d.refeicoes.iter_mut().find(|r| r.id == id) { Some(r) => *r = refeicao, None => d.refeicoes.push(refeicao) }
    d.refeicoes.sort_by(|x, y| x.hora.cmp(&y.hora));
    a.podar(agora.hoje);
    let t = a.totais(dia);
    Ok(json!({"ok": true, "refeicao": id, "totais": t}))
}

fn salvar_plano_alimentar(a: &mut Alimentacao, args: &Value, agora: &Agora) -> Result<Value, String> {
    if let Some(Value::Array(lista)) = args.get("refeicoes") {
        a.plano = lista.iter().enumerate().map(|(n, r)| {
            let nome = obrigatorio(r, "nome", 80).map_err(|e| format!("refeicoes[{n}]: {e}"))?;
            let id = txt(r, "id", 80).filter(|i| id_valido(i)).unwrap_or_else(|| novo_id("rp", agora.ms + n as i64));
            Ok(RefeicaoPlanejada { id, nome, hora: normalizar_hora(&txt(r, "hora", 8).unwrap_or_default(), "hora").map_err(|e| format!("refeicoes[{n}]: {e}"))?,
                sugestao: txt(r, "sugestao", 500).unwrap_or_default(), kcal: numero_de(r, "kcal"), proteina_g: numero_de(r, "proteina_g") })
        }).collect::<Result<_, String>>()?;
    }
    if let Some(n) = inteiro_de(args, "meta_agua_ml") { a.meta_agua_ml = n.clamp(0, 10_000) as u32; }
    if let Some(n) = inteiro_de(args, "copo_ml") { a.copo_ml = n.clamp(50, 2000) as u32; }
    if args.get("meta_kcal").is_some() { a.meta_kcal = numero_de(args, "meta_kcal").filter(|n| *n > 0.0); }
    if args.get("meta_proteina_g").is_some() { a.meta_proteina_g = numero_de(args, "meta_proteina_g").filter(|n| *n > 0.0); }
    Ok(json!({"ok": true, "refeicoes": a.plano.len(), "meta_agua_ml": a.meta_agua_ml}))
}

/* ── Lembretes e ajustes ──────────────────────────────────────────────────────────────────── */

fn salvar_lembretes(l: &mut Lembretes, args: &Value) -> Result<Value, String> {
    for (campo, alvo) in [("ativo", &mut l.ativo), ("tarefas", &mut l.tarefas), ("blocos", &mut l.blocos), ("refeicoes", &mut l.refeicoes), ("treino", &mut l.treino)] {
        if let Some(b) = booleano_de(args, campo) { *alvo = b; }
    }
    if let Some(n) = inteiro_de(args, "antecedencia_min") { l.antecedencia_min = n.clamp(0, 240) as u32; }
    if let Some(h) = txt(args, "acordar", 8) { l.acordar = normalizar_hora(&h, "acordar")?; }
    if let Some(h) = txt(args, "dormir", 8) { l.dormir = normalizar_hora(&h, "dormir")?; }
    if let Some(agua) = args.get("agua") {
        if let Some(b) = booleano_de(agua, "ativo") { l.agua.ativo = b; }
        if let Some(n) = inteiro_de(agua, "intervalo_min") { l.agua.intervalo_min = n.clamp(15, 600) as u32; }
    }
    if let Some(p) = args.get("planejar") {
        if let Some(b) = booleano_de(p, "ativo") { l.planejar.ativo = b; }
        if p.get("dia").is_some() { l.planejar.dia = *dias_de(Some(&json!([p["dia"].clone()])))?.first().ok_or("`planejar.dia` inválido")?; }
        if let Some(h) = txt(p, "hora", 8) { l.planejar.hora = normalizar_hora(&h, "planejar.hora")?; }
    }
    Ok(serde_json::to_value(&*l).unwrap_or_default())
}

fn salvar_lembrete(l: &mut Lembretes, args: &Value, agora: &Agora) -> Result<Value, String> {
    let id = id_ou_novo(args, "l", agora)?;
    let mut p = l.personalizados.iter().find(|p| p.id == id).cloned().unwrap_or(Personalizado { id: id.clone(), ativo: true, tipo: "livre".into(), ..Default::default() });
    if let Some(t) = txt(args, "texto", 500) { p.texto = t; }
    if let Some(h) = txt(args, "hora", 8) { p.hora = normalizar_hora(&h, "hora")?; }
    if args.get("dias").is_some() { p.dias = dias_de(args.get("dias"))?; }
    if let Some(d) = txt(args, "data", 20) { p.data = if d.is_empty() { d } else { texto_da_data(normalizar_data(&d, "data")?) }; }
    if let Some(t) = txt(args, "tipo", 20) {
        if !TIPOS_PERSONALIZADOS.contains(&t.as_str()) { return Err(format!("`tipo` é um de: {}", TIPOS_PERSONALIZADOS.join(", "))); }
        p.tipo = t;
    }
    if let Some(a) = booleano_de(args, "ativo") { p.ativo = a; }
    if p.texto.is_empty() || p.hora.is_empty() { return Err("Lembrete precisa de `texto` e `hora`".into()); }
    match l.personalizados.iter_mut().find(|x| x.id == id) { Some(x) => *x = p, None => l.personalizados.push(p) }
    Ok(json!({"ok": true, "lembrete": id}))
}

fn salvar_ajustes(a: &mut Ajustes, args: &Value) -> Result<Value, String> {
    if let Some(p) = txt(args, "plataforma", 40) {
        if !p.is_empty() && !crate::orquestracao::pode_orquestrar(&p) {
            return Err("O agente da rotina precisa de ferramentas só da sessão: use claude ou codex".into());
        }
        a.plataforma = p;
    }
    for (campo, alvo, max) in [("conta", &mut a.conta, 120), ("modelo", &mut a.modelo, 80), ("esforco", &mut a.esforco, 20)] {
        if let Some(v) = txt(args, campo, max) { *alvo = v; }
    }
    if let Some(m) = txt(args, "modo_conversa", 20) {
        if !MODOS_DE_CONVERSA.contains(&m.as_str()) { return Err("`modo_conversa` é semanal ou continua".into()); }
        a.modo_conversa = m;
    }
    if let Some(b) = booleano_de(args, "telegram_lembretes") { a.telegram_lembretes = b; }
    if let Some(b) = booleano_de(args, "telegram_conversa") { a.telegram_conversa = b; }
    let mut v = serde_json::to_value(&*a).unwrap_or_default();
    if let Some(o) = v.as_object_mut() { o.remove("conversas"); }
    Ok(v)
}

/// Resposta a um lembrete (botão na tela ou no Telegram): `feito`, `adiar`, `pular`, `copo`,
/// `planejar` ou `visto`.
fn responder_aviso(dados: &Path, args: &Value, agora: &Agora) -> Result<Value, String> {
    let chave_aviso = obrigatorio(args, "chave", 200)?;
    let acao = obrigatorio(args, "acao", 20)?;
    let estado: Estado = ler(dados);
    let aviso = estado.aviso(&chave_aviso).cloned().ok_or("Lembrete não encontrado (já saiu da lista)")?;
    // Já respondido (dois toques no botão do Telegram, tela e Telegram ao mesmo tempo): não repete o
    // efeito — sem segundo adiado, sem copo a mais, sem desmarcar o que já foi feito.
    if !aviso.resposta.is_empty() && acao != "visto" {
        return Ok(json!({"ok": true, "acao": acao, "tipo": aviso.disparo.tipo, "ja_respondido": aviso.resposta}));
    }
    let d = aviso.disparo;
    let dia = data(&d.data).unwrap_or(agora.hoje);
    let item = match d.tipo.as_str() {
        "tarefa" => format!("tarefa:{}", d.alvo),
        "bloco" | "treino" => format!("bloco:{}", d.alvo),
        "refeicao" => format!("refeicao:{}", d.alvo),
        "lembrete" => format!("lembrete:{}", d.alvo),
        outro => outro.to_owned(),
    };
    let mut saida = json!({"ok": true, "acao": acao, "tipo": d.tipo});
    let resposta = match acao.as_str() {
        "adiar" => {
            let minutos = inteiro_de(args, "minutos").unwrap_or(15).clamp(1, 24 * 60) as u32;
            let novo = mudar::<Estado, _>(dados, |e| e.adiar(&chave_aviso, minutos, agora.ms))?;
            saida["novo_em_ms"] = json!(novo.em_ms);
            return Ok(saida);
        }
        "copo" => {
            let total = mudar::<Alimentacao, _>(dados, |a| { let ml = i64::from(a.copo_ml.max(50)); Ok(a.somar_agua(dia, ml)) })?;
            saida["agua_ml"] = json!(total);
            "copo"
        }
        "feito" if d.tipo == "agua" => {
            let total = mudar::<Alimentacao, _>(dados, |a| { let ml = i64::from(a.copo_ml.max(50)); Ok(a.somar_agua(dia, ml)) })?;
            saida["agua_ml"] = json!(total);
            "feito"
        }
        "feito" if d.tipo == "tarefa" => {
            let ctx = crate::tarefas::contexto(dados).map_err(|e| e.to_string())?;
            crate::tarefas::concluir(&ctx.raiz, &d.alvo, true, agora.ms.max(0) as u128, agora.hoje).map_err(|e| e.to_string())?;
            "feito"
        }
        "feito" | "pular" if d.tipo != "agua" && d.tipo != "planejar" => {
            let marca = if acao == "feito" { "feito" } else { "pulado" };
            if !(d.tipo == "tarefa" && marca == "pulado") {
                mudar::<Plano, _>(dados, |p| p.marcar(dia, &item, marca))?;
            }
            if marca == "feito" { "feito" } else { "pulado" }
        }
        "pular" => "pulado",
        "planejar" => "planejar",
        "visto" => "",
        _ => return Err("`acao` é feito, adiar, pular, copo, planejar ou visto".into()),
    };
    mudar::<Estado, _>(dados, |e| {
        if resposta.is_empty() {
            if let Some(a) = e.avisos.iter_mut().rev().find(|a| a.disparo.chave == chave_aviso) { a.visto = true; }
        } else {
            e.responder(&chave_aviso, resposta);
        }
        Ok(())
    })?;
    Ok(saida)
}

/// Objeto JSON vazio, para quem chama sem argumentos.
pub fn sem_argumentos() -> Value {
    Value::Object(Map::new())
}
