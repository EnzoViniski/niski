//! Terminal da conversa (espelho da PTY) e terminais sem agente (shells) abertos pela web.
//! Saiu do antigo `mobile.rs` na Onda 2.0, sem mudança de lógica.
use super::*;

/// Quanto do histórico do terminal vai na primeira leitura. O engine guarda até 5 MB; mandar tudo
/// pela rede para desenhar uma tela é desperdício — 512 KB cobrem bem mais que a tela visível.
pub(super) const TERMINAL_INICIAL_BYTES: usize = 512 * 1024;
#[derive(Deserialize)] pub(super) struct Desde { #[serde(default)] desde: Option<u64> }

/// O terminal da conversa como o PC desenha: a saída bruta da PTY a partir de `desde`.
///
/// Só leitura de propósito — **não** marca a sessão como exibida (`headless`): quem responde às
/// perguntas do terminal continua sendo o xterm do desktop ou o próprio engine. Se a página também
/// respondesse, a resposta chegaria em dobro e vazaria como texto digitado dentro da CLI.
pub(super) async fn terminal(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Query(desde): Query<Desde>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let (_, sessao) = current(&state,&id)?;
        let Some(sessao) = sessao else {
            return Ok(Json(json!({"live":false,"data":"","from_seq":0,"next_seq":0,"cols":0,"rows":0})));
        };
        ler_saida(&sessao, desde.desde)
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao ler o terminal"))?
}

/// A saída bruta da PTY a partir de `desde` — comum ao terminal de uma conversa e a um shell.
pub(super) fn ler_saida(sessao: &Arc<LiveSession>, desde: Option<u64>) -> Result<Json<Value>, ApiError> {
    {
        let (cols, rows) = sessao.meta.lock().map(|meta| (meta.cols, meta.rows)).unwrap_or((120, 40));
        let saida = sessao.output.lock().map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Terminal indisponível"))?;
        let pedido = desde.unwrap_or(0);
        // Primeira leitura (ou cliente que ficou para trás do buffer): só o fim, começando num bloco
        // inteiro. Daí em diante vem só o que é novo.
        let mut inicio = saida.chunks.front().map(|(seq,_)| *seq).unwrap_or(saida.next_seq).max(pedido);
        if pedido == 0 || saida.chunks.front().is_some_and(|(seq,_)| *seq > pedido) {
            let mut bytes = 0;
            inicio = saida.next_seq;
            for (seq, bloco) in saida.chunks.iter().rev() {
                if bytes + bloco.len() > TERMINAL_INICIAL_BYTES && bytes > 0 { break; }
                bytes += bloco.len();
                inicio = *seq;
            }
        }
        let data: String = saida.chunks.iter().filter(|(seq,_)| *seq >= inicio).map(|(_,bloco)| bloco.as_str()).collect();
        Ok(Json(json!({"live":true,"data":data,"from_seq":inicio,"next_seq":saida.next_seq,"cols":cols,"rows":rows,
            "reset":inicio != pedido})))
    }
}

/* ── Terminais sem agente (shells) ───────────────────────────────────────────────────────────
   Para quem está no celular ou no outro computador conseguir rodar um `npm run dev` num projeto.
   A pasta sai **sempre** da lista de projetos: o navegador escolhe o projeto, nunca um caminho. */

/// Um shell vivo pelo id. Sessão de agente não serve: aquela se lê pela conversa.
pub(super) fn shell_vivo(state: &EngineState, id: &str) -> Result<Arc<LiveSession>, ApiError> {
    let sessoes = state.sessions.lock().map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Sessões indisponíveis"))?;
    match sessoes.get(id) {
        Some(SessionEntry::Live(sessao)) if sessao.meta.lock().map(|meta| meta.provider.is_none()).unwrap_or(false) => Ok(sessao.clone()),
        _ => Err(error(StatusCode::NOT_FOUND,"Terminal não está aberto no PC")),
    }
}

pub(super) async fn listar_terminais(State(state): State<Arc<EngineState>>) -> Json<Value> {
    let lista: Vec<Value> = state.sessions.lock().map(|sessoes| sessoes.values().filter_map(|entrada| match entrada {
        SessionEntry::Live(sessao) => {
            let meta = settle(sessao);
            meta.provider.is_none().then(|| json!({"id":meta.id,"name":meta.name,"project_id":meta.project_id,"cwd":meta.cwd,
                "state":meta.state,"created_at_ms":meta.created_at_ms,"updated_at_ms":meta.last_activity_at_ms}))
        }
        SessionEntry::Historical(_) => None,
    }).collect()).unwrap_or_default();
    Json(json!(lista))
}

pub(super) async fn saida_do_terminal(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Query(desde): Query<Desde>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || ler_saida(&shell_vivo(&state,&id)?, desde.desde))
        .await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao ler o terminal"))?
}

pub(super) async fn teclas_do_terminal(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Json(body): Json<TeclasBody>) -> Result<Json<Value>, ApiError> {
    if body.dados.is_empty() || body.dados.len() > TECLAS_MAX {
        return Err(error(StatusCode::BAD_REQUEST,"Entrada vazia ou grande demais"));
    }
    tokio::task::spawn_blocking(move || {
        shell_vivo(&state,&id)?;
        write_terminal(&state,&id,&body.dados).map_err(|erro|error(StatusCode::CONFLICT,&erro.to_string()))?;
        Ok(Json(json!({"ok":true})))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao enviar teclas"))?
}

pub(super) async fn fechar_terminal(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        shell_vivo(&state,&id)?;
        close_session(&state,&id).map_err(|erro|error(StatusCode::CONFLICT,&erro.to_string()))?;
        Ok(Json(json!({"ok":true})))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao fechar o terminal"))?
}

#[derive(Deserialize)] #[serde(deny_unknown_fields)] pub(super) struct AbrirTerminalBody { project_id: String }
pub(super) async fn abrir_terminal(State(state): State<Arc<EngineState>>, Json(body): Json<AbrirTerminalBody>) -> Result<(StatusCode,Json<Value>), ApiError> {
    tokio::task::spawn_blocking(move || {
        let projeto = known_projects(&state).into_iter().find(|projeto| projeto.id == body.project_id && !projeto.path.is_empty())
            .ok_or_else(||error(StatusCode::NOT_FOUND,"Projeto não encontrado neste PC"))?;
        if !Path::new(&projeto.path).is_dir() { return Err(error(StatusCode::CONFLICT,"A pasta do projeto não existe mais neste PC")); }
        limite_de_sessoes::conferir(&state)?; // Teto único e configurável.
        // Headless: ninguém exibe esta PTY num terminal de verdade até o desktop abrir a aba.
        let resposta = spawn_terminal(&state, projeto.id, "Terminal".into(), projeto.path, None, None, 32, 120,
            SessionOrigin { headless: true, ..SessionOrigin::default() })
            .map_err(|erro|error(StatusCode::INTERNAL_SERVER_ERROR,&erro.to_string()))?;
        let EngineResponse::Session { session } = resposta else { return Err(error(StatusCode::INTERNAL_SERVER_ERROR,"Resposta inesperada")); };
        Ok((StatusCode::CREATED,Json(json!({"id":session.id,"name":session.name,"project_id":session.project_id}))))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao abrir o terminal"))?
}
