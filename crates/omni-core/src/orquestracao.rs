//! Fork: regra pura do orquestrador de agentes (sem I/O de rede).
//!
//! Uma conversa vira **orquestradora** pelo botão "Tornar orquestradora": a CLI dela é reaberta com
//! um servidor MCP do engine (`crates/omni-engine/src/orquestracao.rs`) e ganha ferramentas para
//! criar e comandar subagentes de qualquer plataforma que o app suporta. Aqui ficam o registro
//! orquestrador → subagentes (`orquestracao.json` na pasta do engine), o limite de subagentes, o
//! estado que o fluxo visual mostra e as flags que ligam o MCP na linha de comando de cada CLI.
//!
//! O registro guarda só ids, plataforma, conta, modelo, a tarefa e a data: nada de token nem de
//! conteúdo de conversa. O token do MCP vive só em memória, no engine.

use serde::{Deserialize, Serialize};
use std::path::Path;

pub const ARQUIVO: &str = "orquestracao.json";
/// Subagentes vivos por orquestrador quando o usuário não escolheu outro número.
pub const LIMITE_PADRAO: usize = 3;
/// Teto do limite configurável: cada subagente gasta assento e limite de uso da conta.
pub const LIMITE_MAXIMO: usize = 8;
/// Nome do servidor MCP nas CLIs (`mcp__medza__criar_agente` no Claude).
pub const NOME_DO_SERVIDOR: &str = "medza";
/// Variável de ambiente da PTY com o token do orquestrador. Nunca vai na linha de comando.
pub const VARIAVEL_DO_TOKEN: &str = "MEDZA_ORQ_TOKEN";
/// Porta do MCP = porta do engine + este deslocamento (a do celular é +1). Assim cada variante
/// (instalado, dev, build local) tem a sua, derivada da identidade.
pub const DESLOCAMENTO_DA_PORTA: u16 = 3;
/// `esperar` nunca segura a chamada mais que isto (o Codex corta ferramenta em 60 s por padrão; o
/// orquestrador Codex sobe com `tool_timeout_sec` maior, ver `flags_do_orquestrador`).
pub const ESPERA_MAXIMA_S: u64 = 120;
pub const ULTIMAS_PADRAO: usize = 5;
pub const ULTIMAS_MAXIMO: usize = 30;

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Registro {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub limite_de_agentes: Option<usize>,
    #[serde(default)]
    pub orquestradores: Vec<Orquestrador>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Orquestrador {
    /// Chave da conversa orquestradora (`conversation_id` do índice).
    pub orquestrador: String,
    /// Com as ferramentas ligadas. "Deixar de orquestrar" desliga e mantém os subagentes no fluxo.
    #[serde(default = "verdadeiro")]
    pub ativo: bool,
    #[serde(default)]
    pub agentes: Vec<Agente>,
}

fn verdadeiro() -> bool { true }

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Agente {
    #[serde(rename = "conversationKey")]
    pub conversation_key: String,
    pub plataforma: String,
    #[serde(default)]
    pub conta: Option<String>,
    #[serde(default)]
    pub modelo: Option<String>,
    #[serde(default)]
    pub esforco: Option<String>,
    pub tarefa: String,
    pub criado_em: u64,
    /// Fork (harness): caminhos que o subagente pode editar e criar, relativos à pasta do orquestrador.
    /// Dois subagentes vivos do mesmo orquestrador nunca dividem um caminho (`briefing::conflitos`).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub editar: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub criar: Vec<String>,
    /// Fork: worktree isolada onde ele roda (`criar_agente` com `worktree`), e a branch dela. Sem isso,
    /// roda na pasta do orquestrador.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub worktree: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch: Option<String>,
    /// Fork (Copiloto abre conversa na tela): conversa que o usuário pediu para ver; a janela do
    /// copiloto abre numa aba do workspace atual. Sem o campo (registros antigos), `false`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub na_tela: bool,
}

impl Agente {
    /// Tudo o que é dele (editar + criar).
    pub fn arquivos(&self) -> impl Iterator<Item = &String> { self.editar.iter().chain(self.criar.iter()) }
}

/// Fork (harness): briefing do subagente e posse de arquivos entre subagentes.
#[path = "orquestracao_briefing.rs"]
pub mod briefing;

pub fn ler(dir: &Path) -> Registro {
    std::fs::read(dir.join(ARQUIVO)).ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn gravar(dir: &Path, registro: &Registro) -> std::io::Result<()> {
    let caminho = dir.join(ARQUIVO);
    let temporario = caminho.with_extension("json.tmp");
    std::fs::write(&temporario, serde_json::to_vec_pretty(registro)?)?;
    std::fs::rename(temporario, caminho)
}

impl Registro {
    pub fn limite(&self) -> usize {
        self.limite_de_agentes.unwrap_or(LIMITE_PADRAO).clamp(1, LIMITE_MAXIMO)
    }

    pub fn orquestrador(&self, conversa: &str) -> Option<&Orquestrador> {
        self.orquestradores.iter().find(|o| o.orquestrador == conversa)
    }

    pub fn ativo(&self, conversa: &str) -> bool {
        self.orquestrador(conversa).is_some_and(|o| o.ativo)
    }

    /// Liga ou desliga as ferramentas. Ligar cria a entrada; desligar só marca (o fluxo continua).
    pub fn definir_ativo(&mut self, conversa: &str, ativo: bool) {
        match self.orquestradores.iter_mut().find(|o| o.orquestrador == conversa) {
            Some(orquestrador) => orquestrador.ativo = ativo,
            None if ativo => self.orquestradores.push(Orquestrador { orquestrador: conversa.into(), ativo, agentes: Vec::new() }),
            None => {}
        }
    }

    /// O subagente, **só** se ele pertence a este orquestrador: é o que limita o alcance do token.
    pub fn agente_de(&self, orquestrador: &str, agente: &str) -> Option<&Agente> {
        self.orquestrador(orquestrador)?.agentes.iter().find(|a| a.conversation_key == agente)
    }

    pub fn adicionar(&mut self, orquestrador: &str, agente: Agente) {
        self.definir_ativo(orquestrador, true);
        if let Some(entrada) = self.orquestradores.iter_mut().find(|o| o.orquestrador == orquestrador) {
            entrada.agentes.retain(|a| a.conversation_key != agente.conversation_key);
            entrada.agentes.push(agente);
        }
    }

    /// Orquestrador de quem esta conversa é subagente (para o fluxo abrir a partir de um subagente).
    pub fn dono_de(&self, agente: &str) -> Option<&str> {
        self.orquestradores.iter().find(|o| o.agentes.iter().any(|a| a.conversation_key == agente)).map(|o| o.orquestrador.as_str())
    }

    /// Esquece uma conversa apagada: como orquestradora e como subagente.
    pub fn esquecer(&mut self, conversa: &str) -> bool {
        let antes = self.clone();
        self.orquestradores.retain(|o| o.orquestrador != conversa);
        for orquestrador in &mut self.orquestradores { orquestrador.agentes.retain(|a| a.conversation_key != conversa); }
        antes != *self
    }
}

/// Recusa com mensagem clara quando o orquestrador já tem `limite` subagentes vivos.
pub fn pode_criar(vivos: usize, limite: usize) -> Result<(), String> {
    if vivos >= limite {
        return Err(format!(
            "Limite de {limite} subagente(s) vivo(s) por orquestrador atingido. Pare um com parar_agente ou peça ao usuário para aumentar o limite no fluxo."
        ));
    }
    Ok(())
}

/// Estado de um nó do fluxo, na ordem de prioridade (pedido de permissão vence "trabalhando").
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Estado {
    /// Pedido de permissão, menu ou pergunta da CLI na tela.
    EsperandoVoce,
    Trabalhando,
    /// Parado no campo vazio: pronto para a próxima mensagem.
    Pronto,
    /// Sessão caiu, limite de uso ou erro da API.
    Erro,
    /// Sessão encerrada (parar_agente ou fechada pelo usuário).
    Parado,
    /// Aberta, mas a tela ainda não foi reconhecida (subindo, confiar na pasta…).
    Abrindo,
}

impl Estado {
    pub fn texto(self) -> &'static str {
        match self {
            Self::EsperandoVoce => "esperando você",
            Self::Trabalhando => "trabalhando",
            Self::Pronto => "pronto",
            Self::Erro => "erro",
            Self::Parado => "parado",
            Self::Abrindo => "abrindo",
        }
    }
    /// Conta para o limite: o que ainda ocupa uma PTY com a CLI.
    pub fn vivo(self) -> bool { !matches!(self, Self::Parado | Self::Erro) }
}

/// O que o engine sabe de uma sessão, já lido da tela e do meta.
#[derive(Debug, Clone, Default)]
pub struct Sinais<'a> {
    pub viva: bool,
    /// `SessionState` serializado (`working`, `answered`, `approval_required`, `crashed`, `stopped`…).
    pub estado: Option<&'a str>,
    pub aviso: Option<&'a str>,
    pub pode_mandar: bool,
    pub pode_aprovar: bool,
    pub tem_escolha: bool,
    pub cli_rodando: bool,
}

