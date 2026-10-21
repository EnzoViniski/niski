//! Fork: cópia dos arquivos de estado antes de instalar uma versão nova pelo updater.
//!
//! O updater troca só o `.app` (mesmo bundle id): a pasta de dados fica como está. Mesmo assim, uma
//! versão nova pode reescrever um formato de estado ao abrir; esta cópia é o caminho de volta.
//!
//! Vai para `<pasta de dados do engine>/backups-de-atualizacao/<versão>-<AAAAMMDD-HHMMSS>/`, só as 3
//! mais recentes ficam. Só entra o que está na lista [`PERMITIDOS`] (arquivo comum, sem link, até
//! [`TAMANHO_MAXIMO`]): nunca `engine.token`, `mobile.json`, `ssh.json`, credenciais, transcripts ou logs.
//! A interface manda junto o que ela guarda no localStorage (workspace/abas, configurações), também
//! por lista ([`CHAVES_DA_INTERFACE`]), gravado em `interface.json`.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

pub const PASTA: &str = "backups-de-atualizacao";
const MANTER: usize = 3;
const TAMANHO_MAXIMO: u64 = 16 * 1024 * 1024;
const TAMANHO_MAXIMO_DA_INTERFACE: usize = 8 * 1024 * 1024;

/// Estado pequeno do engine (nomes lidos no código: `omni-engine`, `omni-core`, `src-tauri`).
pub const PERMITIDOS: &[&str] = &[
    "sessions.json",              // abas de agente/terminal do engine (main.rs)
    "conversations.json",         // índice de conversas (indice_de_conversas.rs)
    "profiles.json",              // contas por pasta de dados
    "workspace.json",             // projetos publicados para o celular (mobile/mod.rs)
    "orquestracao.json",          // orquestradores e subagentes
    "routing.json",               // roteamento de contas
    "agendamentos.json",
    "modos-das-conversas.json",
    "conversas-ocultas.json",
    "atividade-vista.json",
    "memoria-compartilhada.json",
    "extensoes.json",
    "ganchos.json",
    "ganchos-confirmados.json",
    "sessoes.json",               // limite de sessões
    "antigravity.json",
    "codex-app-server.json",
    "estado-da-interface.json",   // cópia do localStorage da interface (estado_da_interface.rs)
    // Fork (Sobre mim): `omni_core::sobre_mim`, um markdown por área (o LEIA-ME.md é gerado).
    "sobre-mim/quem-sou.md",
    "sobre-mim/vida-pessoal.md",
    "sobre-mim/trabalho.md",
    "sobre-mim/estudos.md",
    "sobre-mim/preferencias.md",
    "sobre-mim/outros.md",
    "sobre-mim/propostas.md",
    "sobre-mim/ajustes.json",
];

/// Chaves do localStorage da interface que valem guardar (workspace, abas, configurações).
pub const CHAVES_DA_INTERFACE: &[&str] = &[
    "omni-agents-workspaces",
    "omni-agents-workspace",
    "omni-agents-settings",
    "omni-agents-kanban",
    "omni-agents-recent-projects",
    "medza-escolhas",
];

#[derive(Debug, serde::Serialize)]
pub struct Backup {
    pub pasta: String,
    pub arquivos: usize,
}

/// Versão em nome de pasta: só `[0-9A-Za-z.+-]`.
fn versao_segura(versao: &str) -> String {
    let limpa: String = versao.chars().filter(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+')).take(40).collect();
    if limpa.is_empty() { "desconhecida".into() } else { limpa }
}

/// `AAAAMMDD-HHMMSS` em UTC, sem dependência de data (algoritmo de dias civis de Howard Hinnant).
/// Fork (Roadmaps): também nomeia a cópia antes de migrar um roadmap.
pub(crate) fn carimbo(agora: SystemTime) -> String {
    let segundos = agora.duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0) as i64;
    let (dias, resto) = (segundos.div_euclid(86_400), segundos.rem_euclid(86_400));
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let dia = doy - (153 * mp + 2) / 5 + 1;
    let mes = if mp < 10 { mp + 3 } else { mp - 9 };
    let ano = yoe + era * 400 + if mes <= 2 { 1 } else { 0 };
    format!("{ano:04}{mes:02}{dia:02}-{:02}{:02}{:02}", resto / 3600, resto % 3600 / 60, resto % 60)
}

