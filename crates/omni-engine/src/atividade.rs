//! Fork: atividade observada no engine. Só os ids vistos vão para disco; nenhum texto de conversa.
//! Filho de `mobile` para compartilhar a API sem alterar o `main.rs` da frente Ganchos.
use super::*;
use omni_protocol::TerminalSession;
use std::collections::BTreeSet;

const MAX_ITENS: usize = 500;
const MAX_VISTOS: usize = 4_096;
const ARQUIVO: &str = "atividade-vista.json";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Tipo { Agendamento, PrecisaDeVoce, Terminou, Erro, Limite, SubagenteTerminou }

impl Tipo {
    fn resumo(self) -> &'static str {
        match self {
            Self::Agendamento => "Agendamento",
            Self::PrecisaDeVoce => "Precisa de você",
            Self::Terminou => "Terminou o turno",
            Self::Erro => "O agente encontrou um erro",
            Self::Limite => "Limite de uso atingido",
            Self::SubagenteTerminou => "Subagente do orquestrador terminou",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct Item {
    id: String,
    conversa: String,
    sessao: String,
    projeto_id: String,
    projeto: String,
    titulo: String,
    plataforma: String,
    conta_id: Option<String>,
    conta: String,
    tipo: Tipo,
    desde_ms: u64,
    visto: bool,
    resumo: String,
    orquestrador: Option<String>,
}

pub(super) struct Registro {
    itens: Vec<Item>,
    vistos: BTreeSet<String>,
    ultimos: HashMap<String, String>,
}

impl Registro {
    pub(super) fn abrir(dir: &Path) -> Self {
        Self { itens: Vec::new(), vistos: omni_protocol::read_json_or_default(&dir.join(ARQUIVO)), ultimos: HashMap::new() }
    }

    fn observar(&mut self, itens: Vec<Item>) {
        for mut item in itens {
            // Uma conversa cujo item saiu do limite de 500 não volta como notícia antiga.
            if self.ultimos.insert(item.conversa.clone(), item.id.clone()).as_ref() == Some(&item.id) { continue; }
            if self.itens.iter().any(|anterior| anterior.id == item.id) { continue; }
            item.visto = self.vistos.contains(&item.id);
            self.itens.push(item);
        }
        self.itens.sort_by(|a, b| b.desde_ms.cmp(&a.desde_ms).then_with(|| a.id.cmp(&b.id)));
        self.itens.truncate(MAX_ITENS);
    }

    fn marcar(&mut self, dir: &Path, ids: &[String]) -> std::io::Result<()> {
        // POSTs do PC e do celular passam pela mesma trava, incluindo a escrita atômica.
        let mut vistos = self.vistos.clone();
        for item in &self.itens {
            if ids.contains(&item.id) { vistos.insert(item.id.clone()); }
        }
        if vistos == self.vistos { return Ok(()); }
        if vistos.len() > MAX_VISTOS {
            let atuais: BTreeSet<_> = self.itens.iter().map(|i| i.id.as_str()).collect();
            vistos.retain(|id| atuais.contains(id.as_str()));
        }
        atomic_write_json(&dir.join(ARQUIVO), &vistos)?;
        self.vistos = vistos;
        for item in &mut self.itens { item.visto = self.vistos.contains(&item.id); }
        Ok(())
    }
}

/// Ponto de ligação futuro com `ganchos::tipo(...)`. Até lá, só sinais estruturados atuais.
fn classificar(meta: &TerminalSession, aprovar: bool, escolha: bool, subagente: bool) -> Option<Tipo> {
    if aprovar || escolha || meta.state == SessionState::ApprovalRequired { return Some(Tipo::PrecisaDeVoce); }
    if meta.notice.as_deref() == Some("usage_limit") { return Some(Tipo::Limite); }
    if meta.notice.as_deref() == Some("api_error") || meta.state == SessionState::Crashed { return Some(Tipo::Erro); }
    // Fork (Telegram): sessão interrompida depois de ter atividade também chama o usuário.
    if meta.state == SessionState::Stopped && meta.attention_seq > 0 { return Some(Tipo::Erro); }
    if meta.state == SessionState::Answered && meta.attention_seq > 0 {
        return Some(if subagente { Tipo::SubagenteTerminou } else { Tipo::Terminou });
    }
    None // shell ocioso, CLI recém-aberta e sessão órfã não significam fim de turno.
}

/// Fork (pergunta-da-cli): o "precisa de você" diz quando é a própria CLI perguntando no terminal.
fn resumo(tipo: Tipo, cap: Option<&interaction::Capabilities>) -> &'static str {
    match cap {
        Some(c) if tipo == Tipo::PrecisaDeVoce && !c.approve && c.escolha.as_ref().is_some_and(|m| m.da_cli) => "A CLI está perguntando no terminal",
        Some(c) if tipo == Tipo::PrecisaDeVoce && !c.approve && c.escolha.is_none() && c.espera.is_some() => "Esperando algo no terminal",
        _ => tipo.resumo(),
    }
}

fn coletar(state: &EngineState) -> Vec<Item> {
    let dir = state.state_file.parent().unwrap();
    let indice = omni_core::conversations::read_index(dir);
    let perfis = omni_core::profiles(dir);
    let projetos = state.mobile.workspace.lock().expect("workspace poisoned").projects.clone();
    let registro = omni_core::orquestracao::ler(dir);
    // Copia as referências primeiro: nunca segura a trava de sessões durante leitura de arquivo.
    let sessoes: HashMap<_, _> = state.sessions.lock().expect("sessions poisoned").iter()
        .map(|(id, s)| (id.clone(), match s {
            SessionEntry::Live(s) => (Some(s.clone()), None),
            SessionEntry::Historical(meta) => (None, Some(meta.clone())),
        })).collect();
    let mut itens = Vec::new();
    for conversa in indice {
        let Some(segmento) = conversa.segments.last() else { continue };
        let Some((viva, antiga)) = segmento.terminal_session_id.as_ref().and_then(|id| sessoes.get(id)) else { continue };
        let meta = viva.as_ref().map(|s| settle(s)).or_else(|| antiga.clone()).unwrap();
        // Ignora uma ligação antiga/reutilizada, como `linked_session` da API da conversa.
        if meta.conversation_id.as_deref() != Some(&conversa.id) || meta.profile_id != segmento.profile_id
            || meta.provider.as_deref() != Some(&segmento.provider) { continue; }
        let cap = viva.as_ref().map(interaction::capabilities);
        let orquestrador = registro.orquestradores.iter()
            .find(|o| o.agentes.iter().any(|a| a.conversation_key == conversa.id)).map(|o| o.orquestrador.clone());
        // Fork (pergunta-da-cli): menu da CLI ou espera no terminal também é "precisa de você".
        let Some(tipo) = classificar(&meta, cap.as_ref().is_some_and(|c| c.approve),
            cap.as_ref().is_some_and(|c| c.escolha.is_some() || c.espera.is_some()), orquestrador.is_some()) else { continue };
        let resumo = if meta.state == SessionState::Stopped { "O agente foi parado" } else { resumo(tipo, cap.as_ref()) };
        let tipo_id = serde_json::to_value(tipo).unwrap();
        let conta = perfis.iter().find(|p| Some(&p.id) == meta.profile_id.as_ref()).map(|p| p.name.clone())
            .unwrap_or_else(|| if meta.profile_id.is_none() { "Padrão".into() } else { "Conta indisponível".into() });
        let projeto = projetos.iter().find(|p| p.id == conversa.project_id).map(|p| p.name.clone())
            .unwrap_or_else(|| Path::new(&conversa.cwd).file_name().map(|p| p.to_string_lossy().into_owned()).unwrap_or_else(|| conversa.project_id.clone()));
        itens.push(Item {
            id: format!("{}:{}:{}:{}", conversa.id, meta.id, meta.attention_seq, tipo_id.as_str().unwrap()),
            titulo: omni_core::conversations::display_title(&conversa), conversa: conversa.id, sessao: meta.id,
            projeto_id: conversa.project_id, projeto, plataforma: segmento.provider.clone(),
            conta_id: meta.profile_id, conta, tipo, desde_ms: meta.last_activity_at_ms.max(meta.created_at_ms),
            visto: false, resumo: resumo.into(), orquestrador,
        });
    }
    itens.extend(super::agendamentos::atividade(state).into_iter().filter_map(|i| serde_json::from_value(i).ok())); // Fork: tipo agendamento (3D).
    itens
}

fn atualizar(state: &EngineState) {
    let itens = coletar(state);
    state.mobile.atividade.lock().expect("atividade poisoned").observar(itens);
}

// Fork (Telegram): cópia dos eventos atuais; não expõe títulos/transcripts além da escolha do usuário.
pub(super) fn para_avisos(state: &EngineState) -> Vec<Value> {
    let registro = state.mobile.atividade.lock().expect("atividade poisoned");
    registro.itens.iter().filter(|i| registro.ultimos.get(&i.conversa) == Some(&i.id))
        .rev().filter_map(|i| serde_json::to_value(i).ok()).collect()
}

#[cfg(test)]
pub(super) fn registrar_fixture(state: &EngineState, item: Value) {
    state.mobile.atividade.lock().unwrap().observar(vec![serde_json::from_value(item).unwrap()]);
}

pub(super) fn iniciar(state: Arc<EngineState>) {
    tokio::spawn(async move {
        let mut intervalo = tokio::time::interval(std::time::Duration::from_secs(2));
        intervalo.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            intervalo.tick().await;
            let state = state.clone();
            let _ = tokio::task::spawn_blocking(move || atualizar(&state)).await;
        }
    });
}

