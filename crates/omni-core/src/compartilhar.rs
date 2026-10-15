//! Extensões da conta principal valendo em todas as contas da mesma IA.
//!
//! Cada conta do OMNI é um config dir próprio, então uma conta extra (a "EmailUber") nascia sem
//! nenhum MCP, plugin ou skill da principal. Antes de abrir uma sessão numa conta extra — sessão
//! nova, reinício ou troca de conta do failover —, o engine leva da conta principal (a nativa, a
//! `builtin`):
//!
//! - **Claude**: `mcpServers` do `.claude.json` (globais e por pasta), `enabledPlugins` e
//!   `extraKnownMarketplaces` do `settings.json`, o registro de plugins instalados (que aponta para
//!   o cache da principal — nada é baixado de novo) e as pastas `skills`, `agents` e `commands`.
//! - **Codex**: `[mcp_servers]`, `[plugins]` e `[marketplaces]` do `config.toml`.
//! - **Cursor**: só tem a conta nativa.
//!
//! **Mescla, não substitui**: o que a conta extra já tinha fica; o que tem o mesmo nome segue a
//! principal. Arquivo de skill que já existe na conta extra não é sobrescrito.
//!
//! **Conectores do claude.ai não entram**: são autorizados na conta do claude.ai (na nuvem), não em
//! arquivo — cada conta conecta os seus em claude.ai → Configurações → Conectores.

use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

const PREFERENCIA: &str = "extensoes.json";

/// Ligado por padrão: quem tem conta extra quer as mesmas ferramentas nela.
pub fn ativado(dir: &Path) -> bool {
    fs::read(dir.join(PREFERENCIA)).ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|valor| valor.get("compartilhar").and_then(Value::as_bool))
        .unwrap_or(true)
}

pub fn definir(dir: &Path, ligado: bool) -> std::io::Result<()> {
    fs::write(dir.join(PREFERENCIA), serde_json::to_vec_pretty(&serde_json::json!({ "compartilhar": ligado }))?)
}

/// Liga/desliga e, ligando, já leva tudo para as contas extras (sem esperar a próxima sessão).
pub fn definir_e_levar(dir: &Path, ligado: bool) -> std::io::Result<Vec<(String, Levado)>> {
    definir(dir, ligado)?;
    if !ligado { return Ok(Vec::new()); }
    Ok(crate::profiles(dir).into_iter().filter(|p| !p.builtin)
        .map(|p| { let levado = levar(dir, &p.provider, &p.id); (p.id, levado) }).collect())
}

/// O que foi levado para a conta — para a tela dizer "12 MCPs e 7 plugins".
#[derive(Debug, Default, serde::Serialize, PartialEq)]
pub struct Levado { pub mcps: usize, pub plugins: usize, pub skills: usize }

/// Leva as extensões da conta principal para `profile_id`, se a preferência estiver ligada e a
/// conta não for a principal. Falha num arquivo não impede o resto (nem a sessão de abrir).
pub fn antes_de_abrir(dir: &Path, provider: &str, profile_id: &str) -> Levado {
    if !ativado(dir) { return Levado::default(); }
    levar(dir, provider, profile_id)
}

pub fn levar(dir: &Path, provider: &str, profile_id: &str) -> Levado {
    let perfis = crate::profiles(dir);
    let Some(alvo) = perfis.iter().find(|p| p.id == profile_id && p.provider == provider) else { return Levado::default() };
    if alvo.builtin { return Levado::default(); }
    let Some(principal) = perfis.iter().find(|p| p.provider == provider && p.builtin) else { return Levado::default() };
    match provider {
        "claude" => claude(principal, alvo),
        "codex" => codex(Path::new(&principal.config_dir), Path::new(&alvo.config_dir)),
        _ => Levado::default(),
    }
}

fn ler(caminho: &Path) -> Option<Value> {
    serde_json::from_slice(&fs::read(caminho).ok()?).ok()
}

