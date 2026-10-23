//! Fork (perguntas na Conversa): rótulo e descrição na mesma linha, em coluna.
//!
//! O Codex alinha a descrição das opções numa coluna ("1. Allow                   Run the tool…"); a
//! opção mais longa fica a **dois** espaços dela ("2. Allow for this session  Run the tool…"). Cortar
//! no primeiro vão de três espaços deixava a descrição grudada no rótulo dessa opção, e o botão do
//! cartão saía com a frase inteira (snapshots oficiais do Codex 0.164, `mcp_server_elicitation` e
//! `request_user_input`). Aqui a coluna é a mesma em todas as opções que têm descrição.

/// Onde termina cada vão de 2+ espaços (o começo do texto depois dele), em caracteres.
fn fins_de_vao(texto: &str) -> Vec<usize> {
    let letras: Vec<char> = texto.chars().collect();
    let mut fins = Vec::new();
    let mut espacos = 0;
    for (i, c) in letras.iter().enumerate() {
        if *c == ' ' { espacos += 1; continue; }
        if espacos >= 2 && i > espacos { fins.push(i); }
        espacos = 0;
    }
    fins
}

/// `(rótulo, descrição)` de cada opção. A coluna é a primeira posição que **todas** as opções com
/// algum vão largo têm em comum; sem coluna comum, só um vão de três espaços separa (o de antes).
pub(crate) fn separar(rotulos: &[String]) -> Vec<(String, Option<String>)> {
    let vaos: Vec<Vec<usize>> = rotulos.iter().map(|r| fins_de_vao(r)).collect();
    let com_vao: Vec<&Vec<usize>> = vaos.iter().filter(|v| !v.is_empty()).collect();
    let coluna = com_vao.first().and_then(|primeiro| primeiro.iter().copied().find(|p| com_vao.iter().all(|v| v.contains(p))));
    rotulos.iter().map(|r| {
        let corte = match coluna {
            Some(c) if fins_de_vao(r).contains(&c) => Some(c),
            Some(_) => None,
            None => r.find("   ").map(|i| r[..i].chars().count()),
        };
        match corte {
            Some(c) => {
                let rotulo: String = r.chars().take(c).collect();
                let descricao: String = r.chars().skip(c).collect();
                let descricao = descricao.trim().to_owned();
                (rotulo.trim().to_owned(), (!descricao.is_empty()).then_some(descricao))
            }
            None => (r.trim().to_owned(), None),
        }
    }).collect()
}

#[cfg(test)]
mod testes {
    use super::*;

    fn s(v: &[&str]) -> Vec<String> { v.iter().map(|x| x.to_string()).collect() }

    #[test]
    fn coluna_comum_separa_ate_a_opcao_mais_longa() {
        let r = separar(&s(&["Allow                   Run the tool and continue", "Allow for this session  Run the tool and remember",
            "Always allow            Run the tool and remember forever", "Cancel                  Cancel this tool call"]));
        assert_eq!(r[1], ("Allow for this session".into(), Some("Run the tool and remember".into())));
        assert_eq!(r[3], ("Cancel".into(), Some("Cancel this tool call".into())));
        let r = separar(&s(&["Option 1           First choice.", "None of the above  Optionally, add details in notes (tab)"]));
        assert_eq!(r[1].0, "None of the above");
    }

    #[test]
    fn sem_coluna_fica_como_antes() {
        assert_eq!(separar(&s(&["Red", "Blue"])), [("Red".into(), None), ("Blue".into(), None)]);
        assert_eq!(separar(&s(&["Lado a lado     ┌────┐"]))[0].0, "Lado a lado");
        assert_eq!(separar(&s(&["Yes, proceed (y)", "No, and tell Codex  (esc)"]))[0], ("Yes, proceed (y)".into(), None),
            "opção sem vão não ganha descrição");
    }
}