pub fn estado(s: &Sinais) -> Estado {
    if !s.viva { return Estado::Parado; }
    if s.pode_aprovar || s.tem_escolha || s.estado == Some("approval_required") { return Estado::EsperandoVoce; }
    if matches!(s.aviso, Some("usage_limit" | "api_error")) || s.estado == Some("crashed") { return Estado::Erro; }
    if matches!(s.estado, Some("stopped" | "orphan")) { return Estado::Parado; }
    if s.pode_mandar { return Estado::Pronto; }
    if s.estado == Some("working") && s.cli_rodando { return Estado::Trabalhando; }
    if !s.cli_rodando { return Estado::Abrindo; }
    if s.estado == Some("answered") { return Estado::Pronto; }
    Estado::Trabalhando
}

/// Só letras, números e `._-`: o valor vira argumento de um comando digitado num shell (a mesma regra
/// do `argumento_seguro` dos fluxos).
pub fn argumento_seguro(valor: &str) -> bool {
    !valor.is_empty() && valor.len() <= 64 && valor.chars().all(|c| c.is_ascii_alphanumeric() || "._-".contains(c))
}

/// Modelo e esforço na linha de comando da CLI do subagente. Plataforma sem regra conhecida recebe
/// `--model` (o padrão das CLIs) e o esforço é recusado com motivo.
pub fn argumentos_de_modelo(plataforma: &str, modelo: Option<&str>, esforco: Option<&str>) -> Result<String, String> {
    let mut saida = String::new();
    if let Some(modelo) = modelo.filter(|m| !m.is_empty()) {
        if !argumento_seguro(modelo) { return Err("Modelo com caracteres não aceitos: use só letras, números, ponto, hífen e sublinhado".into()); }
        saida += &match plataforma { "codex" => format!(" -m {modelo}"), _ => format!(" --model {modelo}") };
    }
    if let Some(esforco) = esforco.filter(|e| !e.is_empty()) {
        if !argumento_seguro(esforco) { return Err("Esforço com caracteres não aceitos".into()); }
        saida += &match plataforma {
            "claude" | "antigravity" => format!(" --effort {esforco}"),
            "codex" => format!(" -c model_reasoning_effort={esforco}"),
            outra => return Err(format!("A plataforma {outra} não aceita esforço pela linha de comando; crie sem esforco")),
        };
    }
    Ok(saida)
}

