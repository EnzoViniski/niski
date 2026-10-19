//! Fork: pasta onde o Claude Code guarda os transcripts de um projeto, igual à da CLI também para
//! caminho longo.
//!
//! A CLI troca todo caractere não-alfanumérico por `-` e, quando o resultado passa de 200
//! caracteres, corta em 200 e acrescenta `-<hash>` (djb2 do caminho original, base 36). Sem o corte,
//! o OMNI procurava `projects/<slug inteiro>/<id>.jsonl`, que nunca existe: a Conversa ficava vazia
//! com o agente trabalhando, e o reinício não achava o transcript e repetia `--session-id` (que a CLI
//! recusa por já estar em uso) em vez de `--resume`. Caso real: projeto dentro de
//! `~/Library/Application Support/Claude/local-agent-mode-sessions/...` (214 caracteres).
//!
//! A CLI rodando no Bun usa outro hash (`Bun.hash`), e ela mesma tolera isso procurando pelo
//! prefixo. [`localizar`] faz o mesmo quando o arquivo não está na pasta calculada.

use crate::conversations::{Conversation, Segment};
use std::path::{Path, PathBuf};

/// Limite da CLI (`MAX_SANITIZED_LENGTH`), em unidades UTF-16 como no JavaScript.
const LIMITE: usize = 200;

/// `hash = hash * 31 + c` em 32 bits com sinal, sobre as unidades UTF-16 (o `charCodeAt` do JS).
fn djb2(texto: &str) -> i32 {
    texto.encode_utf16().fold(0_i32, |hash, unidade| hash.wrapping_shl(5).wrapping_sub(hash).wrapping_add(i32::from(unidade)))
}

fn base36(mut valor: u64) -> String {
    const DIGITOS: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    if valor == 0 { return "0".into(); }
    let mut saida = Vec::new();
    while valor > 0 { saida.push(DIGITOS[(valor % 36) as usize]); valor /= 36; }
    saida.reverse();
    String::from_utf8(saida).unwrap_or_default()
}

/// Cada unidade UTF-16 vira um caractere (o regex `/[^a-zA-Z0-9]/g` do JS não tem a flag `u`: um
/// emoji vira dois `-`). Devolve o texto trocado e quantas unidades ele tem.
fn trocado(caminho: &str) -> Vec<char> {
    caminho.encode_utf16().map(|unidade| match char::from_u32(u32::from(unidade)) {
        Some(c) if c.is_ascii_alphanumeric() => c,
        _ => '-',
    }).collect()
}

/// Nome da pasta do projeto em `<config>/projects/`.
pub fn slug(cwd: &str) -> String {
    let caminho = cwd.trim_end_matches(['\\', '/']);
    let trocado = trocado(caminho);
    if trocado.len() <= LIMITE { return trocado.into_iter().collect(); }
    let prefixo: String = trocado[..LIMITE].iter().collect();
    format!("{prefixo}-{}", base36(u64::from(djb2(caminho).unsigned_abs())))
}

/// Prefixo comum a todas as pastas possíveis de um caminho longo (`<200 caracteres>-`); `None` para
/// caminho curto, cuja pasta é uma só.
fn prefixo_longo(cwd: &str) -> Option<String> {
    let trocado = trocado(cwd.trim_end_matches(['\\', '/']));
    (trocado.len() > LIMITE).then(|| trocado[..LIMITE].iter().collect::<String>() + "-")
}

/// Transcript da sessão. A pasta calculada vale quando o arquivo está lá ou ainda não existe (sessão
/// nova: é onde a CLI vai gravar). Caminho longo sem o arquivo na pasta calculada: procura nas pastas
/// com o mesmo prefixo, como a própria CLI faz.
pub fn localizar(config_dir: &Path, cwd: &str, session_id: &str) -> PathBuf {
    let arquivo = format!("{session_id}.jsonl");
    let projetos = config_dir.join("projects");
    let calculado = projetos.join(slug(cwd)).join(&arquivo);
    if calculado.is_file() { return calculado; }
    let Some(prefixo) = prefixo_longo(cwd) else { return calculado };
    std::fs::read_dir(&projetos).into_iter().flatten().flatten()
        .filter(|pasta| pasta.file_name().to_string_lossy().starts_with(&prefixo))
        .map(|pasta| pasta.path().join(&arquivo))
        .find(|caminho| caminho.is_file())
        .unwrap_or(calculado)
}

