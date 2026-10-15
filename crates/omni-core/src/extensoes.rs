//! MCPs, plugins e conectores de cada IA, por conta — a aba Extensões das Configurações.
//!
//! Só leitura, direto dos arquivos que cada CLI grava (o OMNI não guarda cópia):
//!
//! | IA     | MCPs                                                        | Plugins                                   | Conectores                        |
//! |--------|-------------------------------------------------------------|-------------------------------------------|-----------------------------------|
//! | Claude | `.claude.json` (global e por pasta) + `.mcp.json` do projeto | `settings.json` + `plugins/installed_plugins.json` | `claudeAiMcpEverConnected` |
//! | Codex  | `config.toml` → `[mcp_servers.*]`                           | `config.toml` → `[plugins.*]`             | —                                 |
//! | Cursor | `mcp.json` + `.cursor/mcp.json` do projeto                  | `plugins/local/*`                         | —                                 |
//!
//! **Nada de segredo sai daqui.** Variáveis de ambiente e cabeçalhos vão só pelo nome; URL perde a
//! query e o usuário/senha; argumento de comando com cara de chave vira `•••`. É isso que deixa a
//! tela segura para abrir com alguém olhando — e para um print.

use serde::Serialize;
use serde_json::Value;
use std::path::{Path, PathBuf};

#[derive(Debug, Serialize, PartialEq)]
pub struct Mcp {
    nome: String,
    /// `global` (vale em todo projeto), `pasta` (só numa pasta, só para você — o "local" do Claude)
    /// ou `projeto` (arquivo dentro do projeto, vai junto no git).
    escopo: &'static str,
    /// `stdio`, `http` ou `sse`.
    tipo: String,
    /// Comando (com argumentos sensíveis mascarados) ou URL sem query.
    destino: String,
    /// Nomes das variáveis de ambiente e dos cabeçalhos — nunca os valores.
    variaveis: Vec<String>,
    ativo: bool,
    /// Pasta à qual o MCP se aplica, no escopo `pasta`.
    pasta: Option<String>,
    arquivo: String,
}

#[derive(Debug, Serialize, PartialEq)]
pub struct Plugin {
    nome: String,
    origem: Option<String>,
    ativo: bool,
    versao: Option<String>,
    escopo: Option<String>,
    arquivo: String,
}

#[derive(Debug, Serialize)]
pub struct ExtensoesDaConta {
    provider: String,
    conta_id: String,
    conta_nome: String,
    /// A conta nativa da CLI — é dela que as extras recebem as extensões.
    principal: bool,
    mcps: Vec<Mcp>,
    plugins: Vec<Plugin>,
    /// Conectores do claude.ai que já conectaram nesta conta (Gmail, Drive, Notion…).
    conectores: Vec<String>,
    /// Arquivos onde as extensões desta conta moram (para o "onde fica").
    arquivos: Vec<String>,
}

/// Extensões de todas as contas cadastradas em `dir` (a pasta do engine, onde mora o `profiles.json`).
pub fn listar(dir: &Path, projeto: Option<&Path>) -> Vec<ExtensoesDaConta> {
    let projeto = projeto.map(Path::to_path_buf).filter(|p| p.is_dir());
    let principais = crate::compartilhar::principais(dir);
    crate::profiles(dir)
        .into_iter()
        .filter_map(|perfil| {
            let dir = PathBuf::from(&perfil.config_dir);
            let mut conta = ExtensoesDaConta {
                provider: perfil.provider.clone(),
                conta_id: perfil.id.clone(),
                conta_nome: perfil.name.clone(),
                principal: principais.get(&perfil.provider).and_then(serde_json::Value::as_str) == Some(perfil.id.as_str()),
                mcps: Vec::new(),
                plugins: Vec::new(),
                conectores: Vec::new(),
                arquivos: Vec::new(),
            };
            match perfil.provider.as_str() {
                "claude" => {
                    let estado = crate::usage::claude_state_path(&perfil);
                    claude(&mut conta, &estado, &dir, projeto.as_deref());
                }
                "codex" => codex(&mut conta, &dir.join("config.toml")),
                "cursor" => cursor(&mut conta, &dir, projeto.as_deref()),
                _ => return None,
            }
            Some(conta)
        })
        .collect()
}

