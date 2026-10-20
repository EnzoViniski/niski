//! Anexos mandados pelo celular/web e a menção de arquivos (`@`) do composer.
//! Fork: saiu do antigo `mobile.rs` na Onda 2.0, sem mudança de lógica.
use super::*;

/// Tamanho máximo de um anexo vindo do celular. Cobre foto de câmera, PDF e print com folga, e
/// ainda cabe na memória sem susto — o corpo é lido inteiro antes de gravar.
pub(super) const ANEXO_MAX_BYTES: usize = 25 * 1024 * 1024;
/// Pasta dos anexos, dentro do projeto da conversa. Dentro do projeto de propósito: a CLI lê sem
/// pedir permissão extra, e é a mesma família do `.omni-agents/pasted/` que o desktop já usa.
pub(super) const PASTA_ANEXOS: [&str; 2] = [".omni-agents", "anexos"];

/// Nome seguro para gravar: só o último componente, sem nada que o shell ou o Windows interpretem.
/// O caminho vai cru no prompt, então espaço e vírgula também saem — a CLI leria só o pedaço.
pub(super) fn nome_seguro(bruto: &str) -> String {
    let base = bruto.rsplit(['/', '\\']).next().unwrap_or("");
    let limpo: String = base.chars()
        .map(|c| if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' { c } else { '-' })
        .collect();
    let limpo = limpo.trim_matches(|c| c == '.' || c == '-');
    // Nome reservado do Windows (`CON`, `NUL`…) ou vazio vira genérico em vez de falhar na escrita.
    let tronco = limpo.split('.').next().unwrap_or("").to_ascii_uppercase();
    let reservado = matches!(tronco.as_str(), "CON"|"PRN"|"AUX"|"NUL") || ((tronco.starts_with("COM") || tronco.starts_with("LPT"))
        && tronco.len() == 4 && tronco.as_bytes()[3].is_ascii_digit());
    let limpo = if limpo.is_empty() || reservado { "anexo" } else { limpo };
    // Nome longo quebra caminho no Windows (MAX_PATH) sem ganho nenhum: corta, preservando a extensão.
    match limpo.rsplit_once('.') {
        Some((tronco, ext)) if !tronco.is_empty() && ext.len() <= 10 => format!("{}.{}", &tronco[..tronco.len().min(60)], ext),
        _ => limpo[..limpo.len().min(70)].to_owned(),
    }
}

pub(super) async fn anexar(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, headers: HeaderMap, corpo: axum::body::Bytes) -> Result<Json<Value>,ApiError> {
    if corpo.is_empty() { return Err(error(StatusCode::BAD_REQUEST,"Arquivo vazio")); }
    let nome = headers.get("x-omni-nome").and_then(|v| v.to_str().ok())
        .map(|v| percent_decode(v)).unwrap_or_default();
    tokio::task::spawn_blocking(move || {
        // A pasta sai da conversa registrada no servidor — o celular nunca manda caminho.
        let (conversation,_) = current(&state,&id)?;
        let mut pasta = std::path::PathBuf::from(&conversation.cwd);
        if !pasta.is_dir() { return Err(error(StatusCode::CONFLICT,"A pasta do projeto não existe mais neste PC")); }
        for parte in PASTA_ANEXOS { pasta.push(parte); }
        std::fs::create_dir_all(&pasta).map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR,"Não foi possível criar a pasta de anexos"))?;
        // Prefixo com data e sequência: dois "image.png" colados seguidos não se sobrescrevem.
        let seguro = nome_seguro(&nome);
        let destino = pasta.join(format!("{}-{:04x}-{}", now_ms(), uuid::Uuid::new_v4().as_u128() as u16, seguro));
        let mut arquivo = std::fs::OpenOptions::new().write(true).create_new(true).open(&destino)
            .map_err(|_| error(StatusCode::CONFLICT,"Já existe um anexo com esse nome; tente de novo"))?;
        use std::io::Write as _;
        if arquivo.write_all(&corpo).is_err() {
            drop(arquivo);
            let _ = std::fs::remove_file(&destino);
            return Err(error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao gravar o anexo no PC"));
        }
        Ok(Json(json!({"caminho":destino.to_string_lossy(),"nome":seguro,"bytes":corpo.len()})))
    }).await.map_err(|_| error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao gravar o anexo"))?
}

/// `X-Omni-Nome` chega com `encodeURIComponent` (cabeçalho HTTP não carrega acento). Sequência
/// inválida vira o próprio texto: o nome ainda passa pelo `nome_seguro` depois.
pub(super) fn percent_decode(valor: &str) -> String {
    let bytes = valor.as_bytes();
    let mut saida = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        // Bytes, não `&valor[..]`: fatiar a `str` no meio de um caractere acentuado dá pânico.
        if bytes[i] == b'%' && i + 2 < bytes.len() {
            let par = std::str::from_utf8(&bytes[i + 1..i + 3]).ok().and_then(|hex| u8::from_str_radix(hex, 16).ok());
            if let Some(byte) = par { saida.push(byte); i += 3; continue; }
        }
        saida.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&saida).into_owned()
}

