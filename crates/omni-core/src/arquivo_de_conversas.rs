//! Arquivo local de transcripts, independente da limpeza de histórico feita pelas CLIs.
//! Copia apenas JSONL validados das conversas; nunca configurações ou autenticação das contas.
use crate::{Profile, conversations::{Conversation, resolve_transcript}, history::{self, HistoryEntry, ScanCache}};
use std::{collections::HashSet, fs::{self, File, OpenOptions}, io, path::{Path, PathBuf}, sync::atomic::{AtomicU64, Ordering}};

fn perfil_no_arquivo(dir: &Path, perfil: &Profile) -> Profile {
    let id: String = perfil.id.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
    let mut copia = perfil.clone();
    copia.config_dir = dir.join("arquivo-conversas").join(&perfil.provider).join(id).to_string_lossy().into_owned();
    copia
}

pub fn com_arquivo(dir: &Path, perfis: &[Profile]) -> Vec<Profile> {
    let mut todos = perfis.to_vec();
    todos.extend(perfis.iter().filter(|p| matches!(p.provider.as_str(), "claude" | "codex")).map(|p| perfil_no_arquivo(dir, p)));
    todos
}

fn copiar(dir: &Path, perfil: &Profile, origem: &Path) -> io::Result<()> {
    let Some(origem) = history::validated(origem, &perfil.provider, std::slice::from_ref(perfil)) else { return Ok(()) };
    let raiz = Path::new(&perfil.config_dir).canonicalize()?;
    let relativo = origem.strip_prefix(&raiz).map_err(io::Error::other)?;
    // Restringe às árvores nativas de transcript. Um JSONL de configuração não é uma conversa.
    if !matches!(relativo.components().next().and_then(|c| c.as_os_str().to_str()), Some("projects" | "sessions" | "archived_sessions")) { return Ok(()) }
    let destino = PathBuf::from(perfil_no_arquivo(dir, perfil).config_dir).join(relativo);
    fs::create_dir_all(destino.parent().unwrap())?;
    let trava = OpenOptions::new().create(true).truncate(false).write(true).open(destino.with_extension("lock"))?;
    trava.lock()?;
    let meta = fs::metadata(&origem)?;
    if meta.len() == 0 { return Ok(()) }
    if fs::metadata(&destino).is_ok_and(|m| m.len() == meta.len() && m.modified().ok() == meta.modified().ok()) { return Ok(()) }
    static SEQ: AtomicU64 = AtomicU64::new(0);
    let temporario = destino.with_extension(format!("{}.{}.tmp", std::process::id(), SEQ.fetch_add(1, Ordering::Relaxed)));
    let mut opcoes = OpenOptions::new();
    opcoes.write(true).create_new(true);
    #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; opcoes.mode(0o600); }
    let resultado = (|| {
        // Fork (espaço em disco): clone APFS (não ocupa espaço até o original mudar); cópia byte a
        // byte só onde não houver clone.
        let saida = if crate::clonar_arquivo::clonar(&origem, &temporario)? {
            somente_dono(&temporario)?;
            OpenOptions::new().write(true).open(&temporario)?
        } else {
            let mut saida = opcoes.open(&temporario)?;
            io::copy(&mut File::open(&origem)?, &mut saida)?;
            saida
        };
        let depois = fs::metadata(&origem)?;
        // A CLI estava escrevendo: a próxima rodada pega um snapshot estável.
        if meta.len() != depois.len() || meta.modified().ok() != depois.modified().ok() { return Ok(()) }
        if let Ok(modificacao) = meta.modified() { saida.set_times(fs::FileTimes::new().set_modified(modificacao))?; }
        saida.sync_all()?;
        fs::rename(&temporario, &destino)
    })();
    let _ = fs::remove_file(&temporario);
    resultado
}