/// Grava só se mudou: o `.claude.json` é reescrito pela própria CLI o tempo todo, e regravar igual
/// à toa só aumenta a chance de pisar numa escrita dela.
fn gravar_se_mudou(caminho: &Path, antes: &Option<Value>, depois: &Value) {
    if antes.as_ref() == Some(depois) { return; }
    if let Some(pasta) = caminho.parent() { let _ = fs::create_dir_all(pasta); }
    if let Ok(bytes) = serde_json::to_vec_pretty(depois) {
        let temporario = caminho.with_extension("omni-tmp");
        if fs::write(&temporario, bytes).is_ok() { let _ = fs::rename(&temporario, caminho); }
    }
}

/// `destino[chave]` recebe cada entrada de `origem[chave]` (objeto), sobrescrevendo mesmo nome.
fn mesclar_objeto(destino: &mut Map<String, Value>, chave: &str, origem: Option<&Value>) -> usize {
    let Some(entradas) = origem.and_then(|o| o.get(chave)).and_then(Value::as_object) else { return 0 };
    if entradas.is_empty() { return 0; }
    let alvo = destino.entry(chave).or_insert_with(|| Value::Object(Map::new()));
    if !alvo.is_object() { *alvo = Value::Object(Map::new()); }
    let mapa = alvo.as_object_mut().expect("objeto");
    for (nome, valor) in entradas { mapa.insert(nome.clone(), valor.clone()); }
    entradas.len()
}

fn claude(principal: &crate::Profile, alvo: &crate::Profile) -> Levado {
    let mut levado = Levado::default();
    let (de, para) = (Path::new(&principal.config_dir), Path::new(&alvo.config_dir));

    // MCPs: globais e os de cada pasta.
    let origem = ler(&crate::usage::claude_state_path(principal));
    let caminho = crate::usage::claude_state_path(alvo);
    let antes = ler(&caminho);
    let mut estado = antes.clone().filter(Value::is_object).unwrap_or_else(|| Value::Object(Map::new()));
    let mapa = estado.as_object_mut().expect("objeto");
    levado.mcps += mesclar_objeto(mapa, "mcpServers", origem.as_ref());
    if let Some(pastas) = origem.as_ref().and_then(|o| o.get("projects")).and_then(Value::as_object) {
        let projetos = mapa.entry("projects").or_insert_with(|| Value::Object(Map::new()));
        if let Some(projetos) = projetos.as_object_mut() {
            for (pasta, config) in pastas {
                if config.get("mcpServers").and_then(Value::as_object).is_none_or(Map::is_empty) { continue; }
                let entrada = projetos.entry(pasta.clone()).or_insert_with(|| Value::Object(Map::new()));
                if let Some(entrada) = entrada.as_object_mut() { levado.mcps += mesclar_objeto(entrada, "mcpServers", Some(config)); }
            }
        }
    }
    gravar_se_mudou(&caminho, &antes, &estado);

    // Plugins: quais estão ligados e de quais lojas, no settings.json.
    let origem = ler(&de.join("settings.json"));
    let caminho = para.join("settings.json");
    let antes = ler(&caminho);
    let mut settings = antes.clone().filter(Value::is_object).unwrap_or_else(|| Value::Object(Map::new()));
    let mapa = settings.as_object_mut().expect("objeto");
    levado.plugins += mesclar_objeto(mapa, "enabledPlugins", origem.as_ref());
    mesclar_objeto(mapa, "extraKnownMarketplaces", origem.as_ref());
    gravar_se_mudou(&caminho, &antes, &settings);

    // Registro de instalados e de lojas: apontam para o cache da principal, nada é baixado de novo.
    for arquivo in ["installed_plugins.json", "known_marketplaces.json"] {
        let Some(origem) = ler(&de.join("plugins").join(arquivo)) else { continue };
        let caminho = para.join("plugins").join(arquivo);
        let antes = ler(&caminho);
        let mut destino = antes.clone().filter(Value::is_object).unwrap_or_else(|| Value::Object(Map::new()));
        let mapa = destino.as_object_mut().expect("objeto");
        if arquivo == "installed_plugins.json" {
            mapa.entry("version").or_insert_with(|| origem.get("version").cloned().unwrap_or(Value::from(2)));
            mesclar_objeto(mapa, "plugins", Some(&origem));
        } else if let Some(lojas) = origem.as_object() {
            for (nome, valor) in lojas { mapa.insert(nome.clone(), valor.clone()); }
        }
        gravar_se_mudou(&caminho, &antes, &destino);
    }

    for pasta in ["skills", "agents", "commands"] {
        levado.skills += copiar_sem_sobrescrever(&de.join(pasta), &para.join(pasta));
    }
    levado
}