/// O índice de conversas guardou, para projetos de caminho longo, um `transcript_path` com o slug
/// sem corte, que nunca existe. Trecho do Claude com caminho guardado ausente: troca pelo caminho
/// atual quando ele existe. Os outros trechos voltam como estão.
pub fn trecho_com_transcript_atual(segment: &Segment, conversation: &Conversation, profiles: &[crate::Profile]) -> Option<Segment> {
    if segment.provider != "claude" { return None; }
    let guardado = segment.transcript_path.as_deref()?;
    if Path::new(guardado).exists() { return None; }
    let sessao = segment.external_session_id.as_deref()?;
    let perfil = profiles.iter().find(|p| Some(&p.id) == segment.profile_id.as_ref() && p.provider == "claude")?;
    let atual = localizar(Path::new(&perfil.config_dir), &conversation.cwd, sessao);
    atual.is_file().then(|| Segment { transcript_path: Some(atual.to_string_lossy().into_owned()), ..segment.clone() })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Caminho longo sintético, no formato que o Claude Code 2.x usa. O algoritmo foi conferido contra
    /// uma pasta real criada pela CLI; o valor esperado abaixo trava o resultado contra regressão.
    const LONGO: &str = "/Users/ana/Library/Application Support/Claude/local-agent-mode-sessions/00000000-1111-2222-3333-444444444444/55555555-6666-7777-8888-999999999999/local_aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee/outputs/projeto-exemplo";
    const PASTA_DO_LONGO: &str = "-Users-ana-Library-Application-Support-Claude-local-agent-mode-sessions-00000000-1111-2222-3333-444444444444-55555555-6666-7777-8888-999999999999-local-aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee-outputs-pro-dfxask";

    #[test]
    fn caminho_longo_e_cortado_com_hash_como_na_cli() {
        assert_eq!(slug(LONGO), PASTA_DO_LONGO);
        assert_eq!(slug(&format!("{LONGO}/")), PASTA_DO_LONGO, "barra final não muda o hash");
    }

    #[test]
    fn caminho_curto_nao_muda() {
        assert_eq!(slug("/Users/ana/scraper"), "-Users-ana-scraper");
        assert_eq!(slug(&"a".repeat(200)), "a".repeat(200), "200 cabe sem corte");
        assert_eq!(slug(&"a".repeat(201)).len(), 200 + 1 + base36(u64::from(djb2(&"a".repeat(201)).unsigned_abs())).len());
    }

    #[test]
    fn hash_do_bun_e_achado_pelo_prefixo() {
        let dir = tempfile::tempdir().unwrap();
        let pasta = dir.path().join("projects").join(format!("{}-outrohash", &PASTA_DO_LONGO[..200]));
        std::fs::create_dir_all(&pasta).unwrap();
        std::fs::write(pasta.join("s1.jsonl"), "{}\n").unwrap();
        assert_eq!(localizar(dir.path(), LONGO, "s1"), pasta.join("s1.jsonl"));
        // Sessão que ainda não gravou nada: o caminho calculado, onde a CLI vai escrever.
        assert_eq!(localizar(dir.path(), LONGO, "s2"), dir.path().join("projects").join(PASTA_DO_LONGO).join("s2.jsonl"));
    }

    /// O bug do projeto do Yorcookie: agente trabalhando, Conversa vazia. O índice guardou o caminho
    /// sem corte (que nunca existe) e o timeline pulava o trecho como "ainda não gravado".
    #[test]
    fn conversa_de_caminho_longo_com_caminho_velho_no_indice_mostra_as_mensagens() {
        use crate::conversations::{timeline, TitleSource};
        let dir = tempfile::tempdir().unwrap();
        let config = dir.path().to_string_lossy().into_owned();
        let pasta = dir.path().join("projects").join(PASTA_DO_LONGO);
        std::fs::create_dir_all(&pasta).unwrap();
        let linhas = [
            r#"{"type":"user","uuid":"u1","message":{"role":"user","content":"oi"}}"#,
            r#"{"type":"assistant","uuid":"a1","message":{"role":"assistant","content":[{"type":"text","text":"ola"}]}}"#,
        ];
        std::fs::write(pasta.join("s1.jsonl"), linhas.join("\n") + "\n").unwrap();
        let velho = dir.path().join("projects").join(trocado(LONGO).into_iter().collect::<String>()).join("s1.jsonl");
        let perfil = crate::Profile{id:"p".into(),provider:"claude".into(),name:"T".into(),config_dir:config,builtin:true,created_at_ms:0,last_used_at_ms:None,authenticated:false,email:None};
        let trecho = Segment{provider:"claude".into(),profile_id:Some("p".into()),external_session_id:Some("s1".into()),
            transcript_path:Some(velho.to_string_lossy().into_owned()),terminal_session_id:None,started_at_ms:0,ended_at_ms:None};
        let conversa = Conversation{id:"c".into(),project_id:"pr".into(),cwd:LONGO.into(),title:"t".into(),title_source:TitleSource::default(),created_at_ms:0,segments:vec![trecho]};
        let lida = timeline(&conversa, std::slice::from_ref(&perfil), None, 100);
        assert_eq!(lida.messages.len(), 2);
        assert!(lida.unavailable_segments.is_empty());
        assert_eq!(crate::conversations::fontes(&conversa, std::slice::from_ref(&perfil)).len(), 1);
    }
}