/// Snapshot das conversas conhecidas, limitado pelo chamador (o engine usa a janela de 15 s).
pub fn guardar_indice(dir: &Path, conversas: &[Conversation], perfis: &[Profile]) {
    let mut vistos = HashSet::new();
    for conversa in conversas {
        for trecho in &conversa.segments {
            if !matches!(trecho.provider.as_str(), "claude" | "codex") { continue }
            let Some(caminho) = resolve_transcript(trecho, conversa, perfis) else { continue };
            if !vistos.insert(caminho.clone()) { continue }
            if let Some(perfil) = perfis.iter().find(|p| p.provider == trecho.provider && Some(&p.id) == trecho.profile_id.as_ref()) {
                if copiar(dir, perfil, &caminho).is_err() { crate::registro::aviso_sem_repetir("arquivo_conversas", "historico_nao_arquivado", &[]); }
            }
        }
    }
}

/// A tela Histórico inclui as cópias locais quando o arquivo da CLI já não existe.
pub fn scan(dir: &Path, provider: &str, perfis: &[Profile], cache: &mut ScanCache) -> Vec<HistoryEntry> {
    let nativas = history::scan(provider, perfis, cache);
    for entrada in &nativas {
        if let Some(perfil) = perfis.iter().find(|p| p.provider == provider && p.id == entrada.profile_id) {
            if copiar(dir, perfil, Path::new(&entrada.path)).is_err() { crate::registro::aviso_sem_repetir("arquivo_conversas", "historico_nao_arquivado", &[]); }
        }
    }
    let copias: Vec<_> = perfis.iter().filter(|p| p.provider == provider).map(|p| perfil_no_arquivo(dir, p)).collect();
    let mut ids: HashSet<_> = nativas.iter().map(|e| e.session_id.clone()).collect();
    let mut entradas = nativas;
    entradas.extend(history::scan(provider, &copias, cache).into_iter().filter(|e| ids.insert(e.session_id.clone())));
    entradas.sort_by_key(|e| std::cmp::Reverse(e.updated_at_ms));
    entradas
}

fn somente_dono(caminho: &Path) -> io::Result<()> {
    #[cfg(unix)] { use std::os::unix::fs::PermissionsExt; fs::set_permissions(caminho, fs::Permissions::from_mode(0o600))?; }
    #[cfg(not(unix))] { let _ = caminho; }
    Ok(())
}

/// Fork (espaço em disco): até a 0.9.3 o arquivo guardava cópias byte a byte. Uma vez por pasta de
/// dados, troca as que ainda são idênticas ao original (mesmo tamanho e data, o critério de `copiar`)
/// por clones, devolvendo o espaço. Cópia cujo original sumiu ou mudou fica como está.
pub fn reclonar_copias_antigas(dir: &Path, perfis: &[Profile]) {
    let raiz_do_arquivo = dir.join("arquivo-conversas");
    let marca = raiz_do_arquivo.join(".clones-v1");
    if marca.exists() || !raiz_do_arquivo.is_dir() { return }
    let mut reclonadas = 0usize;
    for perfil in perfis.iter().filter(|p| matches!(p.provider.as_str(), "claude" | "codex")) {
        let copias = PathBuf::from(perfil_no_arquivo(dir, perfil).config_dir);
        let Ok(raiz) = Path::new(&perfil.config_dir).canonicalize() else { continue };
        let mut pendentes = vec![copias.clone()];
        while let Some(pasta) = pendentes.pop() {
            let Ok(itens) = fs::read_dir(&pasta) else { continue };
            for item in itens.flatten() {
                let caminho = item.path();
                let Ok(tipo) = item.file_type() else { continue };
                if tipo.is_dir() { pendentes.push(caminho); continue }
                if !tipo.is_file() || caminho.extension().is_none_or(|e| e != "jsonl") { continue }
                let Ok(relativo) = caminho.strip_prefix(&copias) else { continue };
                if reclonar(&raiz.join(relativo), &caminho).unwrap_or(false) { reclonadas += 1; }
            }
        }
    }
    crate::registro::info("arquivo_conversas_reclonado", &[("arquivos", &reclonadas.to_string())]);
    let _ = fs::write(marca, "");
}