fn ler_json(caminho: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(caminho).ok()?).ok()
}

fn texto(caminho: &Path) -> String {
    caminho.to_string_lossy().into_owned()
}

/* ── Claude ───────────────────────────────────────────────────────────────────────────────── */

fn claude(conta: &mut ExtensoesDaConta, estado: &Path, dir: &Path, projeto: Option<&Path>) {
    conta.arquivos.push(texto(estado));
    if let Some(dados) = ler_json(estado) {
        mcps_do_json(&mut conta.mcps, dados.get("mcpServers"), "global", None, estado);
        if let Some(pastas) = dados.get("projects").and_then(Value::as_object) {
            for (pasta, config) in pastas {
                mcps_do_json(&mut conta.mcps, config.get("mcpServers"), "pasta", Some(pasta.clone()), estado);
            }
        }
        conta.conectores = dados.get("claudeAiMcpEverConnected").and_then(Value::as_array).into_iter().flatten()
            .filter_map(Value::as_str)
            .map(|nome| nome.trim_start_matches("claude.ai ").to_owned())
            .collect();
    }
    if let Some(projeto) = projeto {
        let arquivo = projeto.join(".mcp.json");
        if let Some(dados) = ler_json(&arquivo) {
            conta.arquivos.push(texto(&arquivo));
            mcps_do_json(&mut conta.mcps, dados.get("mcpServers"), "projeto", None, &arquivo);
        }
    }

    // Instalados (com versão e escopo) cruzados com os ligados no settings.json.
    let settings = dir.join("settings.json");
    let instalados = dir.join("plugins").join("installed_plugins.json");
    conta.arquivos.push(texto(&settings));
    let mut ligados = ler_json(&settings).and_then(|s| s.get("enabledPlugins").cloned()).unwrap_or(Value::Null);
    // Plugin de escopo projeto é ligado no `.claude/settings*.json` do próprio projeto — e esse vence.
    if let Some(projeto) = projeto {
        for nome in ["settings.json", "settings.local.json"] {
            let arquivo = projeto.join(".claude").join(nome);
            if let Some(Value::Object(doProjeto)) = ler_json(&arquivo).and_then(|s| s.get("enabledPlugins").cloned()) {
                conta.arquivos.push(texto(&arquivo));
                let base = ligados.as_object().cloned().unwrap_or_default();
                ligados = Value::Object(base.into_iter().chain(doProjeto).collect());
            }
        }
    }
    let mut vistos = std::collections::BTreeSet::new();
    if let Some(lista) = ler_json(&instalados).and_then(|d| d.get("plugins").cloned()).and_then(|p| p.as_object().cloned()) {
        conta.arquivos.push(texto(&instalados));
        for (id, instalacoes) in lista {
            let primeira = instalacoes.as_array().and_then(|l| l.first()).cloned().unwrap_or(Value::Null);
            conta.plugins.push(plugin_claude(&id, ligados.get(&id).and_then(Value::as_bool).unwrap_or(false),
                primeira.get("version").and_then(Value::as_str), primeira.get("scope").and_then(Value::as_str), &instalados));
            vistos.insert(id);
        }
    }
    // Ligado no settings mas sem registro de instalação (instalado por outro caminho).
    if let Some(mapa) = ligados.as_object() {
        for (id, ligado) in mapa {
            if !vistos.contains(id) {
                conta.plugins.push(plugin_claude(id, ligado.as_bool().unwrap_or(false), None, None, &settings));
            }
        }
    }
    conta.plugins.sort_by(|a, b| b.ativo.cmp(&a.ativo).then(a.nome.cmp(&b.nome)));
}

