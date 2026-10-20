//! Fork (chat-estavel): trocar modelo e esforço de um Codex aberto que ainda não recebeu mensagem.
//!
//! O Codex só grava o rollout (e o id nativo) depois do primeiro prompt, e a troca exigia esse id
//! para o `codex resume` — o seletor ficava travado em "Mande a primeira mensagem antes de trocar".
//! Sem conversa ainda não há nada a perder: basta reabrir a CLI com as opções novas. O reinício
//! (`retomar_codex::comando`) ainda procura o rollout antes de digitar e, se ele já existir, retoma
//! a mesma conversa com estas opções.
use super::*;

/// Um pedaço do comando fora de aspas: `inicio..fim` no texto original e o valor sem as aspas.
struct Pedaco { inicio: usize, fim: usize, valor: String }

/// Separa o comando em palavras como o shell faria com aspas simples e duplas (sem expansão). O
/// texto entre aspas fica intacto: só se remove pedaço inteiro, nunca se reescreve o meio dele.
fn pedacos(comando: &str) -> Vec<Pedaco> {
    let mut lista = Vec::new();
    let mut atual: Option<(usize, String)> = None;
    let mut aspa: Option<char> = None;
    for (posicao, letra) in comando.char_indices() {
        match (aspa, letra) {
            (Some(fechar), _) if letra == fechar => aspa = None,
            (Some(_), _) => atual.get_or_insert_with(|| (posicao, String::new())).1.push(letra),
            (None, '\'' | '"') => { aspa = Some(letra); atual.get_or_insert_with(|| (posicao, String::new())); }
            (None, _) if letra.is_whitespace() => {
                if let Some((inicio, valor)) = atual.take() { lista.push(Pedaco { inicio, fim: posicao, valor }); }
            }
            (None, _) => atual.get_or_insert_with(|| (posicao, String::new())).1.push(letra),
        }
    }
    if let Some((inicio, valor)) = atual { lista.push(Pedaco { inicio, fim: comando.len(), valor }); }
    lista
}

const ESFORCO: &str = "model_reasoning_effort=";

/// Índices dos pedaços que dizem o modelo (`-m x`, `--model x`, `--model=x`) e o esforço
/// (`-c model_reasoning_effort=x`), com o valor de cada um.
fn opcoes(lista: &[Pedaco]) -> (Vec<usize>, Option<String>, Option<String>) {
    let (mut indices, mut modelo, mut esforco) = (Vec::new(), None, None);
    let mut i = 0;
    while i < lista.len() {
        let valor = lista[i].valor.as_str();
        let seguinte = lista.get(i + 1).map(|p| p.valor.as_str());
        if matches!(valor, "-m" | "--model") && seguinte.is_some() {
            modelo = seguinte.map(str::to_owned);
            indices.extend([i, i + 1]); i += 2; continue;
        }
        if let Some(resto) = valor.strip_prefix("--model=") {
            modelo = Some(resto.to_owned()); indices.push(i); i += 1; continue;
        }
        if matches!(valor, "-c" | "--config") {
            if let Some(resto) = seguinte.and_then(|s| s.strip_prefix(ESFORCO)) {
                esforco = Some(resto.trim_matches('"').to_owned());
                indices.extend([i, i + 1]); i += 2; continue;
            }
        }
        i += 1;
    }
    (indices, modelo, esforco)
}

/// Modelo e esforço que o comando digitado escolhe (`None` = o padrão da conta).
pub(super) fn ajustes_do_comando(comando: &str) -> (Option<String>, Option<String>) {
    let (_, modelo, esforco) = opcoes(&pedacos(comando));
    (modelo, esforco)
}

/// O mesmo comando com o modelo e o esforço trocados: tira as opções antigas e põe as novas no fim,
/// na sintaxe do `comando_de_retomada`. O resto da linha (memória, `--oss`, aspas) fica como estava.
pub(super) fn trocar_no_comando(comando: &str, ajustes: &Ajustes) -> String {
    let lista = pedacos(comando);
    let (indices, _, _) = opcoes(&lista);
    let mut saida = String::with_capacity(comando.len() + 48);
    let mut desde = 0;
    for &i in &indices {
        // Some junto o espaço que vem antes do pedaço.
        let antes = comando[..lista[i].inicio].trim_end();
        if antes.len() >= desde { saida.push_str(&comando[desde..antes.len()]); }
        desde = lista[i].fim;
    }
    saida.push_str(&comando[desde..]);
    let mut saida = saida.trim_end().to_owned();
    if let Some(modelo) = &ajustes.model { saida += &format!(" -m {modelo}"); }
    if let Some(esforco) = &ajustes.effort { saida += &format!(" -c {ESFORCO}{esforco}"); }
    saida
}

