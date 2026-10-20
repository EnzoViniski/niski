//! Fork: revisão oficial numa conversa nova, na pasta exata da origem.
//! Doc conferida em 2026-10-20: code.claude.com/docs/en/code-review-local e
//! learn.chatgpt.com/docs/developer-commands?surface=cli#codex-review.
//! Filho de mobile::orquestracao para compartilhar a trava e a entrega inicial confirmada.
use super::*;
use std::sync::OnceLock;

#[derive(Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pedido {
    provider: String,
    escopo: String,
    base: Option<String>,
}

pub(in crate::mobile) fn rotas() -> Router<Arc<EngineState>> {
    Router::new().route("/conversas/{id}/revisao", post(criar))
}

fn comando(pedido: &Pedido, base: Option<&str>) -> Result<String, &'static str> {
    match (pedido.provider.as_str(), pedido.escopo.as_str()) {
        // Sem alvo, Claude inclui commits da branch. O alvo explícito limita a revisão local.
        ("claude", "local") => Ok("/code-review medium uncommitted changes only: staged, unstaged and untracked files; exclude committed branch changes".into()),
        ("claude", "branch") => Ok(format!("/code-review medium {}...HEAD", base.ok_or("Informe a branch base")?)),
        ("codex", "local") => Ok(" review --uncommitted".into()),
        ("codex", "branch") => Ok(format!(" review --base {}", base.ok_or("Informe a branch base")?)),
        _ => Err("Escolha Claude ou Codex e o escopo local ou branch"),
    }
}

fn base_valida(base: &str) -> bool {
    !base.is_empty()
        && base.len() <= 200
        && !base.starts_with('-')
        && !base.contains("..")
        && base
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "._/-".contains(c))
}
fn resolver_base(pasta: &str, pedido: &Pedido) -> Result<Option<String>, ApiError> {
    if pedido.escopo != "branch" {
        return Ok(None);
    }
    let base = pedido
        .base
        .as_deref()
        .filter(|b| base_valida(b))
        .ok_or_else(|| {
            error(
                StatusCode::BAD_REQUEST,
                "Informe uma referência Git válida como base",
            )
        })?;
    let saida = std::process::Command::new("git")
        .args(["rev-parse", "--verify", &format!("{base}^{{commit}}")])
        .current_dir(pasta)
        .output()
        .map_err(|_| error(StatusCode::CONFLICT, "Git indisponível"))?;
    let sha = String::from_utf8_lossy(&saida.stdout).trim().to_owned();
    if !saida.status.success() || sha.len() < 40 || !sha.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(error(
            StatusCode::BAD_REQUEST,
            "Branch base não encontrada nesta pasta",
        ));
    }
    Ok(Some(sha))
}

fn conferir_limite(
    state: &EngineState,
    registro: &regra::Registro,
    origem: &str,
) -> Result<(), ApiError> {
    let vivos = registro
        .orquestrador(origem)
        .map(|o| {
            o.agentes
                .iter()
                .filter(|a| current(state, &a.conversation_key).is_ok_and(|(_, s)| s.is_some()))
                .count()
        })
        .unwrap_or(0);
    regra::pode_criar(vivos, registro.limite()).map_err(|m| error(StatusCode::CONFLICT, &m))
}