fn plugin_claude(id: &str, ativo: bool, versao: Option<&str>, escopo: Option<&str>, arquivo: &Path) -> Plugin {
    let (nome, origem) = id.split_once('@').map_or((id, None), |(n, o)| (n, Some(o.to_owned())));
    Plugin { nome: nome.to_owned(), origem, ativo, versao: versao.map(str::to_owned), escopo: escopo.map(str::to_owned), arquivo: texto(arquivo) }
}

/* ── Codex ────────────────────────────────────────────────────────────────────────────────── */

fn codex(conta: &mut ExtensoesDaConta, arquivo: &Path) {
    conta.arquivos.push(texto(arquivo));
    let Some(config) = std::fs::read_to_string(arquivo).ok().and_then(|t| t.parse::<toml::Table>().ok()) else { return };
    if let Some(servidores) = config.get("mcp_servers").and_then(toml::Value::as_table) {
        for (nome, srv) in servidores {
            let Some(srv) = srv.as_table() else { continue };
            let url = srv.get("url").and_then(toml::Value::as_str);
            let mut variaveis: Vec<String> = srv.get("env").and_then(toml::Value::as_table).map(|e| e.keys().cloned().collect()).unwrap_or_default();
            variaveis.extend(srv.get("http_headers").and_then(toml::Value::as_table).map(|h| h.keys().cloned().collect::<Vec<_>>()).unwrap_or_default());
            if let Some(token) = srv.get("bearer_token_env_var").and_then(toml::Value::as_str) { variaveis.push(token.to_owned()); }
            let args: Vec<String> = srv.get("args").and_then(toml::Value::as_array).into_iter().flatten()
                .filter_map(|a| a.as_str().map(str::to_owned)).collect();
            conta.mcps.push(Mcp {
                nome: nome.clone(),
                escopo: "global",
                tipo: if url.is_some() { "http".into() } else { "stdio".into() },
                destino: match url {
                    Some(url) => url_segura(url),
                    None => comando_seguro(srv.get("command").and_then(toml::Value::as_str).unwrap_or(""), &args),
                },
                variaveis,
                ativo: srv.get("enabled").and_then(toml::Value::as_bool).unwrap_or(true),
                pasta: None,
                arquivo: texto(arquivo),
            });
        }
    }
    if let Some(plugins) = config.get("plugins").and_then(toml::Value::as_table) {
        for (id, cfg) in plugins {
            let (nome, origem) = id.split_once('@').map_or((id.as_str(), None), |(n, o)| (n, Some(o.to_owned())));
            conta.plugins.push(Plugin {
                nome: nome.to_owned(),
                origem,
                ativo: cfg.get("enabled").and_then(toml::Value::as_bool).unwrap_or(true),
                versao: None,
                escopo: None,
                arquivo: texto(arquivo),
            });
        }
        conta.plugins.sort_by(|a, b| b.ativo.cmp(&a.ativo).then(a.nome.cmp(&b.nome)));
    }
}

/* ── Cursor ───────────────────────────────────────────────────────────────────────────────── */

fn cursor(conta: &mut ExtensoesDaConta, dir: &Path, projeto: Option<&Path>) {
    let global = dir.join("mcp.json");
    conta.arquivos.push(texto(&global));
    if let Some(dados) = ler_json(&global) {
        mcps_do_json(&mut conta.mcps, dados.get("mcpServers"), "global", None, &global);
    }
    if let Some(projeto) = projeto {
        let arquivo = projeto.join(".cursor").join("mcp.json");
        if let Some(dados) = ler_json(&arquivo) {
            conta.arquivos.push(texto(&arquivo));
            mcps_do_json(&mut conta.mcps, dados.get("mcpServers"), "projeto", None, &arquivo);
        }
    }
    let locais = dir.join("plugins").join("local");
    if let Ok(entradas) = std::fs::read_dir(&locais) {
        for entrada in entradas.flatten().filter(|e| e.path().is_dir()) {
            conta.plugins.push(Plugin {
                nome: entrada.file_name().to_string_lossy().into_owned(),
                origem: Some("local".into()),
                ativo: true,
                versao: None,
                escopo: None,
                arquivo: texto(&entrada.path()),
            });
        }
    }
}

