//! Fork (Editor de vídeo): o chat do editor agora é a conversa normal do app (PTY do engine, a mesma
//! tela das abas de agente), rodando com a pasta do projeto como diretório de trabalho. Aqui fica o
//! que o app prepara para essa conversa:
//!
//! - **Pastas que o agente pode ler**: a pasta do computador escolhida para o vídeo (ex.: a pasta da
//!   Medza com identidade visual) e as pastas de referências escolhidas. O Claude recebe por
//!   `permissions.additionalDirectories` no `.claude/settings.json` do projeto (doc oficial de
//!   settings; vale depois de a pasta ser confiável, o que o app já faz ao abrir a conversa); o Codex
//!   lê fora da pasta pelo próprio sandbox. CLAUDE.md e AGENTS.md ganham a lista com o que é cada uma.
//! - **Assinatura dos arquivos** que o preview mostra (data e tamanho): o editor confere enquanto o
//!   agente trabalha e recarrega quando muda. Barato: só `stat`, nada é lido.

use std::collections::hash_map::DefaultHasher;
use std::fs;
use std::hash::{Hash, Hasher};
use std::io;
use std::path::{Path, PathBuf};

use super::arquivos::{self, gravar_atomico, invalido, travar};
use super::projetos::{self, VERSAO_DO_HYPERFRAMES};
use super::referencias;

const INICIO: &str = "<!-- medza:pastas -->";
const FIM: &str = "<!-- /medza:pastas -->";
/// Pastas de referências que um vídeo pode ligar de uma vez (fora isso, é engano).
const MAXIMO_DE_REFERENCIAS: usize = 24;

/// Uma pasta que o agente pode ler, com o que dizer sobre ela no CLAUDE.md.
#[derive(Debug, Clone, PartialEq)]
pub struct PastaLiberada {
    pub caminho: PathBuf,
    pub titulo: String,
    pub descricao: String,
}

/// Pasta do computador escolhida pela pessoa: caminho absoluto de uma pasta que existe. Link
/// simbólico vale (a pasta da pessoa pode morar no iCloud), mas vira o caminho real.
pub fn pasta_do_computador(caminho: &str) -> io::Result<PathBuf> {
    let p = Path::new(caminho.trim());
    if caminho.trim().is_empty() || !p.is_absolute() || caminho.contains('\0') { return Err(invalido("Escolha uma pasta do computador.")); }
    let real = fs::canonicalize(p).map_err(|_| invalido("Essa pasta não existe mais."))?;
    if !real.is_dir() { return Err(invalido("Escolha uma pasta, não um arquivo.")); }
    if real.parent().is_none() { return Err(invalido("Escolha uma pasta específica, não o disco inteiro.")); }
    if std::env::var_os("HOME").is_some_and(|casa| Path::new(&casa) == real) {
        return Err(invalido("Escolha uma pasta específica, não a pasta do usuário inteira."));
    }
    Ok(real)
}

/// Pastas liberadas para o projeto: a do computador (se houver) e as de referências que existem.
pub fn pastas_liberadas(raiz: &Path, computador: Option<&str>, refs: &[String]) -> io::Result<Vec<PastaLiberada>> {
    if refs.len() > MAXIMO_DE_REFERENCIAS { return Err(invalido("Pastas de referências demais.")); }
    let mut lista = Vec::new();
    if let Some(c) = computador.filter(|c| !c.trim().is_empty()) {
        let caminho = pasta_do_computador(c)?;
        let nome = caminho.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        lista.push(PastaLiberada { caminho, titulo: format!("Pasta do computador “{nome}”"),
            descricao: "Material da pessoa para este vídeo (identidade visual, logos, fontes, textos, informações). Leia antes de criar; use o que fizer sentido. Não apague nem mude nada nela sem a pessoa pedir.".into() });
    }
    for id in refs {
        // Pasta de referências apagada depois de escolhida: só sai da lista (o vídeo não quebra).
        let Ok(pasta) = referencias::existente(raiz, id) else { continue };
        let dados = arquivos::ler_json(&pasta.join(referencias::PASTA_JSON)).unwrap_or_default();
        let nome = dados["nome"].as_str().unwrap_or("Referências").to_owned();
        let mut descricao = dados["descricao"].as_str().unwrap_or_default().trim().to_owned();
        let links: Vec<String> = dados["links"].as_array().into_iter().flatten()
            .filter_map(|l| l["url"].as_str().map(|u| format!("{u}{}", l["nota"].as_str().filter(|n| !n.is_empty()).map(|n| format!(" ({n})")).unwrap_or_default())))
            .take(20).collect();
        if !links.is_empty() { descricao = format!("{descricao}{}Links: {}", if descricao.is_empty() { "" } else { " " }, links.join("; ")); }
        lista.push(PastaLiberada { caminho: pasta.join(referencias::ARQUIVOS), titulo: format!("Pasta de referências “{nome}”"), descricao });
    }
    Ok(lista)
}