pub fn porta_do_mcp(porta_do_engine: u16) -> u16 { porta_do_engine.wrapping_add(DESLOCAMENTO_DA_PORTA) }

pub fn url_do_mcp(porta: u16) -> String { format!("http://127.0.0.1:{porta}/mcp") }

/// Plataformas que sabem registrar um MCP **só para a sessão** (conferido nas CLIs instaladas em
/// 2026-10-19). Antigravity só tem `agy mcp add` (config global): fica como subagente.
pub fn pode_orquestrar(plataforma: &str) -> bool { matches!(plataforma, "claude" | "codex") }

/// Flags que ligam o MCP do orquestrador na linha de comando. O token **não** aparece: o Claude
/// expande `${MEDZA_ORQ_TOKEN}` do ambiente; o Codex lê a variável pelo nome.
/// - Claude: `--mcp-config <json>` (code.claude.com/docs/en/mcp), entre aspas simples para o shell
///   não expandir o `${…}` antes da CLI.
/// - Codex: `-c mcp_servers.medza.*` (chaves de learn.chatgpt.com/docs/extend/mcp). Sem aspas: o
///   valor que não é TOML vale como texto, e o reinício do Codex (`retomar_codex`) recusa aspas.
pub fn flags_do_orquestrador(plataforma: &str, porta: u16) -> Option<String> {
    let url = url_do_mcp(porta);
    match plataforma {
        "claude" => {
            let config = serde_json::json!({"mcpServers": {NOME_DO_SERVIDOR: {
                "type": "http", "url": url,
                "headers": {"Authorization": format!("Bearer ${{{VARIAVEL_DO_TOKEN}}}")},
            }}});
            Some(format!(" --mcp-config '{config}'"))
        }
        "codex" => Some(format!(
            " -c mcp_servers.{NOME_DO_SERVIDOR}.url={url} -c mcp_servers.{NOME_DO_SERVIDOR}.bearer_token_env_var={VARIAVEL_DO_TOKEN} -c mcp_servers.{NOME_DO_SERVIDOR}.tool_timeout_sec={}",
            ESPERA_MAXIMA_S + 30
        )),
        _ => None,
    }
}

