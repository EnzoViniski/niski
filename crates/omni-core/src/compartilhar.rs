//! Extensões da conta principal valendo em todas as contas da mesma IA.
//!
//! Cada conta do OMNI é um config dir próprio, então uma conta extra (a "EmailUber") nascia sem
//! nenhum MCP, plugin ou skill da principal. Antes de abrir uma sessão numa conta extra — sessão
//! nova, reinício ou troca de conta do failover —, o engine leva da conta principal (a marcada com
//! estrela na tela; sem marca, a nativa, `builtin`):
//!
//! - **Claude**: `mcpServers` do `.claude.json` (globais e por pasta), `enabledPlugins` e
//!   `extraKnownMarketplaces` do `settings.json`, o registro de plugins instalados (que aponta para
//!   o cache da principal — nada é baixado de novo) e as pastas `skills`, `agents` e `commands`.
//! - **Codex**: `[mcp_servers]`, `[plugins]` e `[marketplaces]` do `config.toml`.
//! - **Cursor**: só tem a conta nativa.
//!
//! **Mescla, não substitui**: o que a conta extra já tinha fica; o que tem o mesmo nome segue a
//! principal. Arquivo de skill que a conta já tinha (ou editou depois de receber) não é
//! sobrescrito; o que veio da principal sem mexer acompanha as edições dela.
//!
//! Troca de estrela e liga/desliga só gravam nas contas sem sessão aberta — a CLI regrava os
//! arquivos dela o tempo todo. As abertas recebem antes da próxima sessão.
//!
//! **Conectores do claude.ai não entram**: são autorizados na conta do claude.ai (na nuvem), não em
//! arquivo — cada conta conecta os seus em claude.ai → Configurações → Conectores.

use serde_json::{Map, Value};
use std::fs;
use std::path::Path;

const PREFERENCIA: &str = "extensoes.json";

fn preferencias(dir: &Path) -> Map<String, Value> {
    fs::read(dir.join(PREFERENCIA)).ok()
        .and_then(|bytes| serde_json::from_slice::<Value>(&bytes).ok())
        .and_then(|valor| valor.as_object().cloned())
        .unwrap_or_default()
}

fn gravar_preferencias(dir: &Path, mapa: Map<String, Value>) -> std::io::Result<()> {
    fs::write(dir.join(PREFERENCIA), serde_json::to_vec_pretty(&Value::Object(mapa))?)
}

/// Ligado por padrão: quem tem conta extra quer as mesmas ferramentas nela.
pub fn ativado(dir: &Path) -> bool {
    preferencias(dir).get("compartilhar").and_then(Value::as_bool).unwrap_or(true)
}

/// Grava só a chave; a conta principal escolhida continua.
pub fn definir(dir: &Path, ligado: bool) -> std::io::Result<()> {
    let mut mapa = preferencias(dir);
    mapa.insert("compartilhar".into(), Value::Bool(ligado));
    gravar_preferencias(dir, mapa)
}

/// A conta de onde as outras da mesma IA recebem as extensões (e os conectores): a marcada com
/// estrela, se ainda existir; senão a nativa.
pub fn principal(dir: &Path, provider: &str) -> Option<crate::Profile> {
    let perfis = crate::profiles(dir);
    let marcada = preferencias(dir).get("principal").and_then(|p| p.get(provider)).and_then(Value::as_str).map(str::to_owned);
    marcada.and_then(|id| perfis.iter().find(|p| p.id == id && p.provider == provider).cloned())
        .or_else(|| perfis.into_iter().find(|p| p.provider == provider && p.builtin))
}

/// Id da principal de cada IA que tem mais de uma conta possível (Claude e Codex).
pub fn principais(dir: &Path) -> Map<String, Value> {
    ["claude", "codex"].into_iter()
        .filter_map(|provider| principal(dir, provider).map(|p| (provider.to_owned(), Value::String(p.id))))
        .collect()
}