/// Seção do CLAUDE.md/AGENTS.md com as pastas (entre marcas, para trocar sem mexer no resto).
pub fn secao(pastas: &[PastaLiberada]) -> String {
    let mut texto = format!("{INICIO}\n## Pastas fora do projeto que você pode ler\n\n");
    if pastas.is_empty() {
        texto.push_str("Nenhuma por enquanto. A pessoa escolhe a pasta do computador e as pastas de referências no editor.\n");
    } else {
        texto.push_str("Use estas pastas como fonte (estilo, cores, logos, fontes, informações). Para pôr um arquivo delas no\n\
vídeo, copie para `assets/` deste projeto e use o caminho relativo: o preview só enxerga esta pasta.\n\n");
        for p in pastas {
            texto.push_str(&format!("- **{}**: `{}`", p.titulo, p.caminho.display()));
            if !p.descricao.is_empty() { texto.push_str(&format!(". {}", p.descricao.replace('\n', " "))); }
            texto.push('\n');
        }
    }
    texto.push_str(FIM);
    texto
}

/// Troca a seção das pastas no texto (ou põe no fim).
pub fn com_secao(texto: &str, secao: &str) -> String {
    match (texto.find(INICIO), texto.find(FIM)) {
        (Some(i), Some(f)) if f > i => format!("{}{secao}{}", &texto[..i], &texto[f + FIM.len()..]),
        _ => format!("{}\n\n{secao}\n", texto.trim_end()),
    }
}

/// `.claude/settings.json` do projeto com as pastas liberadas e o `lint`/`timeline` do HyperFrames
/// sem perguntar. Outras chaves que já estejam lá ficam (a pessoa pode ter mexido).
pub fn configuracao_do_claude(atual: Option<serde_json::Value>, pastas: &[PastaLiberada]) -> serde_json::Value {
    let mut v = atual.filter(|v| v.is_object()).unwrap_or_else(|| serde_json::json!({}));
    if !v["permissions"].is_object() { v["permissions"] = serde_json::json!({}); }
    v["permissions"]["additionalDirectories"] = pastas.iter().map(|p| serde_json::Value::from(p.caminho.to_string_lossy().into_owned())).collect();
    let mut permitir: Vec<serde_json::Value> = v["permissions"]["allow"].as_array().cloned().unwrap_or_default();
    for regra in [format!("Bash(npx --yes hyperframes@{VERSAO_DO_HYPERFRAMES} lint *)"), format!("Bash(npx --yes hyperframes@{VERSAO_DO_HYPERFRAMES} timeline *)")] {
        if !permitir.iter().any(|r| r.as_str() == Some(&regra)) { permitir.push(regra.into()); }
    }
    v["permissions"]["allow"] = permitir.into();
    v
}

/// Grava as pastas liberadas no projeto: seção do CLAUDE.md/AGENTS.md e `.claude/settings.json`.
pub fn configurar(raiz: &Path, id: &str, computador: Option<&str>, refs: &[String]) -> io::Result<Vec<PastaLiberada>> {
    let pasta = projetos::pasta_do_projeto(raiz, id)?;
    if arquivos::arquivo_comum(&pasta.join(projetos::PROJETO)).is_none() { return Err(invalido("Esse projeto não existe.")); }
    let pastas = pastas_liberadas(raiz, computador, refs)?;
    let secao = secao(&pastas);
    let _trava = travar();
    for nome in ["CLAUDE.md", "AGENTS.md"] {
        let caminho = pasta.join(nome);
        // Link no lugar das instruções: não escreve através dele.
        if fs::symlink_metadata(&caminho).is_ok_and(|m| !m.file_type().is_file()) { continue; }
        let atual = fs::read_to_string(&caminho).unwrap_or_default();
        let novo = com_secao(&atual, &secao);
        if novo != atual { gravar_atomico(&caminho, novo.as_bytes())?; }
    }
    let dir = pasta.join(".claude");
    if !arquivos::pasta_comum(&dir) { arquivos::criar_pasta(&dir)?; }
    let config = dir.join("settings.json");
    let novo = configuracao_do_claude(arquivos::ler_json(&config), &pastas);
    if arquivos::ler_json(&config).as_ref() != Some(&novo) { arquivos::gravar_json(&config, &novo)?; }
    Ok(pastas)
}