/* ── Menção de arquivos (@) ───────────────────────────────────────────────────────────────────
   O `@` do composer: a lista sai da pasta do projeto da conversa. Varredura própria, sem depender
   de `rg` (o celular pede ao PC, e o PC pode não ter), com cache curto por pasta. */

/// Pastas que nunca valem uma menção e custam caro de varrer.
pub(super) const PASTAS_IGNORADAS: [&str; 14] = [".git","node_modules","target","target-engine","dist","dist-mobile","build",".next",
    ".turbo",".venv","venv","__pycache__",".omni-agents",".cache"];
pub(super) const ARQUIVOS_MAX: usize = 20_000;
pub(super) const ARQUIVOS_CACHE_MS: u64 = 30_000;

/// Caminhos relativos (com `/`) de tudo que é arquivo na pasta, até `ARQUIVOS_MAX`.
pub(super) fn varrer_arquivos(raiz: &Path) -> Vec<String> {
    let mut saida = Vec::new();
    let mut pilha = vec![(raiz.to_path_buf(), 0_usize)];
    while let Some((pasta, profundidade)) = pilha.pop() {
        let Ok(entradas) = std::fs::read_dir(&pasta) else { continue };
        for entrada in entradas.flatten() {
            let nome = entrada.file_name().to_string_lossy().into_owned();
            let Ok(tipo) = entrada.file_type() else { continue };
            if tipo.is_dir() {
                if profundidade < 12 && !PASTAS_IGNORADAS.contains(&nome.as_str()) { pilha.push((entrada.path(), profundidade + 1)); }
            } else if tipo.is_file() {
                if let Ok(relativo) = entrada.path().strip_prefix(raiz) {
                    saida.push(relativo.to_string_lossy().replace('\\', "/"));
                    if saida.len() >= ARQUIVOS_MAX { return saida; }
                }
            }
        }
    }
    saida.sort();
    saida
}

/// Melhores correspondências para o que foi digitado depois do `@`: nome do arquivo que começa com a
/// busca, depois nome que contém, depois caminho que contém; empate fica com o caminho mais curto.
pub(super) fn melhores_arquivos(lista: &[String], busca: &str, quantos: usize) -> Vec<String> {
    let busca = busca.to_lowercase().replace('\\', "/");
    let mut pontuados: Vec<(u8, usize, &String)> = lista.iter().filter_map(|caminho| {
        let minusculo = caminho.to_lowercase();
        let nome = minusculo.rsplit('/').next().unwrap_or(&minusculo);
        let nota = if busca.is_empty() { 3 }
            else if nome.starts_with(&busca) { 0 }
            else if nome.contains(&busca) { 1 }
            else if minusculo.contains(&busca) { 2 }
            else { return None };
        Some((nota, caminho.len(), caminho))
    }).collect();
    pontuados.sort();
    pontuados.into_iter().take(quantos).map(|(_,_,caminho)| caminho.clone()).collect()
}

#[derive(Deserialize)] pub(super) struct BuscaArquivos { #[serde(default)] q: String }
pub(super) async fn arquivos_da_conversa(State(state): State<Arc<EngineState>>, RoutePath(id): RoutePath<String>, Query(busca): Query<BuscaArquivos>) -> Result<Json<Value>, ApiError> {
    tokio::task::spawn_blocking(move || {
        let pasta = pasta_da_conversa(&state,&id).filter(|p| Path::new(p).is_dir())
            .ok_or_else(||error(StatusCode::NOT_FOUND,"A pasta desta conversa não existe neste PC"))?;
        let lista = {
            let mut cache = state.mobile.arquivos.lock().map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Cache indisponível"))?;
            match cache.get(&pasta) {
                Some((quando, lista)) if now_ms().saturating_sub(*quando) < ARQUIVOS_CACHE_MS => lista.clone(),
                _ => {
                    let lista = Arc::new(varrer_arquivos(Path::new(&pasta)));
                    if cache.len() > 16 { cache.clear(); }
                    cache.insert(pasta.clone(), (now_ms(), lista.clone()));
                    lista
                }
            }
        };
        Ok(Json(json!({"arquivos":melhores_arquivos(&lista,&busca.q,40),"total":lista.len()})))
    }).await.map_err(|_|error(StatusCode::INTERNAL_SERVER_ERROR,"Falha ao listar arquivos"))?
}
