//! Fork (Editor de vídeo): o agente do chat ao lado do editor. Roda a CLI oficial (Claude Code ou
//! Codex) na conta escolhida, com a pasta do projeto como diretório de trabalho, e continua a mesma
//! sessão a cada pedido (`--resume` / `exec resume`). Nada de chave de API própria nem de token.
//!
//! - Claude: `-p --output-format stream-json --verbose --permission-mode acceptEdits` (edita só
//!   dentro da pasta), ferramentas de leitura/edição e o `lint` do HyperFrames na versão fixada;
//!   `--setting-sources project` (lê as skills e o CLAUDE.md do projeto, sem os ganchos do usuário)
//!   e `--strict-mcp-config` (sem servidores MCP). Flags conferidas no `--help` do Claude 2.1.306.
//! - Codex: `exec --json` com sandbox `workspace-write` e rede liberada só para o `npx` do lint
//!   (`-c sandbox_workspace_write.network_access=true`); `exec resume <id>` para continuar.
//!   Conferido no `--help` do Codex 0.163.
//! - Cada linha da saída vira um evento `video-agente` para o chat (texto, ferramenta, arquivo
//!   mexido). Log só com cli/ok/ms.

use std::path::Path;
use std::time::{Duration, Instant};

use serde_json::Value;
use tauri::Emitter;

use super::processo::{self, Execucao, Linha};
use super::projetos::VERSAO_DO_HYPERFRAMES;

const LIMITE: Duration = Duration::from_secs(20 * 60);
pub const EVENTO: &str = "video-agente";