/// `<versão>-AAAAMMDD-HHMMSS` → o carimbo, que ordena por data entre versões diferentes.
fn carimbo_do_nome(nome: &str) -> Option<&str> {
    let carimbo = nome.get(nome.len().checked_sub(15)?..)?;
    let separado = nome.len() > 16 && nome.as_bytes()[nome.len() - 16] == b'-';
    let formato = carimbo.bytes().enumerate().all(|(i, b)| if i == 8 { b == b'-' } else { b.is_ascii_digit() });
    (separado && formato).then_some(carimbo)
}

/// Copia o permitido de `origem` para `origem/backups-de-atualizacao/<versão>-<data>/` e apaga os
/// backups além dos 3 mais recentes. `interface`: chave → valor do localStorage (filtrado aqui).
pub fn fazer_backup(origem: &Path, versao: &str, interface: &BTreeMap<String, String>, agora: SystemTime) -> io::Result<Backup> {
    let raiz = origem.join(PASTA);
    let (versao, carimbo) = (versao_segura(versao), carimbo(agora));
    let mut destino = raiz.join(format!("{versao}-{carimbo}"));
    // Dois cliques no mesmo segundo não misturam cópias.
    let mut n = 1;
    while destino.exists() {
        n += 1;
        destino = raiz.join(format!("{versao}-{n}-{carimbo}"));
    }
    fs::create_dir_all(&destino)?;

    let mut arquivos = 0;
    for nome in PERMITIDOS {
        let caminho = origem.join(nome);
        // `symlink_metadata`: link não é seguido (poderia apontar para fora da pasta).
        let Ok(meta) = fs::symlink_metadata(&caminho) else { continue };
        if !meta.file_type().is_file() || meta.len() > TAMANHO_MAXIMO { continue; }
        // Fork (Sobre mim): item numa subpasta; a subpasta também não pode ser link.
        if let Some((sub, _)) = nome.split_once('/') {
            if !fs::symlink_metadata(origem.join(sub)).map(|m| m.is_dir()).unwrap_or(false) { continue; }
            fs::create_dir_all(destino.join(sub))?;
        }
        fs::copy(&caminho, destino.join(nome))?;
        arquivos += 1;
    }

    // Fork (Roadmaps): a pasta inteira dos roadmaps, com a mesma regra (lista fechada, sem link).
    arquivos += crate::roadmaps::armazenamento::copiar_tudo(&origem.join(crate::roadmaps::armazenamento::PASTA), &destino.join(crate::roadmaps::armazenamento::PASTA))?;
    // Fork (Editor de vídeo): o texto dos projetos e das pastas de referências; mídia grande fica pelo hash.
    arquivos += crate::videos::projetos::copiar_tudo(&origem.join(crate::videos::projetos::PASTA), &destino.join(crate::videos::projetos::PASTA))?;

    let salvas: BTreeMap<&str, &str> = interface.iter()
        .filter(|(chave, _)| CHAVES_DA_INTERFACE.contains(&chave.as_str()))
        .map(|(chave, valor)| (chave.as_str(), valor.as_str()))
        .collect();
    let total: usize = salvas.values().map(|valor| valor.len()).sum();
    if !salvas.is_empty() && total <= TAMANHO_MAXIMO_DA_INTERFACE {
        let texto = serde_json::to_vec_pretty(&salvas).map_err(io::Error::other)?;
        fs::write(destino.join("interface.json"), texto)?;
        arquivos += 1;
    }

    podar(&raiz)?;
    Ok(Backup { pasta: destino.to_string_lossy().into_owned(), arquivos })
}

/// Mantém os [`MANTER`] mais recentes. Só mexe em pastas com o formato de nome deste módulo.
fn podar(raiz: &Path) -> io::Result<()> {
    let mut nossas: Vec<(String, PathBuf)> = fs::read_dir(raiz)?
        .flatten()
        .filter(|item| item.file_type().map(|tipo| tipo.is_dir()).unwrap_or(false))
        .filter_map(|item| {
            let nome = item.file_name().to_string_lossy().into_owned();
            carimbo_do_nome(&nome).map(|carimbo| (format!("{carimbo}{nome}"), item.path()))
        })
        .collect();
    nossas.sort_by(|a, b| b.0.cmp(&a.0));
    for (_, pasta) in nossas.into_iter().skip(MANTER) {
        fs::remove_dir_all(pasta)?;
    }
    Ok(())
}