/// O comando sem as flags do orquestrador (para "Deixar de orquestrar" e para não duplicá-las).
pub fn sem_flags(comando: &str) -> String {
    let mut saida = comando.to_owned();
    // Claude: ` --mcp-config '{…"medza"…}'` (o JSON não tem aspas simples dentro).
    while let Some(inicio) = saida.find(" --mcp-config '") {
        let corpo = inicio + " --mcp-config '".len();
        let Some(fim) = saida[corpo..].find('\'') else { break };
        if !saida[corpo..corpo + fim].contains(&format!("\"{NOME_DO_SERVIDOR}\"")) { break; }
        saida.replace_range(inicio..corpo + fim + 1, "");
    }
    // Codex: cada ` -c mcp_servers.medza.<chave>=<valor>`.
    let marca = format!(" -c mcp_servers.{NOME_DO_SERVIDOR}.");
    while let Some(inicio) = saida.find(&marca) {
        let resto = &saida[inicio + 1..];
        let fim = resto.find(" -").map(|i| inicio + 1 + i).unwrap_or(saida.len());
        saida.replace_range(inicio..fim, "");
    }
    saida
}

/// O comando com as flags certas: com elas se `ativo`, sem elas se não. Idempotente.
pub fn ajustar_comando(comando: &str, plataforma: &str, porta: u16, ativo: bool) -> String {
    let limpo = sem_flags(comando);
    match (ativo, flags_do_orquestrador(plataforma, porta)) {
        (true, Some(flags)) => format!("{limpo}{flags}"),
        _ => limpo,
    }
}

/// Texto que pode virar prompt numa CLI: as mesmas regras da rota `/conversas/{id}/prompt`.
pub fn texto_valido(texto: &str) -> Result<(), String> {
    if texto.trim().is_empty() || texto.len() > 16_000 || texto.chars().any(|c| c.is_control() && c != '\n' && c != '\t')
        || texto.trim_start().starts_with('/') {
        return Err("Mande texto de até 16000 bytes, sem comandos slash nem caracteres de controle".into());
    }
    Ok(())
}

pub fn segundos_de_espera(pedido: Option<u64>) -> u64 { pedido.unwrap_or(60).clamp(1, ESPERA_MAXIMA_S) }

pub fn ultimas(pedido: Option<usize>) -> usize { pedido.unwrap_or(ULTIMAS_PADRAO).clamp(1, ULTIMAS_MAXIMO) }