fn reclonar(original: &Path, copia: &Path) -> io::Result<bool> {
    let trava = OpenOptions::new().create(true).truncate(false).write(true).open(copia.with_extension("lock"))?;
    trava.lock()?;
    let (antes, guardada) = (fs::metadata(original)?, fs::metadata(copia)?);
    if antes.len() != guardada.len() || antes.modified().ok() != guardada.modified().ok() { return Ok(false) }
    let temporario = copia.with_extension(format!("{}.reclone.tmp", std::process::id()));
    let _ = fs::remove_file(&temporario);
    if !crate::clonar_arquivo::clonar(original, &temporario)? { return Ok(false) }
    let resultado = (|| {
        let depois = fs::metadata(original)?;
        // A CLI escreveu no meio: a cópia antiga continua valendo.
        if depois.len() != antes.len() || depois.modified().ok() != antes.modified().ok() { return Ok(false) }
        somente_dono(&temporario)?;
        if let Ok(modificacao) = antes.modified() {
            OpenOptions::new().write(true).open(&temporario)?.set_times(fs::FileTimes::new().set_modified(modificacao))?;
        }
        fs::rename(&temporario, copia)?;
        Ok(true)
    })();
    let _ = fs::remove_file(&temporario);
    resultado
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn conversa_continua_no_historico_e_pode_ser_restaurada_quando_a_cli_apaga_o_original() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("conta");
        let perfil = Profile { id:"a".into(),provider:"claude".into(),name:"Padrão".into(),config_dir:config.to_string_lossy().into_owned(),builtin:false,
            created_at_ms:0,last_used_at_ms:None,authenticated:false,email:None };
        let original = config.join("projects").join("-repo").join("abc.jsonl");
        fs::create_dir_all(original.parent().unwrap()).unwrap();
        let texto = "{\"type\":\"user\",\"cwd\":\"/repo\",\"message\":{\"role\":\"user\",\"content\":\"Continue esta tarefa\"}}\n";
        fs::write(&original, texto).unwrap();
        let mut cache = ScanCache::default();
        assert_eq!(scan(dir.path(), "claude", std::slice::from_ref(&perfil), &mut cache).len(), 1);
        fs::remove_file(&original).unwrap();
        let perfis = com_arquivo(dir.path(), std::slice::from_ref(&perfil));
        let entradas = scan(dir.path(), "claude", std::slice::from_ref(&perfil), &mut cache);
        assert_eq!(entradas.len(), 1);
        assert!(history::transcript(Path::new(&entradas[0].path), "claude", &perfis).unwrap().messages[0].text.contains("Continue"));
        let conversa = Conversation { id:"c".into(), project_id:"p".into(),cwd:"/repo".into(),title:"Teste".into(),
            title_source:crate::conversations::TitleSource::Auto,created_at_ms:0,segments:vec![crate::conversations::Segment {
                provider:"claude".into(),profile_id:Some("a".into()),external_session_id:Some("abc".into()),
                transcript_path:Some(original.to_string_lossy().into_owned()),terminal_session_id:None,started_at_ms:0,ended_at_ms:None,
            }] };
        assert_eq!(resolve_transcript(&conversa.segments[0], &conversa, &perfis).unwrap(), PathBuf::from(&entradas[0].path).canonicalize().unwrap());
        let achado = crate::achar_transcript::achar_claude(&perfis, Some("a"), "/repo", "abc").unwrap();
        crate::achar_transcript::colocar_onde_a_cli_procura(&achado, &perfil, "/repo", "abc").unwrap();
        assert_eq!(fs::read_to_string(original).unwrap(), texto);
    }

    #[test]
    fn copias_antigas_identicas_viram_clone_uma_vez_e_as_diferentes_ficam() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("conta");
        let perfil = Profile { id:"a".into(),provider:"claude".into(),name:"Padrão".into(),config_dir:config.to_string_lossy().into_owned(),builtin:false,
            created_at_ms:0,last_used_at_ms:None,authenticated:false,email:None };
        let original = config.join("projects").join("-repo").join("abc.jsonl");
        fs::create_dir_all(original.parent().unwrap()).unwrap();
        fs::write(&original, "{\"type\":\"user\"}\n").unwrap();
        let copia = PathBuf::from(perfil_no_arquivo(dir.path(), &perfil).config_dir).join("projects/-repo/abc.jsonl");
        fs::create_dir_all(copia.parent().unwrap()).unwrap();
        fs::copy(&original, &copia).unwrap();
        let data = fs::metadata(&original).unwrap().modified().unwrap();
        File::options().write(true).open(&copia).unwrap().set_times(fs::FileTimes::new().set_modified(data)).unwrap();
        // Cópia de um original que já mudou: fica intacta.
        let outra = copia.with_file_name("velha.jsonl");
        fs::write(&outra, "antiga").unwrap();
        fs::write(original.with_file_name("velha.jsonl"), "nova e maior").unwrap();

        reclonar_copias_antigas(dir.path(), std::slice::from_ref(&perfil));

        assert_eq!(fs::read_to_string(&copia).unwrap(), "{\"type\":\"user\"}\n");
        assert_eq!(fs::metadata(&copia).unwrap().modified().unwrap(), data);
        assert_eq!(fs::read_to_string(&outra).unwrap(), "antiga");
        assert!(dir.path().join("arquivo-conversas/.clones-v1").exists());
        // Segunda vez não faz nada (a marca existe).
        fs::write(&outra, "mexida").unwrap();
        reclonar_copias_antigas(dir.path(), std::slice::from_ref(&perfil));
        assert_eq!(fs::read_to_string(&outra).unwrap(), "mexida");
    }

    #[test]
    fn codex_arquiva_atualiza_e_restaura_rollout_sem_copiar_configuracoes() {
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().join("codex");
        let perfil = Profile { id:"x".into(),provider:"codex".into(),name:"Padrão".into(),config_dir:config.to_string_lossy().into_owned(),builtin:false,
            created_at_ms:0,last_used_at_ms:None,authenticated:false,email:None };
        let original = config.join("sessions/2026/10/20/rollout-2026-10-20-abc.jsonl");
        fs::create_dir_all(original.parent().unwrap()).unwrap();
        let texto = "{\"type\":\"session_meta\",\"payload\":{\"id\":\"abc\",\"cwd\":\"/repo\"}}\n{\"type\":\"response_item\",\"payload\":{\"type\":\"message\",\"role\":\"user\",\"content\":[{\"type\":\"input_text\",\"text\":\"Tarefa\"}]}}\n";
        fs::write(&original, texto).unwrap();
        fs::write(config.join("config.toml"), "model = 'fixture'").unwrap();
        let mut cache = ScanCache::default();
        assert_eq!(scan(dir.path(), "codex", std::slice::from_ref(&perfil), &mut cache).len(), 1);
        let novo = format!("{texto}{{\"type\":\"event_msg\",\"payload\":{{\"type\":\"fixture\"}}}}\n");
        fs::write(&original, &novo).unwrap();
        scan(dir.path(), "codex", std::slice::from_ref(&perfil), &mut cache);
        fs::remove_file(&original).unwrap();
        let perfis = com_arquivo(dir.path(), std::slice::from_ref(&perfil));
        let achado = crate::achar_transcript::achar_codex(&perfis, Some("x"), "abc").unwrap();
        crate::achar_transcript::colocar_onde_a_cli_procura(&achado, &perfil, "/repo", "abc").unwrap();
        assert_eq!(fs::read_to_string(original).unwrap(), novo);
        assert!(!Path::new(&perfis[1].config_dir).join("config.toml").exists());
    }
}
