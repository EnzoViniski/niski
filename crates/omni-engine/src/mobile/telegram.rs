//! Fork: avisos pela Bot API oficial. Só metadados; respostas continuam no acesso remoto do Medza.
//! O token nunca sai do engine. Cadastro restrito ao IPC do desktop; arquivo próprio, modo 0600.
use super::*;
use std::collections::BTreeSet;

mod api;
mod exemplos; // Fork: um exemplo de cada aviso.
mod rotas;
// Fork (Rotina): o bot da rotina (lembretes com texto e botões, conversa com o agente da rotina).
pub(super) mod rotina;
#[cfg(test)]
mod testes;
pub(super) fn rotas() -> Router<Arc<EngineState>> {
    rotas::rotas()
}

const TIPOS: &[&str] = &[
    "precisa_de_voce",
    "terminou",
    "erro",
    "limite",
    "subagente_terminou",
    "agendamento",
];
const ARQUIVO: &str = "telegram.json";

#[derive(Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct Config {
    token: String,
    bot: String,
    chat: Option<i64>,
    ativo: bool,
    tipos: BTreeSet<String>,
    incluir_titulos: bool,
}

struct Pareando {
    codigo: String,
    expira: u64,
    inicio: u64,
    offset: i64,
}
#[derive(Default)]
struct Dados {
    config: Config,
    pareando: Option<Pareando>,
    erro: Option<String>,
}

pub(super) struct Runtime {
    dados: Mutex<Dados>,
    operacao: tokio::sync::Mutex<()>,
    geracao: AtomicU64,
}
impl Runtime {
    pub(super) fn abrir(dir: &Path) -> Self {
        Self {
            dados: Mutex::new(Dados {
                config: omni_protocol::read_json_or_default(&dir.join(ARQUIVO)),
                ..Default::default()
            }),
            operacao: tokio::sync::Mutex::new(()),
            geracao: AtomicU64::new(0),
        }
    }
    fn status(&self) -> Value {
        let d = self.dados.lock().expect("telegram poisoned");
        json!({"conectado":d.config.chat.is_some(), "ativo":d.config.ativo, "bot":d.config.bot,
            "tipos":d.config.tipos, "incluir_titulos":d.config.incluir_titulos, "erro":d.erro,
            "parear_url":d.pareando.as_ref().filter(|p| p.expira > now_ms()).map(|p| format!("https://t.me/{}?start={}", d.config.bot, p.codigo)),
            "expira_em_ms":d.pareando.as_ref().map(|p| p.expira)})
    }
}

fn salvar(dir: &Path, config: &Config) -> std::io::Result<()> {
    use std::io::Write;
    fs::create_dir_all(dir)?;
    let temporario = dir.join(format!(".telegram-{}.tmp", uuid::Uuid::new_v4()));
    let mut opcoes = fs::OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        opcoes.mode(0o600);
    }
    let resultado = (|| {
        let mut arquivo = opcoes.open(&temporario)?;
        arquivo.write_all(&serde_json::to_vec(config)?)?;
        arquivo.sync_all()?;
        fs::rename(&temporario, dir.join(ARQUIVO))
    })();
    if resultado.is_err() {
        let _ = fs::remove_file(&temporario);
    }
    resultado
}

fn url_remota(state: &EngineState, conversa: &str) -> Option<String> {
    let config = state.mobile.config.lock().ok()?;
    if !config.enabled {
        return None;
    }
    let base = if config.serve {
        state.mobile.serve_state.lock().ok()?.0.clone()
    } else {
        state
            .mobile
            .status
            .lock()
            .ok()?
            .0
            .clone()
            .filter(|_| !config.bind.starts_with("127.") && !config.bind.starts_with("[::1]"))
    }?;
    // Ids do engine são ASCII; o fragmento não carrega token nem pareamento do dispositivo.
    let id: String = conversa
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    Some(format!("{}/#/c/{id}", base.trim_end_matches('/')))
}

fn aviso(item: &Value, config: &Config, url: Option<String>) -> Value {
    let resumo = item["resumo"].as_str().unwrap_or("Atividade nova");
    let plataforma = item["plataforma"].as_str().unwrap_or("agente");
    let mut texto = format!(
        "{} · {plataforma}\n{resumo}",
        omni_protocol::NOME_DO_PRODUTO
    );
    if config.incluir_titulos {
        for campo in ["projeto", "titulo"] {
            if let Some(valor) = item[campo].as_str() {
                texto.push('\n');
                texto.extend(valor.chars().take(300));
            }
        }
    }
    texto.push_str(if url.is_some() {
        "\nAbra a conversa para responder (Tailscale conectado)."
    } else {
        "\nAtive o acesso remoto no Niski para abrir a conversa pelo celular."
    });
    let mut corpo =
        json!({"chat_id":config.chat, "text":texto, "link_preview_options":{"is_disabled":true}});
    if let Some(url) = url {
        corpo["reply_markup"] =
            json!({"inline_keyboard":[[{"text":"Abrir conversa no Niski", "url":url}]]});
    }
    corpo
}