pub(super) fn rotas() -> Router<Arc<EngineState>> {
    Router::new().route("/atividade", get(listar)).route("/atividade/vistos", post(marcar))
}

async fn listar(State(state): State<Arc<EngineState>>, headers: HeaderMap) -> Result<Response, ApiError> {
    let corpo = tokio::task::spawn_blocking(move || {
        atualizar(&state);
        json!(state.mobile.atividade.lock().expect("atividade poisoned").itens)
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao ler a atividade"))?;
    Ok(com_etag(&headers, &corpo))
}

#[derive(Deserialize)]
struct Vistos { ids: Vec<String> }

async fn marcar(State(state): State<Arc<EngineState>>, Json(body): Json<Vistos>) -> Result<Json<Value>, ApiError> {
    if body.ids.len() > MAX_ITENS || body.ids.iter().any(|id| id.len() > 512) {
        return Err(error(StatusCode::BAD_REQUEST, "Lista de vistos inválida"));
    }
    tokio::task::spawn_blocking(move || {
        state.mobile.atividade.lock().expect("atividade poisoned").marcar(state.state_file.parent().unwrap(), &body.ids)
            .map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Não foi possível guardar os vistos"))?;
        Ok(Json(json!({"ok":true})))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR, "Falha ao marcar a atividade"))?
}

#[cfg(test)]
#[path = "atividade_testes.rs"]
mod testes;