/// Copia a árvore sem sobrescrever o que já existe. Devolve quantas entradas de primeiro nível
/// passaram a existir (uma skill = uma pasta).
fn copiar_sem_sobrescrever(de: &Path, para: &Path) -> usize {
    let Ok(entradas) = fs::read_dir(de) else { return 0 };
    let mut novas = 0;
    for entrada in entradas.flatten() {
        let destino = para.join(entrada.file_name());
        let ja_existia = destino.exists();
        if entrada.path().is_dir() {
            let _ = fs::create_dir_all(&destino);
            copiar_sem_sobrescrever(&entrada.path(), &destino);
        } else if !ja_existia {
            let _ = fs::create_dir_all(para);
            let _ = fs::copy(entrada.path(), &destino);
        }
        if !ja_existia { novas += 1; }
    }
    novas
}

fn codex(de: &Path, para: &Path) -> Levado {
    let mut levado = Levado::default();
    let ler_toml = |caminho: &Path| fs::read_to_string(caminho).ok().and_then(|t| t.parse::<toml::Table>().ok());
    let Some(origem) = ler_toml(&de.join("config.toml")) else { return levado };
    let caminho = para.join("config.toml");
    let antes = ler_toml(&caminho);
    let mut destino = antes.clone().unwrap_or_default();
    for chave in ["mcp_servers", "plugins", "marketplaces"] {
        let Some(tabela) = origem.get(chave).and_then(toml::Value::as_table) else { continue };
        let alvo = destino.entry(chave).or_insert_with(|| toml::Value::Table(toml::Table::new()));
        if let Some(alvo) = alvo.as_table_mut() {
            for (nome, valor) in tabela { alvo.insert(nome.clone(), valor.clone()); }
        }
        match chave { "mcp_servers" => levado.mcps += tabela.len(), "plugins" => levado.plugins += tabela.len(), _ => {} }
    }
    if antes.as_ref() != Some(&destino) {
        if let Ok(texto) = toml::to_string(&destino) {
            let _ = fs::create_dir_all(para);
            let _ = fs::write(&caminho, texto);
        }
    }
    levado
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn perfis(dir: &Path, principal: &Path, extra: &Path, provider: &str) {
        fs::write(dir.join("profiles.json"), json!({"profiles":[
            {"id":"principal","provider":provider,"name":"Padrão","config_dir":principal.to_string_lossy(),"builtin":true,"created_at_ms":1},
            {"id":"extra","provider":provider,"name":"Trabalho","config_dir":extra.to_string_lossy(),"builtin":false,"created_at_ms":1}
        ]}).to_string()).unwrap();
    }

    #[test]
    fn claude_leva_mcps_plugins_e_skills_sem_apagar_o_que_a_conta_tinha() {
        let raiz = tempfile::tempdir().unwrap();
        let home = raiz.path().join("home");
        let principal = home.join(".claude");
        let extra = raiz.path().join("extra");
        fs::create_dir_all(principal.join("plugins")).unwrap();
        fs::create_dir_all(principal.join("skills").join("revisar")).unwrap();
        fs::create_dir_all(&extra).unwrap();
        perfis(raiz.path(), &principal, &extra, "claude");
        // Conta principal nativa: o `.claude.json` fica ao lado do config dir.
        fs::write(home.join(".claude.json"), json!({"mcpServers":{"stitch":{"type":"http","url":"https://x"}},
            "projects":{"D:/proj":{"mcpServers":{"n8n":{"type":"http","url":"http://l"}}}}}).to_string()).unwrap();
        fs::write(principal.join("settings.json"), json!({"enabledPlugins":{"vercel@oficial":true},"theme":"dark"}).to_string()).unwrap();
        fs::write(principal.join("plugins").join("installed_plugins.json"), json!({"version":2,"plugins":{"vercel@oficial":[{"scope":"user"}]}}).to_string()).unwrap();
        fs::write(principal.join("skills").join("revisar").join("SKILL.md"), "principal").unwrap();
        // O que a conta extra já tinha.
        fs::write(extra.join(".claude.json"), json!({"mcpServers":{"proprio":{"type":"http","url":"https://p"}},"oauthAccount":{"x":1}}).to_string()).unwrap();
        fs::create_dir_all(extra.join("skills").join("revisar")).unwrap();
        fs::write(extra.join("skills").join("revisar").join("SKILL.md"), "minha versão").unwrap();

        let levado = antes_de_abrir(raiz.path(), "claude", "extra");
        assert_eq!(levado, Levado { mcps: 2, plugins: 1, skills: 0 });
        let estado = ler(&extra.join(".claude.json")).unwrap();
        assert!(estado["mcpServers"]["stitch"].is_object() && estado["mcpServers"]["proprio"].is_object());
        assert!(estado["projects"]["D:/proj"]["mcpServers"]["n8n"].is_object());
        assert_eq!(estado["oauthAccount"]["x"], 1, "login da conta extra não pode sumir");
        let settings = ler(&extra.join("settings.json")).unwrap();
        assert_eq!(settings["enabledPlugins"]["vercel@oficial"], true);
        assert!(settings.get("theme").is_none(), "só extensões vão, não preferências");
        assert!(ler(&extra.join("plugins").join("installed_plugins.json")).unwrap()["plugins"]["vercel@oficial"].is_array());
        assert_eq!(fs::read_to_string(extra.join("skills").join("revisar").join("SKILL.md")).unwrap(), "minha versão");

        // Desligado: nada muda; conta principal: nada a fazer.
        definir(raiz.path(), false).unwrap();
        fs::remove_file(extra.join("settings.json")).unwrap();
        assert_eq!(antes_de_abrir(raiz.path(), "claude", "extra"), Levado::default());
        assert!(!extra.join("settings.json").exists());
        definir(raiz.path(), true).unwrap();
        assert_eq!(antes_de_abrir(raiz.path(), "claude", "principal"), Levado::default());
    }

    #[test]
    fn codex_leva_mcps_e_plugins_do_config_toml() {
        let raiz = tempfile::tempdir().unwrap();
        let (principal, extra) = (raiz.path().join("p"), raiz.path().join("e"));
        fs::create_dir_all(&principal).unwrap();
        fs::create_dir_all(&extra).unwrap();
        perfis(raiz.path(), &principal, &extra, "codex");
        fs::write(principal.join("config.toml"), "model = \"gpt-5\"\n[mcp_servers.repl]\ncommand = \"node\"\n[plugins.\"notion@curated\"]\nenabled = true\n").unwrap();
        fs::write(extra.join("config.toml"), "model = \"o3\"\n").unwrap();
        assert_eq!(levar(raiz.path(), "codex", "extra"), Levado { mcps: 1, plugins: 1, skills: 0 });
        let texto = fs::read_to_string(extra.join("config.toml")).unwrap();
        let tabela: toml::Table = texto.parse().unwrap();
        assert_eq!(tabela["model"].as_str(), Some("o3"), "preferência da conta extra fica");
        assert!(tabela["mcp_servers"]["repl"].is_table());
        assert_eq!(tabela["plugins"]["notion@curated"]["enabled"].as_bool(), Some(true));
    }
}