/// Só aceita o desafio novo, num chat privado do próprio usuário. Grupos e mensagens antigas não pareiam.
fn destinatario(update: &Value, p: &Pareando) -> Option<i64> {
    let m = &update["message"];
    let id = m["chat"]["id"].as_i64()?;
    (p.expira > now_ms()
        && m["chat"]["type"] == "private"
        && m["from"]["id"].as_i64() == Some(id)
        && m["from"]["is_bot"] == false
        && m["date"].as_u64()?.saturating_mul(1000) >= p.inicio.saturating_sub(5_000)
        && m["text"].as_str() == Some(&format!("/start {}", p.codigo)))
    .then_some(id)
}

async fn parear(state: &EngineState, cliente: &impl api::BotApi) -> Result<(), api::Falha> {
    let (config, offset) = {
        let mut d = state
            .mobile
            .telegram
            .dados
            .lock()
            .expect("telegram poisoned");
        let Some(p) = &d.pareando else { return Ok(()) };
        if p.expira <= now_ms() {
            d.pareando = None;
            d.erro = Some("O link expirou. Conecte o bot novamente.".into());
            return Ok(());
        }
        (d.config.clone(), p.offset)
    };
    let updates = cliente
        .chamar(
            &config.token,
            "getUpdates",
            json!({"offset":offset, "timeout":0, "allowed_updates":["message"]}),
        )
        .await?;
    let mut d = state
        .mobile
        .telegram
        .dados
        .lock()
        .expect("telegram poisoned");
    for update in updates.as_array().into_iter().flatten() {
        let Some(p) = &mut d.pareando else { break };
        if let Some(id) = update["update_id"].as_i64() {
            p.offset = p.offset.max(id.saturating_add(1));
        }
        if let Some(chat) = destinatario(update, p) {
            let mut pronta = d.config.clone();
            pronta.chat = Some(chat);
            pronta.ativo = true;
            salvar(state.state_file.parent().unwrap(), &pronta).map_err(|_| api::Falha::disco())?;
            d.config = pronta;
            d.pareando = None;
            d.erro = None;
        }
    }
    Ok(())
}

pub(super) fn iniciar(state: Arc<EngineState>) {
    tokio::spawn(async move {
        let Ok(cliente) = api::Cliente::novo() else {
            return;
        };
        observar(state, cliente, std::time::Duration::from_secs(3)).await;
    });
}

async fn observar(state: Arc<EngineState>, cliente: impl api::BotApi, ritmo: std::time::Duration) {
    let mut vistos = BTreeSet::new();
    let mut destino = None;
    let mut proxima = 0;
    let mut geracao = 0;
    let mut intervalo = tokio::time::interval(ritmo);
    intervalo.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        intervalo.tick().await;
        let atual = state.mobile.telegram.geracao.load(Ordering::SeqCst);
        if atual != geracao {
            geracao = atual;
            proxima = 0;
        } // Corrigir a configuração não espera o backoff antigo.
        if now_ms() < proxima {
            continue;
        }
        let _operacao = state.mobile.telegram.operacao.lock().await;
        let resultado = parear(&state, &cliente).await;
        if let Err(falha) = resultado {
            proxima = now_ms() + falha.espera_ms;
            state
                .mobile
                .telegram
                .dados
                .lock()
                .expect("telegram poisoned")
                .erro = Some(falha.texto.into());
            continue;
        }
        let config = state
            .mobile
            .telegram
            .dados
            .lock()
            .expect("telegram poisoned")
            .config
            .clone();
        let itens = caixa_de_atividade::para_avisos(&state);
        // Conectar/reativar/reiniciar não dispara uma pilha de eventos antigos.
        let novo_destino = config.ativo.then_some(config.chat).flatten();
        if destino != novo_destino || novo_destino.is_none() {
            vistos = itens
                .iter()
                .filter_map(|i| i["id"].as_str().map(str::to_owned))
                .collect();
            destino = novo_destino;
            continue;
        }
        for item in &itens {
            let Some(id) = item["id"].as_str() else {
                continue;
            };
            if vistos.contains(id) {
                continue;
            }
            if config
                .tipos
                .contains(item["tipo"].as_str().unwrap_or_default())
            {
                let corpo = aviso(
                    item,
                    &config,
                    item["conversa"]
                        .as_str()
                        .and_then(|c| url_remota(&state, c)),
                );
                match cliente.chamar(&config.token, "sendMessage", corpo).await {
                    Ok(_) => {
                        state
                            .mobile
                            .telegram
                            .dados
                            .lock()
                            .expect("telegram poisoned")
                            .erro = None;
                    }
                    Err(falha) => {
                        proxima = now_ms() + falha.espera_ms;
                        state
                            .mobile
                            .telegram
                            .dados
                            .lock()
                            .expect("telegram poisoned")
                            .erro = Some(falha.texto.into());
                        break; // Sem perder o evento; tenta de novo com espera (inclusive 429).
                    }
                }
            }
            vistos.insert(id.into());
        }
        let atuais: BTreeSet<_> = itens.iter().filter_map(|i| i["id"].as_str()).collect();
        vistos.retain(|id| atuais.contains(id.as_str()));
    }
}