/* ── Comum ────────────────────────────────────────────────────────────────────────────────── */

/// `mcpServers` no formato JSON que Claude e Cursor compartilham.
fn mcps_do_json(saida: &mut Vec<Mcp>, servidores: Option<&Value>, escopo: &'static str, pasta: Option<String>, arquivo: &Path) {
    let Some(servidores) = servidores.and_then(Value::as_object) else { return };
    for (nome, srv) in servidores {
        let url = srv.get("url").and_then(Value::as_str);
        let tipo = srv.get("type").and_then(Value::as_str).map(str::to_owned)
            .unwrap_or_else(|| if url.is_some() { "http".into() } else { "stdio".into() });
        let nomes = |campo: &str| srv.get(campo).and_then(Value::as_object).map(|m| m.keys().cloned().collect::<Vec<_>>()).unwrap_or_default();
        let mut variaveis = nomes("env");
        variaveis.extend(nomes("headers"));
        let args: Vec<String> = srv.get("args").and_then(Value::as_array).into_iter().flatten()
            .filter_map(|a| a.as_str().map(str::to_owned)).collect();
        saida.push(Mcp {
            nome: nome.clone(),
            escopo,
            tipo,
            destino: match url {
                Some(url) => url_segura(url),
                None => comando_seguro(srv.get("command").and_then(Value::as_str).unwrap_or(""), &args),
            },
            variaveis,
            ativo: !srv.get("disabled").and_then(Value::as_bool).unwrap_or(false),
            pasta: pasta.clone(),
            arquivo: texto(arquivo),
        });
    }
}

/// URL sem query, fragmento nem `usuario:senha@` — é onde chaves costumam ir parar.
fn url_segura(url: &str) -> String {
    let sem_query = url.split(['?', '#']).next().unwrap_or(url);
    match sem_query.split_once("://") {
        Some((esquema, resto)) => {
            let (host, caminho) = resto.split_once('/').map_or((resto, ""), |(h, c)| (h, c));
            let host = host.rsplit('@').next().unwrap_or(host);
            if caminho.is_empty() { format!("{esquema}://{host}") } else { format!("{esquema}://{host}/{caminho}") }
        }
        None => sem_query.to_owned(),
    }
}

/// Parece segredo: nome de flag sensível, ou um valor longo e opaco (token).
fn parece_segredo(texto: &str) -> bool {
    let minusculo = texto.to_lowercase();
    ["key", "token", "secret", "senha", "password", "passwd", "auth", "bearer", "credential"].iter().any(|p| minusculo.contains(p))
}
fn valor_opaco(texto: &str) -> bool {
    texto.len() >= 24 && texto.chars().all(|c| c.is_ascii_alphanumeric() || "-_=+".contains(c))
        && texto.chars().any(|c| c.is_ascii_digit()) && texto.chars().any(|c| c.is_ascii_alphabetic())
}