/// Assinatura do que o preview e a timeline mostram: muda quando o agente edita a composição, o
/// briefing ou as mídias. Só metadados (nada é lido).
pub fn assinatura(pasta: &Path) -> u64 {
    let mut h = DefaultHasher::new();
    let mut arquivos: Vec<PathBuf> = [projetos::INDEX, projetos::BRIEF, projetos::MIDIAS].iter().map(|n| pasta.join(n)).collect();
    if let Ok(itens) = fs::read_dir(pasta.join("compositions")) {
        let mut comps: Vec<PathBuf> = itens.flatten().map(|i| i.path()).filter(|p| p.extension().is_some_and(|e| e == "html")).collect();
        comps.sort();
        arquivos.extend(comps);
    }
    for a in arquivos {
        a.file_name().hash(&mut h);
        if let Ok(m) = fs::symlink_metadata(&a) {
            m.len().hash(&mut h);
            m.modified().ok().and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok()).map(|d| d.as_nanos()).hash(&mut h);
        }
    }
    h.finish()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn projeto(raiz: &Path) -> String {
        let novo = projetos::NovoProjeto {
            projeto: serde_json::json!({ "nome": "Teste", "formato": "9:16" }),
            brief: "# Brief\n".into(), index: "<div data-composition-id=\"main\"></div>".into(), instrucoes: "# Instruções\n\nTexto.\n".into(),
        };
        projetos::criar(raiz, &novo, None, 1_000, 7).unwrap()
    }

    #[test]
    fn pasta_do_computador_exige_pasta_absoluta_que_existe() {
        let dir = tempfile::tempdir().unwrap();
        assert!(pasta_do_computador("").is_err());
        assert!(pasta_do_computador("relativa/x").is_err());
        assert!(pasta_do_computador(&dir.path().join("nao-existe").to_string_lossy()).is_err());
        fs::write(dir.path().join("a.txt"), "x").unwrap();
        assert!(pasta_do_computador(&dir.path().join("a.txt").to_string_lossy()).is_err());
        assert!(pasta_do_computador("/").is_err());
        assert_eq!(pasta_do_computador(&dir.path().to_string_lossy()).unwrap(), fs::canonicalize(dir.path()).unwrap());
    }

    #[test]
    fn configurar_escreve_secao_e_settings_sem_perder_o_resto() {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dir.path().join("videos");
        let id = projeto(&raiz);
        let marca = dir.path().join("Medza");
        fs::create_dir_all(&marca).unwrap();
        let r = referencias::criar(&raiz, &serde_json::json!({ "nome": "Reels", "descricao": "cortes rápidos", "links": [{ "url": "https://x.com/a", "nota": "ritmo" }] }), 1, 2).unwrap();
        let ref_id = r["id"].as_str().unwrap().to_owned();
        let pasta = raiz.join("projetos").join(&id);
        fs::create_dir_all(pasta.join(".claude")).unwrap();
        fs::write(pasta.join(".claude/settings.json"), r#"{"model":"x","permissions":{"allow":["Read"]}}"#).unwrap();

        let pastas = configurar(&raiz, &id, Some(&marca.to_string_lossy()), &[ref_id.clone(), "r-sumiu".into()]).unwrap();
        assert_eq!(pastas.len(), 2, "pasta de referências apagada só sai da lista");
        let claude = fs::read_to_string(pasta.join("CLAUDE.md")).unwrap();
        assert!(claude.starts_with("# Instruções\n\nTexto."), "o texto do app fica");
        assert!(claude.contains("Pasta do computador “Medza”") && claude.contains("cortes rápidos") && claude.contains("https://x.com/a (ritmo)"));
        assert_eq!(fs::read_to_string(pasta.join("AGENTS.md")).unwrap(), claude);
        let s: serde_json::Value = serde_json::from_str(&fs::read_to_string(pasta.join(".claude/settings.json")).unwrap()).unwrap();
        assert_eq!(s["model"], "x");
        let dirs: Vec<&str> = s["permissions"]["additionalDirectories"].as_array().unwrap().iter().map(|v| v.as_str().unwrap()).collect();
        assert_eq!(dirs, vec![fs::canonicalize(&marca).unwrap().to_string_lossy().as_ref(), raiz.join("referencias").join(&ref_id).join("arquivos").to_string_lossy().as_ref()]);
        assert!(s["permissions"]["allow"].as_array().unwrap().iter().any(|v| v == "Read"));

        // Tirar tudo: a seção diz que não há pastas, sem duplicar, e o settings fica sem pastas.
        configurar(&raiz, &id, None, &[]).unwrap();
        let claude = fs::read_to_string(pasta.join("CLAUDE.md")).unwrap();
        assert_eq!(claude.matches(INICIO).count(), 1);
        assert!(claude.contains("Nenhuma por enquanto") && !claude.contains("Medza"));
        let s: serde_json::Value = serde_json::from_str(&fs::read_to_string(pasta.join(".claude/settings.json")).unwrap()).unwrap();
        assert_eq!(s["permissions"]["additionalDirectories"], serde_json::json!([]));
        assert_eq!(s["permissions"]["allow"].as_array().unwrap().len(), 3, "regras do lint não se repetem");
    }

    #[test]
    fn assinatura_muda_quando_a_composicao_muda() {
        let dir = tempfile::tempdir().unwrap();
        let raiz = dir.path().join("videos");
        let id = projeto(&raiz);
        let pasta = raiz.join("projetos").join(&id);
        let antes = assinatura(&pasta);
        assert_eq!(antes, assinatura(&pasta));
        fs::write(pasta.join("index.html"), "<div data-composition-id=\"main\">mudou</div>").unwrap();
        let depois = assinatura(&pasta);
        assert_ne!(antes, depois);
        fs::write(pasta.join("compositions/cena.html"), "<div></div>").unwrap();
        assert_ne!(depois, assinatura(&pasta));
    }
}