/// Corta em `max` caracteres (não bytes), com reticências.
pub fn resumir(texto: &str, max: usize) -> String {
    let texto = texto.trim();
    if texto.chars().count() <= max { return texto.to_owned(); }
    let mut saida: String = texto.chars().take(max.saturating_sub(1)).collect();
    saida.push('…');
    saida
}

#[cfg(test)]
mod tests {
    use super::*;

    fn agente(id: &str) -> Agente {
        Agente { conversation_key: id.into(), plataforma: "claude".into(), modelo: Some("haiku".into()),
            tarefa: "diga ok".into(), criado_em: 1, ..Agente::default() }
    }

    #[test]
    fn registro_grava_le_e_isola_por_orquestrador() {
        let dir = tempfile::tempdir().unwrap();
        let mut registro = ler(dir.path());
        assert_eq!(registro, Registro::default());
        registro.adicionar("orq-a", agente("sub-1"));
        registro.adicionar("orq-b", agente("sub-2"));
        gravar(dir.path(), &registro).unwrap();
        let lido = ler(dir.path());
        assert_eq!(lido, registro);
        assert!(lido.agente_de("orq-a", "sub-1").is_some());
        assert!(lido.agente_de("orq-a", "sub-2").is_none(), "subagente de outro orquestrador");
        assert!(lido.agente_de("sub-1", "sub-1").is_none());
        assert_eq!(lido.dono_de("sub-2"), Some("orq-b"));
        // Formato pedido: chave da conversa como `conversationKey`.
        let bruto = std::fs::read_to_string(dir.path().join(ARQUIVO)).unwrap();
        assert!(bruto.contains("\"conversationKey\": \"sub-1\"") && bruto.contains("\"orquestrador\": \"orq-a\""));
        assert!(!bruto.to_lowercase().contains("token"));
    }

    #[test]
    fn ativar_desativar_e_esquecer() {
        let mut registro = Registro::default();
        registro.definir_ativo("x", false);
        assert!(registro.orquestradores.is_empty(), "desligar o que não existe não cria entrada");
        registro.definir_ativo("x", true);
        registro.adicionar("x", agente("s"));
        assert!(registro.ativo("x"));
        registro.definir_ativo("x", false);
        assert!(!registro.ativo("x"));
        assert!(registro.agente_de("x", "s").is_some(), "desligar mantém o fluxo");
        assert!(registro.esquecer("s"));
        assert!(registro.agente_de("x", "s").is_none());
        assert!(registro.esquecer("x"));
        assert!(!registro.esquecer("x"));
    }

    #[test]
    fn limite_padrao_configuravel_e_com_teto() {
        let mut registro = Registro::default();
        assert_eq!(registro.limite(), 3);
        registro.limite_de_agentes = Some(50);
        assert_eq!(registro.limite(), LIMITE_MAXIMO);
        registro.limite_de_agentes = Some(0);
        assert_eq!(registro.limite(), 1);
        assert!(pode_criar(2, 3).is_ok());
        let erro = pode_criar(3, 3).unwrap_err();
        assert!(erro.contains("Limite de 3"), "{erro}");
    }

    #[test]
    fn estado_do_no_em_tabela() {
        let base = Sinais { viva: true, cli_rodando: true, ..Sinais::default() };
        let casos: Vec<(Sinais, Estado)> = vec![
            (Sinais::default(), Estado::Parado),
            (Sinais { estado: Some("working"), ..base.clone() }, Estado::Trabalhando),
            (Sinais { estado: Some("working"), pode_aprovar: true, ..base.clone() }, Estado::EsperandoVoce),
            (Sinais { estado: Some("answered"), tem_escolha: true, ..base.clone() }, Estado::EsperandoVoce),
            (Sinais { estado: Some("answered"), pode_mandar: true, ..base.clone() }, Estado::Pronto),
            (Sinais { estado: Some("answered"), aviso: Some("usage_limit"), ..base.clone() }, Estado::Erro),
            (Sinais { estado: Some("crashed"), ..base.clone() }, Estado::Erro),
            (Sinais { estado: Some("stopped"), ..base.clone() }, Estado::Parado),
            (Sinais { estado: Some("working"), cli_rodando: false, ..base.clone() }, Estado::Abrindo),
        ];
        for (sinais, esperado) in casos { assert_eq!(estado(&sinais), esperado, "{sinais:?}"); }
        assert!(Estado::Abrindo.vivo() && Estado::Pronto.vivo() && !Estado::Parado.vivo() && !Estado::Erro.vivo());
    }