/// Chamado pela interface antes de baixar a versão nova (`src/lib/atualizacaoRemota.ts`).
#[tauri::command(async)]
pub fn backup_antes_de_atualizar(app: tauri::AppHandle, interface: Option<BTreeMap<String, String>>) -> Result<Backup, String> {
    let origem = crate::engine_client::engine_dir()?;
    let versao = app.package_info().version.to_string();
    let backup = fazer_backup(&origem, &versao, &interface.unwrap_or_default(), SystemTime::now()).map_err(|erro| {
        omni_core::registro::aviso("backup_de_atualizacao_falhou", &[("erro", &erro.kind().to_string())]);
        format!("Não consegui guardar a cópia do estado: {erro}")
    })?;
    omni_core::registro::info("backup_de_atualizacao", &[("arquivos", &backup.arquivos.to_string())]);
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn em(segundos: u64) -> SystemTime { UNIX_EPOCH + Duration::from_secs(segundos) }

    fn pasta_com_estado() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("conversations.json"), r#"{"version":1,"conversations":[]}"#).unwrap();
        fs::write(dir.path().join("sessions.json"), "[]").unwrap();
        fs::write(dir.path().join("profiles.json"), r#"{"profiles":[]}"#).unwrap();
        // Proibidos (conteúdo sintético): nunca podem ir para o backup.
        fs::write(dir.path().join("engine.token"), "sintetico").unwrap();
        fs::write(dir.path().join("mobile.json"), "{}").unwrap();
        fs::write(dir.path().join("ssh.json"), "{}").unwrap();
        fs::write(dir.path().join(".credentials.json"), "{}").unwrap();
        fs::create_dir_all(dir.path().join("execucoes")).unwrap();
        fs::write(dir.path().join("execucoes").join("x.json"), "{}").unwrap();
        fs::write(dir.path().join("transcript.jsonl"), "{}\n").unwrap();
        dir
    }

    #[test]
    fn backup_copia_so_os_permitidos() {
        let dir = pasta_com_estado();
        let mut interface = BTreeMap::new();
        interface.insert("omni-agents-workspaces".to_owned(), r#"{"workspaces":[]}"#.to_owned());
        interface.insert("chave-qualquer".to_owned(), "nao entra".to_owned());
        let feito = fazer_backup(dir.path(), "0.8.0", &interface, em(1_792_454_400)).unwrap();
        let destino = PathBuf::from(&feito.pasta);
        assert_eq!(destino.file_name().unwrap(), "0.8.0-20261020-000000");
        let mut nomes: Vec<String> = fs::read_dir(&destino).unwrap().flatten().map(|i| i.file_name().to_string_lossy().into_owned()).collect();
        nomes.sort();
        assert_eq!(nomes, ["conversations.json", "interface.json", "profiles.json", "sessions.json"]);
        assert_eq!(feito.arquivos, 4);
        let interface_salva = fs::read_to_string(destino.join("interface.json")).unwrap();
        assert!(interface_salva.contains("omni-agents-workspaces") && !interface_salva.contains("chave-qualquer"));
        assert_eq!(fs::read_to_string(destino.join("sessions.json")).unwrap(), "[]");
    }

    #[test]
    fn backup_leva_o_sobre_mim_na_subpasta() {
        let dir = pasta_com_estado();
        fs::create_dir_all(dir.path().join("sobre-mim")).unwrap();
        fs::write(dir.path().join("sobre-mim").join("trabalho.md"), "sintético").unwrap();
        fs::write(dir.path().join("sobre-mim").join("LEIA-ME.md"), "gerado").unwrap();
        let feito = fazer_backup(dir.path(), "0.9.1", &BTreeMap::new(), em(1_792_454_400)).unwrap();
        let destino = PathBuf::from(&feito.pasta);
        assert_eq!(fs::read_to_string(destino.join("sobre-mim").join("trabalho.md")).unwrap(), "sintético");
        assert!(!destino.join("sobre-mim").join("LEIA-ME.md").exists());
        assert_eq!(feito.arquivos, 4);
    }

    // Fork (Roadmaps): fecha, atualiza e continua tudo lá — a cópia leva cada roadmap inteiro.
    #[test]
    fn backup_leva_os_roadmaps() {
        let dir = pasta_com_estado();
        let raiz = dir.path().join(crate::roadmaps::armazenamento::PASTA);
        let id = crate::roadmaps::armazenamento::criar(&raiz, &serde_json::json!({"versao":1,"nome":"Vendas"}), 1, 2).unwrap();
        crate::roadmaps::armazenamento::gravar(&raiz, &id, "cards.json", r#"{"versao":1,"cards":[]}"#).unwrap();
        crate::roadmaps::armazenamento::gravar(&raiz, &id, "etapas/e-1.md", "resumo").unwrap();
        crate::roadmaps::armazenamento::anexar_na_conversa(&raiz, &id, r#"{"de":"eu","texto":"oi"}"#).unwrap();
        let feito = fazer_backup(dir.path(), "0.10.0", &BTreeMap::new(), em(1_792_454_400)).unwrap();
        let copia = PathBuf::from(&feito.pasta).join("roadmaps");
        assert!(copia.join("indice.json").is_file());
        for arquivo in ["meta.json", "cards.json", "conversa.jsonl", "etapas/e-1.md"] {
            assert_eq!(fs::read(copia.join(&id).join(arquivo)).unwrap(), fs::read(raiz.join(&id).join(arquivo)).unwrap(), "{arquivo}");
        }
        assert_eq!(feito.arquivos, 3 + 5);
    }

    #[test]
    fn backup_mantem_os_3_mais_recentes_e_nao_toca_no_resto() {
        let dir = pasta_com_estado();
        let alheia = dir.path().join(PASTA).join("minha-pasta");
        fs::create_dir_all(&alheia).unwrap();
        // Versões diferentes: a ordem é pela data, não pelo nome.
        for (versao, quando) in [("0.9.0", 100), ("0.10.0", 200), ("0.8.0", 300), ("0.8.1", 400)] {
            fazer_backup(dir.path(), versao, &BTreeMap::new(), em(1_792_454_400 + quando)).unwrap();
        }
        let mut nomes: Vec<String> = fs::read_dir(dir.path().join(PASTA)).unwrap().flatten().map(|i| i.file_name().to_string_lossy().into_owned()).collect();
        nomes.sort();
        assert_eq!(nomes, ["0.10.0-20261020-000320", "0.8.0-20261020-000500", "0.8.1-20261020-000640", "minha-pasta"]);
    }

    #[test]
    fn backup_ignora_link_e_versao_estranha() {
        let dir = pasta_com_estado();
        #[cfg(unix)]
        {
            let fora = tempfile::tempdir().unwrap();
            fs::write(fora.path().join("segredo"), "sintetico").unwrap();
            fs::remove_file(dir.path().join("profiles.json")).unwrap();
            std::os::unix::fs::symlink(fora.path().join("segredo"), dir.path().join("profiles.json")).unwrap();
        }
        let feito = fazer_backup(dir.path(), "../../x", &BTreeMap::new(), em(0)).unwrap();
        let destino = PathBuf::from(&feito.pasta);
        assert_eq!(destino.parent().unwrap(), dir.path().join(PASTA));
        assert_eq!(destino.file_name().unwrap(), "....x-19700101-000000");
        #[cfg(unix)]
        assert!(!destino.join("profiles.json").exists());
    }

    #[test]
    fn backup_no_mesmo_segundo_nao_sobrescreve() {
        let dir = pasta_com_estado();
        let a = fazer_backup(dir.path(), "0.8.0", &BTreeMap::new(), em(0)).unwrap();
        let b = fazer_backup(dir.path(), "0.8.0", &BTreeMap::new(), em(0)).unwrap();
        assert_ne!(a.pasta, b.pasta);
        assert!(carimbo_do_nome(Path::new(&b.pasta).file_name().unwrap().to_str().unwrap()).is_some());
    }
}