/// Marca a principal da IA e, com o compartilhamento ligado, já leva as extensões dela às outras.
pub fn definir_principal(dir: &Path, provider: &str, profile_id: &str) -> std::io::Result<Vec<(String, Levado)>> {
    if !crate::profiles(dir).iter().any(|p| p.id == profile_id && p.provider == provider) {
        return Err(std::io::Error::new(std::io::ErrorKind::NotFound, "conta não encontrada"));
    }
    let mut mapa = preferencias(dir);
    let principal = mapa.entry("principal").or_insert_with(|| Value::Object(Map::new()));
    if !principal.is_object() { *principal = Value::Object(Map::new()); }
    principal.as_object_mut().expect("objeto").insert(provider.into(), Value::String(profile_id.into()));
    gravar_preferencias(dir, mapa)?;
    if !ativado(dir) { return Ok(Vec::new()); }
    Ok(levar_a_todas(dir).into_iter().filter(|(id, _)| crate::profiles(dir).iter().any(|p| &p.id == id && p.provider == provider)).collect())
}

/// Contas com sessão aberta agora, pelo `sessions.json` do engine. A CLI regrava o `.claude.json`
/// o tempo todo; gravar nele por fora, com ela aberta, pode fazer um apagar a mudança do outro.
fn com_sessao_aberta(dir: &Path) -> Vec<String> {
    let Some(Value::Array(sessoes)) = ler(&dir.join("sessions.json")) else { return Vec::new() };
    sessoes.iter()
        .filter(|s| matches!(s.get("state").and_then(Value::as_str), Some("working" | "answered" | "approval_required")))
        .filter_map(|s| s.get("profile_id").and_then(Value::as_str).map(str::to_owned))
        .collect()
}

/// Leva para todas as contas que não são a principal — menos as que estão abertas, que recebem
/// antes da próxima sessão (`antes_de_abrir`).
fn levar_a_todas(dir: &Path) -> Vec<(String, Levado)> {
    let abertas = com_sessao_aberta(dir);
    crate::profiles(dir).into_iter()
        .filter(|p| !abertas.contains(&p.id))
        .filter(|p| principal(dir, &p.provider).is_some_and(|principal| principal.id != p.id))
        .map(|p| { let levado = levar(dir, &p.provider, &p.id); (p.id, levado) }).collect()
}