/// Codex aberto, sem id nativo: troca as opções do comando da sessão (e a conta, se pedida) e reabre.
pub(super) fn reabrir_codex_sem_mensagem(state: &Arc<EngineState>, conversation_id: &str, sessao: &Arc<LiveSession>,
    segmento: &omni_core::conversations::Segment, ajustes: &Ajustes) -> Result<()> {
    let dir = state.state_file.parent().unwrap().to_path_buf();
    let perfis = omni_core::profiles(&dir);
    let alvo_id = ajustes.profile_id.clone().or(segmento.profile_id.clone());
    let alvo = perfis.iter().find(|p| Some(&p.id) == alvo_id.as_ref() && p.provider == "codex");
    if alvo_id.is_some() && alvo.is_none() { return Err(anyhow!("Conta não encontrada para este agente")); }
    let sessao_id = {
        let mut meta = sessao.meta.lock().map_err(|_| anyhow!("metadata poisoned"))?;
        let base = meta.initial_command.clone().unwrap_or_else(|| "codex".into());
        meta.initial_command = Some(trocar_no_comando(&base, ajustes));
        meta.env.retain(|(chave, _)| chave != "CODEX_HOME");
        if let Some(alvo) = alvo { meta.env.extend(omni_core::env_for(alvo)); }
        meta.profile_id = alvo_id.clone();
        meta.id.clone()
    };
    if alvo_id != segmento.profile_id {
        // Conta nova: o segmento passa a apontar para ela (a descoberta do rollout procura lá).
        omni_core::indice_de_conversas::mudar_indice(&dir, |indice| {
            let Some(ultimo) = indice.iter_mut().find(|c| c.id == conversation_id).and_then(|c| c.segments.last_mut()) else { return false };
            ultimo.profile_id = alvo_id.clone();
            true
        })?;
    }
    restart_session(state, &sessao_id)?;
    Ok(())
}

#[cfg(test)]
mod testes {
    use super::*;

    fn ajustes(model: Option<&str>, effort: Option<&str>) -> Ajustes {
        Ajustes { profile_id: None, model: model.map(str::to_owned), effort: effort.map(str::to_owned) }
    }

    #[test]
    fn le_modelo_e_esforco_do_comando() {
        assert_eq!(ajustes_do_comando("codex -m gpt-x -c model_reasoning_effort=high"), (Some("gpt-x".into()), Some("high".into())));
        assert_eq!(ajustes_do_comando("codex --model=gpt-y"), (Some("gpt-y".into()), None));
        assert_eq!(ajustes_do_comando("codex"), (None, None));
        // Outro `-c` não é esforço.
        assert_eq!(ajustes_do_comando("codex -c 'developer_instructions=\"use -m x\"'"), (None, None));
    }

    #[test]
    fn troca_so_as_opcoes_de_modelo_e_esforco() {
        assert_eq!(trocar_no_comando("codex -m gpt-x -c model_reasoning_effort=high", &ajustes(Some("gpt-z"), Some("low"))),
            "codex -m gpt-z -c model_reasoning_effort=low");
        assert_eq!(trocar_no_comando("codex", &ajustes(Some("gpt-z"), None)), "codex -m gpt-z");
        // Voltar ao padrão tira as opções.
        assert_eq!(trocar_no_comando("codex --full-auto -m gpt-x", &ajustes(None, None)), "codex --full-auto");
        // Texto entre aspas (memória, instruções) passa intacto, inclusive espaços duplos.
        let memoria = "codex -c 'developer_instructions=\"a  -m b\"' -m gpt-x";
        assert_eq!(trocar_no_comando(memoria, &ajustes(None, Some("medium"))),
            "codex -c 'developer_instructions=\"a  -m b\"' -c model_reasoning_effort=medium");
    }
}
