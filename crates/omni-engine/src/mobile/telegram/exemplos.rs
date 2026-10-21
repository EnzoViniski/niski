//! Fork (Enzo, 21/10/2026): "Enviar exemplo de cada aviso" — mostra no Telegram como fica cada caso
//! (aprovação, pergunta da CLI, terminou, erro, parado, limite, subagente, agendamento) sem esperar o
//! caso acontecer. Usa o mesmo `aviso()` dos avisos de verdade, então o formato é idêntico; o texto
//! leva "(exemplo)" para ninguém achar que um agente parou. Não depende dos tipos marcados.
use super::*;

/// (tipo, plataforma, resumo): os resumos são os mesmos que `atividade.rs` e `agendamentos.rs` geram.
pub(super) const CASOS: &[(&str, &str, &str)] = &[
    ("precisa_de_voce", "claude", "Precisa de você"),
    ("precisa_de_voce", "codex", "A CLI está perguntando no terminal"),
    ("precisa_de_voce", "claude", "Esperando algo no terminal"),
    ("terminou", "codex", "Terminou o turno"),
    ("erro", "claude", "O agente encontrou um erro"),
    ("erro", "codex", "O agente foi parado"),
    ("limite", "claude", "Limite de uso atingido"),
    ("subagente_terminou", "codex", "Subagente do orquestrador terminou"),
    ("agendamento", "agente", "Agendamento: concluído"),
];

/// Conversa mais recente: o botão do exemplo abre algo que existe, em vez de "conversa não existe".
fn conversa_recente(state: &EngineState) -> Option<String> {
    let dir = state.state_file.parent()?;
    omni_core::conversations::read_index(dir).into_iter().max_by_key(|c| c.created_at_ms).map(|c| c.id)
}

pub(super) async fn enviar(state: &EngineState, cliente: &impl api::BotApi, config: &Config, pausa: std::time::Duration) -> Result<usize, api::Falha> {
    let url = conversa_recente(state).and_then(|c| url_remota(state, &c));
    let abertura = format!("{}: exemplos dos avisos, um por caso. Nada disto aconteceu de verdade.", omni_protocol::NOME_DO_PRODUTO);
    cliente.chamar(&config.token, "sendMessage", json!({"chat_id":config.chat, "text":abertura})).await?;
    for (tipo, plataforma, resumo) in CASOS {
        // Pausa curta entre mensagens: o Telegram limita rajadas no mesmo chat.
        tokio::time::sleep(pausa).await;
        let item = json!({"tipo":tipo, "plataforma":plataforma, "resumo":format!("(exemplo) {resumo}"),
            "projeto":"medza-agents", "titulo":"Conversa de exemplo"});
        cliente.chamar(&config.token, "sendMessage", aviso(&item, config, url.clone())).await?;
    }
    Ok(CASOS.len())
}