    #[test]
    fn modelo_e_esforco_por_plataforma_e_qualquer_modelo_seguro() {
        assert_eq!(argumentos_de_modelo("claude", Some("haiku"), Some("low")).unwrap(), " --model haiku --effort low");
        assert_eq!(argumentos_de_modelo("codex", Some("gpt-9-mini"), Some("low")).unwrap(), " -m gpt-9-mini -c model_reasoning_effort=low");
        assert_eq!(argumentos_de_modelo("antigravity", Some("gemini-x"), None).unwrap(), " --model gemini-x");
        // Plataforma nova: o modelo passa mesmo fora de qualquer lista.
        assert_eq!(argumentos_de_modelo("cursor", Some("modelo-novo-2027"), None).unwrap(), " --model modelo-novo-2027");
        assert!(argumentos_de_modelo("cursor", None, Some("high")).is_err());
        assert!(argumentos_de_modelo("claude", Some("haiku; rm -rf ~"), None).is_err());
        assert!(argumentos_de_modelo("claude", Some("$(x)"), None).is_err());
        assert_eq!(argumentos_de_modelo("claude", None, None).unwrap(), "");
    }

    #[test]
    fn flags_ligam_e_desligam_sem_token_na_linha() {
        let claude = ajustar_comando("claude --resume abc --model haiku", "claude", 47424, true);
        assert!(claude.starts_with("claude --resume abc --model haiku --mcp-config '{"), "{claude}");
        assert!(claude.contains("http://127.0.0.1:47424/mcp") && claude.contains("Bearer ${MEDZA_ORQ_TOKEN}"));
        assert_eq!(ajustar_comando(&claude, "claude", 47424, true), claude, "idempotente");
        assert_eq!(ajustar_comando(&claude, "claude", 47424, false), "claude --resume abc --model haiku");
        let json = claude.split('\'').nth(1).unwrap();
        assert_eq!(serde_json::from_str::<serde_json::Value>(json).unwrap()["mcpServers"]["medza"]["type"], "http");

        let codex = ajustar_comando("codex resume 019a -m gpt-x", "codex", 47424, true);
        assert!(!codex.contains(['"', '\'']), "o reinício do Codex recusa aspas: {codex}");
        assert!(codex.contains("-c mcp_servers.medza.bearer_token_env_var=MEDZA_ORQ_TOKEN"));
        assert!(codex.contains("tool_timeout_sec=150"));
        assert_eq!(ajustar_comando(&codex, "codex", 47424, false), "codex resume 019a -m gpt-x");
        assert_eq!(ajustar_comando(&codex, "codex", 1, true).matches("mcp_servers.medza.url").count(), 1);

        // O MCP de outro servidor que o usuário passou à mão fica.
        let outro = "claude --mcp-config '{\"mcpServers\":{\"meu\":{}}}'";
        assert_eq!(sem_flags(outro), outro);
        assert_eq!(ajustar_comando("agy", "antigravity", 1, true), "agy", "antigravity não orquestra");
        assert!(!pode_orquestrar("antigravity") && pode_orquestrar("codex"));
    }

    #[test]
    fn limites_de_texto_espera_e_resumo() {
        assert!(texto_valido("diga só ok").is_ok());
        assert!(texto_valido("/clear").is_err());
        assert!(texto_valido("   ").is_err());
        assert!(texto_valido("a\u{1b}b").is_err());
        assert_eq!(segundos_de_espera(Some(999)), 120);
        assert_eq!(segundos_de_espera(Some(0)), 1);
        assert_eq!(segundos_de_espera(None), 60);
        assert_eq!(ultimas(Some(100)), 30);
        assert_eq!(resumir("ação longa", 4), "açã…");
        assert_eq!(porta_do_mcp(47421), 47424);
    }
}