/// Comando com os argumentos, mascarando valores sensíveis: `--api-key=x` vira `--api-key=•••`, o
/// valor depois de `--token` vira `•••`, e um token solto (longo e opaco) também.
fn comando_seguro(comando: &str, args: &[String]) -> String {
    let mut partes = vec![comando.to_owned()];
    let mut mascarar_proximo = false;
    for arg in args {
        if mascarar_proximo {
            partes.push("•••".into());
            mascarar_proximo = false;
            continue;
        }
        if let Some((chave, _)) = arg.split_once('=') {
            if parece_segredo(chave) { partes.push(format!("{chave}=•••")); continue; }
        }
        if arg.starts_with('-') && parece_segredo(arg) && !arg.contains('=') {
            partes.push(arg.clone());
            mascarar_proximo = true;
            continue;
        }
        partes.push(if valor_opaco(arg) { "•••".into() } else { arg.clone() });
    }
    partes.join(" ").trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn url_perde_query_e_credencial() {
        assert_eq!(url_segura("https://user:senha@api.x.com/mcp?key=abc#f"), "https://api.x.com/mcp");
        assert_eq!(url_segura("http://localhost:3000"), "http://localhost:3000");
    }

    #[test]
    fn comando_mascara_o_que_parece_segredo() {
        let args = |l: &[&str]| l.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(comando_seguro("npx", &args(&["-y", "@modelcontextprotocol/server-github"])), "npx -y @modelcontextprotocol/server-github");
        assert_eq!(comando_seguro("node", &args(&["srv.js", "--api-key=sk-123", "--token", "abc"])), "node srv.js --api-key=••• --token •••");
        assert_eq!(comando_seguro("x", &args(&["fc-3f9a8b7c6d5e4f3a2b1c0d9e8f7a6b5c"])), "x •••");
        assert_eq!(comando_seguro("node", &args(&["C:\\mcp\\server\\index.js"])), "node C:\\mcp\\server\\index.js");
    }

    #[test]
    fn json_le_mcps_sem_expor_valores() {
        let dir = tempfile::tempdir().unwrap();
        let arquivo = dir.path().join("mcp.json");
        let dados = serde_json::json!({"mcpServers":{
            "remoto":{"type":"http","url":"https://mcp.x.com/v1?token=SEGREDO","headers":{"Authorization":"Bearer SEGREDO"}},
            "local":{"command":"npx","args":["-y","pacote"],"env":{"API_KEY":"SEGREDO"},"disabled":true}
        }});
        let mut saida = Vec::new();
        mcps_do_json(&mut saida, dados.get("mcpServers"), "global", None, &arquivo);
        let tudo = serde_json::to_string(&saida).unwrap();
        assert!(!tudo.contains("SEGREDO"), "valor sensível vazou: {tudo}");
        let remoto = saida.iter().find(|m| m.nome == "remoto").unwrap();
        assert_eq!((remoto.tipo.as_str(), remoto.destino.as_str()), ("http", "https://mcp.x.com/v1"));
        assert_eq!(remoto.variaveis, ["Authorization"]);
        let local = saida.iter().find(|m| m.nome == "local").unwrap();
        assert_eq!((local.tipo.as_str(), local.destino.as_str(), local.ativo), ("stdio", "npx -y pacote", false));
        assert_eq!(local.variaveis, ["API_KEY"]);
    }

    #[test]
    fn codex_le_mcps_e_plugins_do_toml() {
        let dir = tempfile::tempdir().unwrap();
        let arquivo = dir.path().join("config.toml");
        std::fs::write(&arquivo, r#"
model = "gpt-5"
[mcp_servers.repl]
command = "node"
args = ["repl.js"]
[mcp_servers.repl.env]
CHAVE = "SEGREDO"
[mcp_servers.web]
url = "https://x.com/mcp?k=SEGREDO"
enabled = false
[plugins."notion@openai-curated"]
enabled = true
[plugins."canva@openai-curated"]
enabled = false
"#).unwrap();
        let mut conta = ExtensoesDaConta { provider: "codex".into(), conta_id: "c".into(), conta_nome: "C".into(), principal: true,
            mcps: Vec::new(), plugins: Vec::new(), conectores: Vec::new(), arquivos: Vec::new() };
        codex(&mut conta, &arquivo);
        assert!(!serde_json::to_string(&conta).unwrap().contains("SEGREDO"));
        assert_eq!(conta.mcps.len(), 2);
        assert!(conta.mcps.iter().any(|m| m.nome == "web" && !m.ativo && m.destino == "https://x.com/mcp"));
        assert_eq!(conta.plugins.iter().map(|p| (p.nome.as_str(), p.ativo)).collect::<Vec<_>>(), [("notion", true), ("canva", false)]);
    }
}