// Chave por engine + origem: o mesmo clique repetido não abre outro agente.
type Cache = HashMap<String, (u64, Pedido, Value)>;
async fn criar(
    State(state): State<Arc<EngineState>>,
    RoutePath(origem): RoutePath<String>,
    headers: HeaderMap,
    Json(pedido): Json<Pedido>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    let chave = headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .filter(|s| !s.is_empty() && s.len() <= 128)
        .ok_or_else(|| error(StatusCode::BAD_REQUEST, "Idempotency-Key obrigatório"))?
        .to_owned();
    tokio::task::spawn_blocking(move || {
        static CACHE: OnceLock<Mutex<Cache>> = OnceLock::new();
        let mut cache = CACHE
            .get_or_init(Default::default)
            .lock()
            .expect("revisoes");
        cache.retain(|_, (ms, _, _)| now_ms().saturating_sub(*ms) < 600_000);
        let chave = format!("{}:{origem}:{chave}", state.state_file.display());
        if let Some((_, anterior, valor)) = cache.get(&chave) {
            if anterior != &pedido {
                return Err(error(
                    StatusCode::CONFLICT,
                    "Chave já usada com outra revisão",
                ));
            }
            return Ok((StatusCode::CREATED, Json(valor.clone())));
        }
        comando(&pedido, Some("base")).map_err(|m| error(StatusCode::BAD_REQUEST, m))?;
        let (mae, _) = current(&state, &origem)?;
        if !Path::new(&mae.cwd).is_dir() {
            return Err(error(StatusCode::CONFLICT, "A pasta da origem não existe"));
        }
        let base = resolver_base(&mae.cwd, &pedido)?;
        let tarefa =
            comando(&pedido, base.as_deref()).map_err(|m| error(StatusCode::BAD_REQUEST, m))?;
        let perfil = mae
            .segments
            .last()
            .filter(|s| s.provider == pedido.provider)
            .and_then(|s| s.profile_id.clone());
        let titulo = format!(
            "Revisão · {}",
            mae.title.chars().take(100).collect::<String>()
        );
        let mut plano = resolve_spawn(
            &state,
            &SpawnBody {
                project_id: mae.project_id.clone(),
                provider: pedido.provider.clone(),
                profile_id: perfil,
                titulo: Some(titulo.clone()),
            },
        )?;
        plano.cwd = mae.cwd.clone();
        if let (Some(sid), Some(perfil)) = (
            &plano.external_session_id,
            omni_core::profiles(dir(&state))
                .iter()
                .find(|p| Some(&p.id) == plano.profile_id.as_ref()),
        ) {
            plano.transcript_path = Some(
                omni_core::conversations::claude_transcript_path(
                    Path::new(&perfil.config_dir),
                    &plano.cwd,
                    sid,
                )
                .to_string_lossy()
                .into(),
            );
        }
        if pedido.provider == "codex" {
            plano.command += &tarefa;
        } else {
            // A conversa já é separada. O modo foreground oficial permite confirmar o turno
            // e evita reenviar /code-review enquanto o subagente de fundo ainda trabalha.
            plano
                .env
                .push(("CLAUDE_CODE_DISABLE_BACKGROUND_TASKS".into(), "1".into()));
        }
        let conversa = format!("conv-{}", uuid::Uuid::new_v4());
        plano.conversation_id = Some(conversa.clone());
        // Não segurar a trava do registro no spawn: preparar() também consulta esse registro.
        conferir_limite(&state, &ler_registro(&state), &origem)?;
        let acao = Action {
            id: state.next_id(),
            conversation_id: String::new(),
            session_id: String::new(),
            state: "executing".into(),
            created_at_ms: now_ms(),
            error: None,
            key: chave.clone(),
            revision: String::new(),
            kind: ActionKind::Spawn(plano.clone()),
        };
        spawn_from_mobile(&state, &acao, &plano).map_err(|_| {
            error(
                StatusCode::CONFLICT,
                "Não consegui abrir a conversa de revisão",
            )
        })?;
        let (_, sessao) = current(&state, &conversa)?;
        let sid = sessao
            .ok_or_else(|| error(StatusCode::CONFLICT, "Sessão de revisão indisponível"))?
            .meta
            .lock()
            .expect("meta")
            .id
            .clone();
        // Relê sob a trava: o MCP pode ter adicionado um filho enquanto a PTY subia.
        let _trava = state.mobile.orquestracao.trava.lock().expect("registro");
        let mut registro = regra::ler(dir(&state));
        if let Err(e) = conferir_limite(&state, &registro, &origem) {
            let _ = close_session(&state, &sid);
            return Err(e);
        }
        let ativo = registro.ativo(&origem);
        registro.adicionar(
            &origem,
            regra::Agente {
                conversation_key: conversa.clone(),
                plataforma: pedido.provider.clone(),
                conta: plano.profile_id.clone(),
                tarefa: format!("Revisão · {}", pedido.escopo),
                criado_em: now_ms(),
                ..Default::default()
            },
        );
        // Criar vínculo não ativa ferramentas MCP nem reinicia a conversa de origem.
        registro.definir_ativo(&origem, ativo);
        if regra::gravar(dir(&state), &registro).is_err() {
            let _ = close_session(&state, &sid);
            return Err(error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Não consegui gravar o vínculo da revisão",
            ));
        }
        drop(_trava);
        if pedido.provider == "claude" {
            state
                .mobile
                .orquestracao
                .pendentes
                .lock()
                .expect("pendentes")
                .insert(conversa.clone(), "esperando a CLI ficar pronta".into());
            let (s, c) = (state.clone(), conversa.clone());
            // Porta interna confiável: a mesma fila e confirmação de início de turno do orquestrador.
            std::thread::spawn(move || {
                ferramentas::mandar_tarefa_quando_pronta(
                    &s,
                    &c,
                    &tarefa,
                    std::time::Duration::from_secs(180),
                )
            });
        }
        let valor = json!({"id":conversa,"session_id":sid,"name":titulo,"origem":origem});
        cache.insert(chave, (now_ms(), pedido, valor.clone()));
        Ok((StatusCode::CREATED, Json(valor)))
    })
    .await
    .map_err(|_| {
        error(
            StatusCode::INTERNAL_SERVER_ERROR,
            "Falha ao preparar revisão",
        )
    })?
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn comandos_oficiais_e_escopos() {
        for cli in ["claude", "codex"] {
            let p = Pedido {
                provider: cli.into(),
                escopo: "local".into(),
                base: None,
            };
            let local = comando(&p, None).unwrap();
            if cli == "claude" {
                assert!(
                    local.starts_with("/code-review medium ")
                        && local.contains("uncommitted changes only")
                );
            } else {
                assert_eq!(local, " review --uncommitted");
            }
            let p = Pedido {
                escopo: "branch".into(),
                ..p
            };
            assert!(comando(&p, None).is_err());
            assert!(comando(&p, Some("abc123")).unwrap().contains("abc123"));
        }
        assert!(comando(
            &Pedido {
                provider: "cursor".into(),
                escopo: "local".into(),
                base: None
            },
            None
        )
        .is_err());
    }
    #[test]
    fn base_nao_vira_comando() {
        for b in ["main", "origin/main", "release/v1.2"] {
            assert!(base_valida(b));
        }
        for b in ["", "--help", "a..b", "a;id", "$(id)", "a\nb", "a b"] {
            assert!(!base_valida(b));
        }
    }
}

#[cfg(test)]
#[path = "revisao_por_agente_testes.rs"]
mod integracao;