#[derive(Debug, Clone, serde::Deserialize)]
pub struct PedidoAoAgente {
    pub projeto: String,
    pub cli: String,
    pub conta: Option<String>,
    pub modelo: Option<String>,
    /// Sessão da CLI para continuar (vazia = conversa nova).
    pub sessao: Option<String>,
    pub texto: String,
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
#[serde(tag = "tipo", rename_all = "camelCase")]
pub enum Evento {
    Inicio { sessao: Option<String>, modelo: Option<String> },
    Texto { texto: String },
    Ferramenta { nome: String, detalhe: String },
    Arquivo { arquivo: String },
    Erro { texto: String },
}

#[derive(Debug, Clone, serde::Serialize, Default)]
pub struct Resposta {
    pub texto: String,
    pub sessao: Option<String>,
    pub modelo: Option<String>,
    pub arquivos: Vec<String>,
}

pub fn sistema() -> String {
    format!("Você está no editor de vídeo do Niski. O diretório atual é um projeto HyperFrames {VERSAO_DO_HYPERFRAMES}: \
siga o CLAUDE.md/AGENTS.md daqui, leia o BRIEF.md e as skills em .claude/skills/ antes de editar. Edite só arquivos \
desta pasta. Confira com `npx --yes hyperframes@{VERSAO_DO_HYPERFRAMES} lint`. Não rode render, preview, publish nem nada \
na nuvem: o app faz o preview e a exportação. Responda curto, em português, dizendo o que mudou.")
}

fn uuid_valido(id: &str) -> bool {
    id.len() >= 8 && id.len() <= 80 && id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

/// Argumentos (sem o executável) e o texto que vai pela entrada padrão.
pub fn argumentos(cli: &str, modelo: Option<&str>, sessao: Option<&str>, nova_sessao: &str, texto: &str) -> Result<(Vec<String>, String), String> {
    let modelo = modelo.map(str::trim).filter(|m| !m.is_empty());
    if let Some(m) = modelo { if !crate::roadmaps::ia::modelo_seguro(m) { return Err("Modelo inválido".into()); } }
    let sessao = sessao.map(str::trim).filter(|s| !s.is_empty());
    if let Some(s) = sessao { if !uuid_valido(s) { return Err("Sessão inválida".into()); } }
    let lint = format!("Bash(npx --yes hyperframes@{VERSAO_DO_HYPERFRAMES} lint:*)");
    let timeline = format!("Bash(npx --yes hyperframes@{VERSAO_DO_HYPERFRAMES} timeline:*)");
    let mut args: Vec<String> = Vec::new();
    match cli {
        "claude" => {
            args.extend(["-p", "--output-format", "stream-json", "--verbose", "--permission-mode", "acceptEdits",
                "--setting-sources", "project", "--strict-mcp-config"].map(String::from));
            args.push("--allowedTools".into());
            for f in ["Read", "Edit", "MultiEdit", "Write", "Glob", "Grep", "TodoWrite", "Bash(ls:*)", "Bash(ffprobe:*)"] { args.push(f.into()); }
            args.push(lint);
            args.push(timeline);
            args.push("--disallowedTools".into());
            args.push("WebSearch".into());
            args.push("WebFetch".into());
            if let Some(m) = modelo { args.push("--model".into()); args.push(m.into()); }
            match sessao {
                Some(s) => { args.push("--resume".into()); args.push(s.into()); }
                None => { args.push("--session-id".into()); args.push(nova_sessao.into()); }
            }
            args.push("--append-system-prompt".into());
            args.push(sistema());
            Ok((args, texto.into()))
        }
        "codex" => {
            args.push("exec".into());
            if let Some(s) = sessao { args.push("resume".into()); args.push(s.into()); }
            args.extend(["--json", "--skip-git-repo-check", "-c", "sandbox_mode=\"workspace-write\"",
                "-c", "sandbox_workspace_write.network_access=true", "-c", "approval_policy=\"never\""].map(String::from));
            if let Some(m) = modelo { args.push("--model".into()); args.push(m.into()); }
            args.push("-".into());
            // O Codex sem tela não tem prompt de sistema na linha de comando: vai na frente do pedido.
            Ok((args, if sessao.is_some() { texto.into() } else { format!("{}\n\n---\n\n{texto}", sistema()) }))
        }
        outra => Err(format!("A CLI “{outra}” ainda não funciona no editor de vídeo")),
    }
}

fn relativo(caminho: &str, pasta: &Path) -> String {
    let p = Path::new(caminho);
    p.strip_prefix(pasta).map(|r| r.to_string_lossy().into_owned()).unwrap_or_else(|_| {
        // `/private/tmp` × `/tmp` e afins: só o nome depois da pasta do projeto.
        let pasta = pasta.to_string_lossy();
        caminho.split_once(pasta.trim_start_matches("/private")).map(|(_, r)| r.trim_start_matches('/').to_owned()).unwrap_or_else(|| caminho.to_owned())
    })
}

fn resumo_do_comando(comando: &str) -> String { comando.chars().take(120).collect() }

/// Eventos de uma linha da saída. `edicoes`: ferramentas de edição ainda sem resultado (id → arquivo).
pub fn eventos_da_linha(cli: &str, linha: &str, pasta: &Path, edicoes: &mut Vec<(String, String)>, resposta: &mut Resposta) -> Vec<Evento> {
    let Ok(v) = serde_json::from_str::<Value>(linha.trim()) else { return Vec::new() };
    let mut saida = Vec::new();
    if cli == "claude" {
        match v["type"].as_str() {
            Some("system") if v["subtype"] == "init" => {
                resposta.sessao = v["session_id"].as_str().map(str::to_owned).or(resposta.sessao.take());
                resposta.modelo = v["model"].as_str().map(str::to_owned);
                saida.push(Evento::Inicio { sessao: resposta.sessao.clone(), modelo: resposta.modelo.clone() });
            }
            Some("assistant") => for bloco in v["message"]["content"].as_array().into_iter().flatten() {
                match bloco["type"].as_str() {
                    Some("text") => if let Some(t) = bloco["text"].as_str().filter(|t| !t.trim().is_empty()) { saida.push(Evento::Texto { texto: t.to_owned() }); },
                    Some("tool_use") => {
                        let nome = bloco["name"].as_str().unwrap_or("ferramenta").to_owned();
                        let entrada = &bloco["input"];
                        let arquivo = entrada["file_path"].as_str().or(entrada["path"].as_str()).map(|c| relativo(c, pasta));
                        let detalhe = arquivo.clone().or_else(|| entrada["command"].as_str().map(resumo_do_comando)).or_else(|| entrada["pattern"].as_str().map(str::to_owned)).unwrap_or_default();
                        if matches!(nome.as_str(), "Edit" | "MultiEdit" | "Write" | "NotebookEdit") {
                            if let (Some(id), Some(a)) = (bloco["id"].as_str(), arquivo) { edicoes.push((id.to_owned(), a)); }
                        }
                        saida.push(Evento::Ferramenta { nome, detalhe });
                    }
                    _ => {}
                }
            },
            Some("user") => for bloco in v["message"]["content"].as_array().into_iter().flatten() {
                if bloco["type"] != "tool_result" || bloco["is_error"] == true { continue; }
                if let Some(pos) = bloco["tool_use_id"].as_str().and_then(|id| edicoes.iter().position(|(e, _)| e == id)) {
                    let (_, arquivo) = edicoes.remove(pos);
                    if !resposta.arquivos.contains(&arquivo) { resposta.arquivos.push(arquivo.clone()); }
                    saida.push(Evento::Arquivo { arquivo });
                }
            },
            Some("result") => {
                resposta.sessao = v["session_id"].as_str().map(str::to_owned).or(resposta.sessao.take());
                let texto = v["result"].as_str().unwrap_or_default().to_owned();
                if v["is_error"] == true || v["subtype"].as_str().is_some_and(|s| s.starts_with("error")) {
                    saida.push(Evento::Erro { texto: if texto.is_empty() { "O Claude não conseguiu terminar o pedido.".into() } else { texto } });
                } else if !texto.trim().is_empty() {
                    resposta.texto = texto;
                }
            }
            _ => {}
        }
        return saida;
    }
    match v["type"].as_str() {
        Some("thread.started") => {
            resposta.sessao = v["thread_id"].as_str().map(str::to_owned).or(resposta.sessao.take());
            saida.push(Evento::Inicio { sessao: resposta.sessao.clone(), modelo: resposta.modelo.clone() });
        }
        Some("item.started") if v["item"]["type"] == "command_execution" => {
            saida.push(Evento::Ferramenta { nome: "Comando".into(), detalhe: v["item"]["command"].as_str().map(resumo_do_comando).unwrap_or_default() });
        }
        Some("item.completed") => match v["item"]["type"].as_str() {
            Some("agent_message") => if let Some(t) = v["item"]["text"].as_str().filter(|t| !t.trim().is_empty()) {
                resposta.texto = t.to_owned();
                saida.push(Evento::Texto { texto: t.to_owned() });
            },
            Some("file_change") => for mudanca in v["item"]["changes"].as_array().into_iter().flatten() {
                if let Some(c) = mudanca["path"].as_str() {
                    let arquivo = relativo(c, pasta);
                    if !resposta.arquivos.contains(&arquivo) { resposta.arquivos.push(arquivo.clone()); }
                    saida.push(Evento::Arquivo { arquivo });
                }
            },
            _ => {}
        },
        Some("error") => if let Some(m) = v["message"].as_str() { saida.push(Evento::Erro { texto: m.to_owned() }); },
        Some("turn.failed") => saida.push(Evento::Erro { texto: v["error"]["message"].as_str().unwrap_or("O Codex não conseguiu terminar o pedido.").to_owned() }),
        _ => {}
    }
    saida
}

#[derive(Clone, serde::Serialize)]
struct EventoParaATela<'a> { projeto: &'a str, #[serde(flatten)] evento: &'a Evento }

pub fn chave(projeto: &str) -> String { format!("video-agente:{projeto}") }

/// Roda o pedido com `exe` (separado de [`pedir`] para os testes usarem uma CLI falsa).
pub fn conversar(pedido: &PedidoAoAgente, exe: &Path, env: &[(String, String)], pasta: &Path, limite: Duration, mut avisar: impl FnMut(&Evento)) -> Result<Resposta, String> {
    let nova = uuid::Uuid::new_v4().to_string();
    let (args, entrada) = argumentos(&pedido.cli, pedido.modelo.as_deref(), pedido.sessao.as_deref(), &nova, &pedido.texto)?;
    let mut resposta = Resposta { sessao: pedido.sessao.clone().filter(|s| !s.is_empty()).or_else(|| (pedido.cli == "claude").then(|| nova.clone())), modelo: pedido.modelo.clone(), ..Default::default() };
    let mut edicoes = Vec::new();
    let mut erros: Vec<String> = Vec::new();
    let chave = chave(&pedido.projeto);
    let fim = processo::rodar(Execucao { chave: &chave, exe, args: &args, entrada: Some(entrada), env, pasta, limite, prioridade_baixa: false }, |linha| {
        if let Linha::Saida(l) = linha {
            for ev in eventos_da_linha(&pedido.cli, &l, pasta, &mut edicoes, &mut resposta) {
                if let Evento::Erro { texto } = &ev { erros.push(texto.clone()); }
                avisar(&ev);
            }
        }
    })?;
    if resposta.texto.trim().is_empty() {
        if let Some(e) = erros.last() { return Err(e.clone()); }
        if !fim.sucesso { return Err(format!("A CLI saiu com erro: {}", if fim.ultima_linha_de_erro.is_empty() { "sem detalhe" } else { &fim.ultima_linha_de_erro })); }
        resposta.texto = if resposta.arquivos.is_empty() { "Pronto.".into() } else { format!("Pronto. Mexi em {}.", resposta.arquivos.join(", ")) };
    }
    Ok(resposta)
}

pub fn pedir(app: &tauri::AppHandle, pedido: &PedidoAoAgente) -> Result<Resposta, String> {
    let raiz = super::raiz()?;
    let pasta = super::projetos::pasta_do_projeto(&raiz, &pedido.projeto).map_err(super::texto_do_erro)?;
    if !pasta.join(super::projetos::PROJETO).is_file() { return Err("Esse projeto não existe.".into()); }
    if pedido.texto.trim().is_empty() || pedido.texto.len() > 64 * 1024 { return Err("Pedido vazio ou grande demais.".into()); }
    let nome = if pedido.cli == "claude" { "Claude Code" } else { "Codex" };
    let exe = crate::roadmaps::ia::executavel(&pedido.cli).ok_or_else(|| format!("O {nome} não foi encontrado neste PC"))?;
    let perfis = crate::profiles::list_profiles();
    let perfil = perfis.iter().filter(|p| p.provider == pedido.cli).find(|p| pedido.conta.as_deref().is_some_and(|c| c == p.id))
        .or_else(|| perfis.iter().find(|p| p.provider == pedido.cli && p.authenticated))
        .ok_or_else(|| format!("Nenhuma conta do {nome} com login"))?;
    let mut env = omni_core::env_for(perfil);
    env.extend(processo::ambiente_do_hyperframes());
    let inicio = Instant::now();
    let projeto = pedido.projeto.clone();
    let resultado = conversar(pedido, &exe, &env, &pasta, LIMITE, |ev| { let _ = app.emit(EVENTO, EventoParaATela { projeto: &projeto, evento: ev }); });
    omni_core::registro::info("video_agente", &[
        ("cli", &pedido.cli), ("ok", if resultado.is_ok() { "sim" } else { "nao" }), ("ms", &inicio.elapsed().as_millis().to_string()),
    ]);
    resultado
}

#[tauri::command(async)]
pub fn videos_agente(app: tauri::AppHandle, pedido: PedidoAoAgente) -> Result<Resposta, String> { pedir(&app, &pedido) }

#[tauri::command]
pub fn videos_agente_cancelar(projeto: String) { processo::cancelar(&chave(&projeto)); }

#[tauri::command]
pub fn videos_agente_rodando(projeto: String) -> bool { processo::rodando(&chave(&projeto)) }

#[cfg(test)]
#[path = "agente_testes.rs"]
mod testes;
