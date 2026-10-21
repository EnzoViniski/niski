//! Fork (Editor de vídeo): edição na mão dentro do preview.
//!
//! O preview roda num iframe de origem opaca: o app não enxerga o DOM da composição. Para clicar num
//! elemento, arrastar, mudar tamanho, cor ou texto, a cópia **servida ao preview** (nunca o arquivo
//! no disco nem o render) ganha:
//!
//! - `data-medza-o="<arquivo>:<n>"` em cada elemento: `n` é a posição da tag de abertura no arquivo
//!   (mesma contagem de `lerTags` em `src/features/video/composicao.ts`), para o editor achar a tag
//!   certa no texto e gravar só ali;
//! - a ponte (`ponte_do_editor.js`): escolhe o elemento clicado, mede e aplica estilos ao vivo, e
//!   conversa com o app só por `postMessage`.

/// Tags que não viram alvo (cabeçalho e código). A contagem `n` inclui todas, como no front.
const SEM_MARCA: &[&str] = &["html", "head", "body", "meta", "title", "link", "base", "script", "style", "noscript"];
const TEXTO_CRU: &[&str] = &["script", "style", "textarea", "title"];

pub const PONTE: &str = include_str!("ponte_do_editor.js");

fn nome_valido(c: u8) -> bool { c.is_ascii_alphanumeric() || c == b':' || c == b'-' }

/// Põe `data-medza-o` em cada tag de abertura (fora as de [`SEM_MARCA`]). Mesmo percurso de `lerTags`:
/// pula comentários, `<!…>`/`<?…>` e o conteúdo de `script`/`style`/`textarea`/`title`.
pub fn marcar(html: &str, arquivo: &str) -> String {
    let b = html.as_bytes();
    let n = b.len();
    let minusculo = html.to_ascii_lowercase();
    let mut saida = String::with_capacity(n + n / 8);
    let mut copiado = 0;
    let mut ordem = 0usize;
    let mut i = 0;
    while i < n {
        let Some(rel) = html[i..].find('<') else { break };
        let lt = i + rel;
        if html[lt..].starts_with("<!--") {
            i = html[lt + 4..].find("-->").map(|f| lt + 4 + f + 3).unwrap_or(n);
            continue;
        }
        if matches!(b.get(lt + 1), Some(b'!') | Some(b'?')) {
            i = html[lt..].find('>').map(|f| lt + f + 1).unwrap_or(n);
            continue;
        }
        let fecha = b.get(lt + 1) == Some(&b'/');
        let mut j = lt + if fecha { 2 } else { 1 };
        let comeco = j;
        while j < n && nome_valido(b[j]) { j += 1; }
        if j == comeco { i = lt + 1; continue; }
        let nome = minusculo[comeco..j].to_owned();
        let fim_do_nome = j;
        // Atributos até o `>` (valor entre aspas pode ter `>`).
        while j < n && b[j] != b'>' {
            if b[j].is_ascii_whitespace() || b[j] == b'/' { j += 1; continue; }
            let inicio = j;
            while j < n && !(b[j].is_ascii_whitespace() || b[j] == b'=' || b[j] == b'>' || b[j] == b'/') { j += 1; }
            let mut k = j;
            while k < n && b[k].is_ascii_whitespace() { k += 1; }
            if k < n && b[k] == b'=' {
                k += 1;
                while k < n && b[k].is_ascii_whitespace() { k += 1; }
                if k < n && (b[k] == b'"' || b[k] == b'\'') {
                    let aspas = b[k];
                    j = b[k + 1..].iter().position(|&c| c == aspas).map(|p| k + 1 + p + 1).unwrap_or(n);
                } else {
                    while k < n && !(b[k].is_ascii_whitespace() || b[k] == b'>') { k += 1; }
                    j = k;
                }
            }
            if j == inicio { j += 1; }
        }
        let fim = (j + 1).min(n);
        if !fecha {
            if !SEM_MARCA.contains(&nome.as_str()) {
                saida.push_str(&html[copiado..fim_do_nome]);
                saida.push_str(&format!(" data-medza-o=\"{}:{ordem}\"", arquivo.replace('"', "")));
                copiado = fim_do_nome;
            }
            ordem += 1;
        }
        i = fim;
        if !fecha && TEXTO_CRU.contains(&nome.as_str()) {
            match minusculo[i..].find(&format!("</{nome}")) { Some(f) => i += f, None => break }
        }
    }
    saida.push_str(&html[copiado..]);
    saida
}

/// Põe `trecho` logo depois da tag `<head …>` (ou no começo).
fn no_head(html: &str, trecho: &str) -> String {
    let baixo = html.to_ascii_lowercase();
    match baixo.find("<head").and_then(|i| baixo[i..].find('>').map(|j| i + j + 1)) {
        Some(i) => format!("{}{trecho}{}", &html[..i], &html[i..]),
        None => format!("{trecho}{html}"),
    }
}

/// HTML do projeto como o preview recebe: marcas, ponte e runtime (o runtime fica primeiro).
pub fn para_o_preview(html: &str, arquivo: &str) -> String {
    let marcado = marcar(html, arquivo);
    let com_ponte = if marcado.contains("__medzaPonte") { marcado } else { no_head(&marcado, &format!("<script>{PONTE}</script>")) };
    super::protocolo::com_runtime(&com_ponte)
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Mesmo texto e mesmas posições do teste de `lerTags` no front (`videoComposicao.test.ts`).
    pub const AMOSTRA: &str = "<!doctype html><html><head><title>a<b></title><style>.x>p{}</style></head><body>\
<!-- <div> --><div id=\"root\" data-composition-id=\"main\"><h1 class='t' data-x=\"a>b\">Oi</h1><img src=x.png>\
<script>if (a<b) document.write('<p>')</script><p>fim</p></div></body></html>";

    #[test]
    fn marca_na_ordem_das_tags_e_pula_cabecalho_comentario_e_codigo() {
        let m = marcar(AMOSTRA, "index.html");
        // Tags de abertura: html0 head1 title2 style3 body4 div5 h1-6 img7 script8 p9.
        assert!(m.contains("<div data-medza-o=\"index.html:5\" id=\"root\""));
        assert!(m.contains("<h1 data-medza-o=\"index.html:6\" class='t' data-x=\"a>b\">Oi</h1>"));
        assert!(m.contains("<img data-medza-o=\"index.html:7\" src=x.png>"));
        assert!(m.contains("<p data-medza-o=\"index.html:9\">fim</p>"));
        assert!(m.contains("document.write('<p>')"), "código dentro de script fica intacto");
        assert!(m.contains("<!-- <div> -->") && m.contains("<title>a<b></title>"));
        assert_eq!(m.matches("data-medza-o").count(), 4);
    }

    #[test]
    fn preview_tem_runtime_depois_ponte_e_o_disco_nao_muda() {
        let html = "<html><head></head><body><div id=\"a\"></div></body></html>";
        let p = para_o_preview(html, "compositions/cena.html");
        let runtime = p.find("hyperframe.runtime").unwrap();
        let ponte = p.find("__medzaPonte").unwrap();
        assert!(runtime < ponte);
        assert!(p.contains("<div data-medza-o=\"compositions/cena.html:3\" id=\"a\">"));
        assert_eq!(p.matches("__medzaPonte").count(), PONTE.matches("__medzaPonte").count(), "uma ponte só");
    }
}