/// Liga/desliga e, ligando, já leva tudo para as contas extras (sem esperar a próxima sessão).
pub fn definir_e_levar(dir: &Path, ligado: bool) -> std::io::Result<Vec<(String, Levado)>> {
    definir(dir, ligado)?;
    if !ligado { return Ok(Vec::new()); }
    Ok(levar_a_todas(dir))
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
    let Some(principal) = principal(dir, provider) else { return Levado::default() };
    if principal.id == alvo.id { return Levado::default(); }
    match provider {
        "claude" => claude(&principal, alvo),
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
    if let Ok(bytes) = serde_json::to_vec_pretty(depois) { gravar_atomico(caminho, &bytes); }
}

/// Temporário + rename: quem lê no meio nunca vê o arquivo pela metade.
fn gravar_atomico(caminho: &Path, bytes: &[u8]) {
    if let Some(pasta) = caminho.parent() { let _ = fs::create_dir_all(pasta); }
    let temporario = caminho.with_extension("omni-tmp");
    if fs::write(&temporario, bytes).is_ok() { let _ = fs::rename(&temporario, caminho); }
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

    // Registro de instalados e de lojas: apontam para o cache da principal, nada é baixado de novo.
    let mut sumidos = Vec::new();
    for arquivo in ["installed_plugins.json", "known_marketplaces.json"] {
        let Some(origem) = ler(&de.join("plugins").join(arquivo)) else { continue };
        let caminho = para.join("plugins").join(arquivo);
        let antes = ler(&caminho);
        let mut destino = antes.clone().filter(Value::is_object).unwrap_or_else(|| Value::Object(Map::new()));
        let mapa = destino.as_object_mut().expect("objeto");
        if arquivo == "installed_plugins.json" {
            mapa.entry("version").or_insert_with(|| origem.get("version").cloned().unwrap_or(Value::from(2)));
            mesclar_objeto(mapa, "plugins", Some(&origem));
            // Plugin desinstalado na principal some do cache dela: o registro aqui apontaria para
            // uma pasta que não existe mais e a CLI daria erro ao carregar.
            if let Some(plugins) = mapa.get_mut("plugins").and_then(Value::as_object_mut) {
                plugins.retain(|nome, instalacoes| {
                    let existe = instalacoes.as_array().is_none_or(|lista| lista.iter().any(|i|
                        i.get("installPath").and_then(Value::as_str).is_none_or(|p| Path::new(p).exists())));
                    if !existe { sumidos.push(nome.clone()); }
                    existe
                });
            }
        } else if let Some(lojas) = origem.as_object() {
            for (nome, valor) in lojas { mapa.insert(nome.clone(), valor.clone()); }
        }
        gravar_se_mudou(&caminho, &antes, &destino);
    }

    // Plugins: quais estão ligados e de quais lojas, no settings.json.
    let origem = ler(&de.join("settings.json"));
    let caminho = para.join("settings.json");
    let antes = ler(&caminho);
    let mut settings = antes.clone().filter(Value::is_object).unwrap_or_else(|| Value::Object(Map::new()));
    let mapa = settings.as_object_mut().expect("objeto");
    levado.plugins += mesclar_objeto(mapa, "enabledPlugins", origem.as_ref());
    mesclar_objeto(mapa, "extraKnownMarketplaces", origem.as_ref());
    if let Some(ligados) = mapa.get_mut("enabledPlugins").and_then(Value::as_object_mut) {
        for nome in &sumidos { ligados.remove(nome); }
    }
    gravar_se_mudou(&caminho, &antes, &settings);

    // Registro do que veio da principal, para acompanhar as edições dela sem pisar nas da conta.
    let registro = para.join("omni-copiadas.json");
    let mut copiadas = ler(&registro).and_then(|v| v.as_object().cloned()).unwrap_or_default();
    let antes = copiadas.clone();
    for pasta in ["skills", "agents", "commands"] {
        levado.skills += copiar_skills(&de.join(pasta), &para.join(pasta), pasta, &mut copiadas);
    }
    if copiadas != antes { gravar_se_mudou(&registro, &None, &Value::Object(copiadas)); }
    levado
}

fn modificado_ms(caminho: &Path) -> Option<u64> {
    let quando = fs::metadata(caminho).ok()?.modified().ok()?;
    Some(quando.duration_since(std::time::UNIX_EPOCH).ok()?.as_millis() as u64)
}

/// Copia a árvore sem pisar no que a conta tem de seu: arquivo que não existe é copiado; arquivo
/// que veio da principal e não foi mexido aqui (a data bate com a anotada em `copiadas`) segue a
/// principal quando ela muda. Devolve quantas entradas de primeiro nível passaram a existir.
fn copiar_skills(de: &Path, para: &Path, chave: &str, copiadas: &mut Map<String, Value>) -> usize {
    let Ok(entradas) = fs::read_dir(de) else { return 0 };
    let mut novas = 0;
    for entrada in entradas.flatten() {
        let destino = para.join(entrada.file_name());
        let chave = format!("{chave}/{}", entrada.file_name().to_string_lossy());
        let ja_existia = destino.exists();
        if entrada.path().is_dir() {
            let _ = fs::create_dir_all(&destino);
            copiar_skills(&entrada.path(), &destino, &chave, copiadas);
        } else {
            let nosso = ja_existia && copiadas.get(&chave).and_then(Value::as_u64) == modificado_ms(&destino);
            let desatualizado = nosso && fs::read(entrada.path()).ok() != fs::read(&destino).ok();
            if !ja_existia || desatualizado {
                let _ = fs::create_dir_all(para);
                if fs::copy(entrada.path(), &destino).is_ok() {
                    if let Some(ms) = modificado_ms(&destino) { copiadas.insert(chave, Value::from(ms)); }
                }
            }
        }
        if !ja_existia { novas += 1; }
    }
    novas
}

fn codex(de: &Path, para: &Path) -> Levado {
    use toml_edit::{DocumentMut, Item};
    let mut levado = Levado::default();
    let Some(origem) = fs::read_to_string(de.join("config.toml")).ok().and_then(|t| t.parse::<DocumentMut>().ok()) else { return levado };
    let caminho = para.join("config.toml");
    let antes = fs::read_to_string(&caminho).unwrap_or_default();
    // toml_edit mantém comentários e formatação do arquivo da conta. Arquivo inválido fica como está.
    let Ok(mut destino) = antes.parse::<DocumentMut>() else { return levado };
    for chave in ["mcp_servers", "plugins", "marketplaces"] {
        let Some(tabela) = origem.get(chave).and_then(Item::as_table_like) else { continue };
        if destino.get(chave).and_then(Item::as_table_like).is_none() {
            let mut nova = toml_edit::Table::new();
            nova.set_implicit(true);
            destino.insert(chave, Item::Table(nova));
        }
        let Some(alvo) = destino.get_mut(chave).and_then(Item::as_table_like_mut) else { continue };
        for (nome, valor) in tabela.iter() { alvo.insert(nome, valor.clone()); }
        match chave { "mcp_servers" => levado.mcps += tabela.len(), "plugins" => levado.plugins += tabela.len(), _ => {} }
    }
    let depois = destino.to_string();
    if depois != antes { gravar_atomico(&caminho, depois.as_bytes()); }
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
    fn estrela_troca_a_conta_de_onde_as_outras_recebem() {
        let raiz = tempfile::tempdir().unwrap();
        let (nativa, extra) = (raiz.path().join("n"), raiz.path().join("e"));
        fs::create_dir_all(&nativa).unwrap();
        fs::create_dir_all(&extra).unwrap();
        perfis(raiz.path(), &nativa, &extra, "codex");
        fs::write(nativa.join("config.toml"), "[mcp_servers.da_nativa]\ncommand = \"a\"\n").unwrap();
        fs::write(extra.join("config.toml"), "[mcp_servers.da_extra]\ncommand = \"b\"\n").unwrap();
        assert_eq!(principal(raiz.path(), "codex").unwrap().id, "principal", "sem estrela vale a nativa");

        definir(raiz.path(), true).unwrap();
        let levados = definir_principal(raiz.path(), "codex", "extra").unwrap();
        assert_eq!(levados, vec![("principal".to_owned(), Levado { mcps: 1, plugins: 0, skills: 0 })]);
        let nativa_agora: toml::Table = fs::read_to_string(nativa.join("config.toml")).unwrap().parse().unwrap();
        assert!(nativa_agora["mcp_servers"]["da_extra"].is_table(), "a nativa passou a receber da estrela");
        assert!(ativado(raiz.path()), "marcar a estrela não mexe na chave");
        assert_eq!(principais(raiz.path())["codex"], "extra");
        assert_eq!(levar(raiz.path(), "codex", "extra"), Levado::default(), "a principal não recebe de si mesma");
        assert!(definir_principal(raiz.path(), "codex", "nao-existe").is_err());
    }

    #[test]
    fn skill_copiada_acompanha_a_principal_e_a_editada_fica() {
        let raiz = tempfile::tempdir().unwrap();
        let (principal, extra) = (raiz.path().join("p"), raiz.path().join("e"));
        fs::create_dir_all(&extra).unwrap();
        perfis(raiz.path(), &principal, &extra, "claude");
        let skill = |conta: &Path, nome: &str| conta.join("skills").join(nome).join("SKILL.md");
        for nome in ["a", "b"] {
            fs::create_dir_all(principal.join("skills").join(nome)).unwrap();
            fs::write(skill(&principal, nome), "v1").unwrap();
        }
        levar(raiz.path(), "claude", "extra");
        std::thread::sleep(std::time::Duration::from_millis(20));
        fs::write(skill(&extra, "b"), "minha").unwrap();
        fs::write(skill(&principal, "a"), "v2").unwrap();
        fs::write(skill(&principal, "b"), "v2").unwrap();
        levar(raiz.path(), "claude", "extra");
        assert_eq!(fs::read_to_string(skill(&extra, "a")).unwrap(), "v2", "copiada sem mexer segue a principal");
        assert_eq!(fs::read_to_string(skill(&extra, "b")).unwrap(), "minha", "editada na conta fica");
    }

    #[test]
    fn plugin_desinstalado_na_principal_sai_da_outra() {
        let raiz = tempfile::tempdir().unwrap();
        let (principal, extra) = (raiz.path().join("p"), raiz.path().join("e"));
        fs::create_dir_all(principal.join("plugins")).unwrap();
        fs::create_dir_all(extra.join("plugins")).unwrap();
        perfis(raiz.path(), &principal, &extra, "claude");
        let sumido = raiz.path().join("cache-sumido").to_string_lossy().into_owned();
        let existe = principal.to_string_lossy().into_owned();
        fs::write(principal.join("plugins").join("installed_plugins.json"), json!({"version":2,"plugins":{"fica@x":[{"installPath":existe}]}}).to_string()).unwrap();
        fs::write(extra.join("plugins").join("installed_plugins.json"), json!({"version":2,"plugins":{"velho@x":[{"installPath":sumido}]}}).to_string()).unwrap();
        fs::write(extra.join("settings.json"), json!({"enabledPlugins":{"velho@x":true}}).to_string()).unwrap();
        levar(raiz.path(), "claude", "extra");
        let registro = ler(&extra.join("plugins").join("installed_plugins.json")).unwrap();
        assert!(registro["plugins"].get("velho@x").is_none() && registro["plugins"]["fica@x"].is_array());
        assert!(ler(&extra.join("settings.json")).unwrap()["enabledPlugins"].get("velho@x").is_none());
    }

    #[test]
    fn conta_com_sessao_aberta_espera_a_proxima() {
        let raiz = tempfile::tempdir().unwrap();
        let (principal, extra) = (raiz.path().join("p"), raiz.path().join("e"));
        fs::create_dir_all(&principal).unwrap();
        fs::create_dir_all(&extra).unwrap();
        perfis(raiz.path(), &principal, &extra, "codex");
        fs::write(principal.join("config.toml"), "[mcp_servers.repl]\ncommand = \"node\"\n").unwrap();
        fs::write(raiz.path().join("sessions.json"), json!([{"profile_id":"extra","state":"working"}]).to_string()).unwrap();
        assert!(definir_e_levar(raiz.path(), true).unwrap().is_empty());
        assert!(!extra.join("config.toml").exists());
        fs::write(raiz.path().join("sessions.json"), json!([{"profile_id":"extra","state":"stopped"}]).to_string()).unwrap();
        assert_eq!(definir_e_levar(raiz.path(), true).unwrap().len(), 1);
        assert_eq!(antes_de_abrir(raiz.path(), "codex", "extra").mcps, 1, "abrir a sessão sempre leva");
    }

    #[test]
    fn codex_leva_mcps_e_plugins_do_config_toml() {
        let raiz = tempfile::tempdir().unwrap();
        let (principal, extra) = (raiz.path().join("p"), raiz.path().join("e"));
        fs::create_dir_all(&principal).unwrap();
        fs::create_dir_all(&extra).unwrap();
        perfis(raiz.path(), &principal, &extra, "codex");
        fs::write(principal.join("config.toml"), "model = \"gpt-5\"\n[mcp_servers.repl]\ncommand = \"node\"\n[plugins.\"notion@curated\"]\nenabled = true\n").unwrap();
        fs::write(extra.join("config.toml"), "# minha conta\nmodel = \"o3\" # rápido\n").unwrap();
        assert_eq!(levar(raiz.path(), "codex", "extra"), Levado { mcps: 1, plugins: 1, skills: 0 });
        let texto = fs::read_to_string(extra.join("config.toml")).unwrap();
        let tabela: toml::Table = texto.parse().unwrap();
        assert_eq!(tabela["model"].as_str(), Some("o3"), "preferência da conta extra fica");
        assert!(texto.starts_with("# minha conta\nmodel = \"o3\" # rápido\n"), "comentários ficam: {texto}");
        assert!(texto.contains("\n[mcp_servers.repl]\ncommand = \"node\"\n") && !texto.contains("[mcp_servers]\n"), "{texto}");
        assert!(tabela["mcp_servers"]["repl"].is_table());
        assert_eq!(tabela["plugins"]["notion@curated"]["enabled"].as_bool(), Some(true));
    }
}
