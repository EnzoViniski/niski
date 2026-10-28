//! Objetivo usa /goal nativo; Repetir controla um ramo que retorna ao controlador.
use super::{Fluxo, No};

pub fn validar_objetivo(no: &No) -> Result<(), String> {
    let objetivo = no.config["objetivo"].as_str().unwrap_or("").trim();
    if objetivo.is_empty()
        || objetivo.chars().count() > 3800
        || objetivo.chars().any(char::is_control)
    {
        return Err("Objetivo: escreva uma condição de até 3800 caracteres, em uma linha".into());
    }
    inteiro(no, "max_turnos", 1, 100)?;
    Ok(())
}

fn inteiro(no: &No, campo: &str, min: u64, max: u64) -> Result<u64, String> {
    no.config[campo]
        .as_u64()
        .filter(|n| (min..=max).contains(n))
        .ok_or_else(|| format!("{}: {campo} deve ser inteiro entre {min} e {max}", no.id))
}

pub fn repetir(no: &No) -> Result<(u32, u64, bool), String> {
    let vezes = inteiro(no, "vezes", 1, 1000)?;
    let minutos = inteiro(no, "intervalo_minutos", 0, 1440)?;
    if no
        .config
        .get("parar_quando_aprovado")
        .is_some_and(|v| !v.is_boolean())
    {
        return Err("Repetir: parar_quando_aprovado deve ser booleano".into());
    }
    Ok((
        vezes as u32,
        minutos * 60_000,
        no.config["parar_quando_aprovado"]
            .as_bool()
            .unwrap_or(false),
    ))
}

pub fn validar_repetir(no: &No, fluxo: &Fluxo) -> Result<(), String> {
    repetir(no)?;
    // Cada iteração precisa voltar ao controlador; o ramo de saída só começa depois do limite.
    let corpo: Vec<_> = super::proximos(fluxo, &no.id, "corpo");
    if corpo.len() != 1
        || corpo.first().is_some_and(|n| n.id == no.id)
        || super::proximos(fluxo, &no.id, "fim").is_empty()
    {
        return Err("Repetir: conecte um ramo em corpo e a continuação em fim".into());
    }
    fn retorna(
        f: &Fluxo,
        atual: &str,
        alvo: &str,
        vistos: &mut Vec<String>,
        prontos: &mut std::collections::HashMap<String, bool>,
    ) -> bool {
        if atual == alvo {
            return true;
        }
        if let Some(ok) = prontos.get(atual) {
            return *ok;
        }
        if vistos.iter().any(|s| s == atual) {
            return false;
        }
        vistos.push(atual.into());
        let Some(no) = f.nos.iter().find(|n| n.id == atual) else {
            return false;
        };
        // Um ramo selecionado de cada vez: sem fan-out, término escondido ou outro controlador.
        if no.tipo == super::TipoDeNo::Repetir {
            return false;
        }
        if super::saidas(&no.tipo).iter().any(|s| {
            f.conexoes
                .iter()
                .filter(|c| c.de == atual && c.saida == *s)
                .count()
                != 1
        }) {
            return false;
        }
        let proximos: Vec<_> = f.conexoes.iter().filter(|c| c.de == atual).collect();
        let ok = !proximos.is_empty()
            && proximos
                .iter()
                .all(|c| retorna(f, &c.para, alvo, &mut vistos.clone(), prontos));
        prontos.insert(atual.into(), ok);
        ok
    }
    if !retorna(
        fluxo,
        &corpo[0].id,
        &no.id,
        &mut vec![],
        &mut std::collections::HashMap::new(),
    ) {
        return Err("Repetir: todo caminho do corpo deve voltar ao nó Repetir".into());
    }
    Ok(())
}

pub fn prompt_objetivo(no: &No, ctx: &super::Contexto) -> Result<String, String> {
    validar_objetivo(no)?;
    let mut copia = no.clone();
    copia.config["prompt"] = no.config["objetivo"].clone();
    let expandido = super::montar_prompt(&copia, ctx);
    if expandido
        .chars()
        .any(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t'))
    {
        return Err("Objetivo contém caracteres de controle".into());
    }
    let condicao = expandido.split_whitespace().collect::<Vec<_>>().join(" ");
    if condicao.chars().count() > 3800 {
        return Err("Objetivo expandido excede 3800 caracteres".into());
    }
    Ok(format!(
        "/goal {condicao} ou pare após {} turnos",
        no.config["max_turnos"]
    ))
}

#[cfg(test)]
mod testes {
    use super::*;
    use serde_json::json;
    #[test]
    fn objetivo_tem_condicao_e_teto() {
        let mut n: No = serde_json::from_value(json!({"id":"o","tipo":"objetivo","agente":"dev","config":{"objetivo":"testes passam para {{entrada}}","max_turnos":4}})).unwrap();
        assert_eq!(
            prompt_objetivo(
                &n,
                &super::super::Contexto {
                    entrada: "auth".into(),
                    ..Default::default()
                }
            )
            .unwrap(),
            "/goal testes passam para auth ou pare após 4 turnos"
        );
        for invalido in [json!(0), json!(101), json!(1.5)] {
            n.config["max_turnos"] = invalido;
            assert!(validar_objetivo(&n).is_err());
        }
        n.config["max_turnos"] = json!(3);
        n.config["objetivo"] = json!("clear\n/model outro");
        assert!(validar_objetivo(&n).is_err());
    }
    #[test]
    fn repeticao_requer_limite_e_retorno() {
        let mut f: Fluxo = serde_json::from_value(json!({"id":"t","nome":"T","nos":[{"id":"e","tipo":"entrada"},{"id":"r","tipo":"repetir","config":{"vezes":3,"intervalo_minutos":2}},{"id":"a","tipo":"agente","agente":"dev"},{"id":"f","tipo":"fim"}],"conexoes":[{"de":"e","para":"r"},{"de":"r","para":"a","saida":"corpo"},{"de":"a","para":"r"},{"de":"r","para":"f","saida":"fim"}]})).unwrap();
        assert!(super::super::validar(&f).is_ok());
        assert_eq!(repetir(&f.nos[1]).unwrap(), (3, 120000, false));
        f.conexoes[2].para = "f".into();
        assert!(super::super::validar(&f).is_err());
        f.conexoes[2].para = "r".into();
        f.nos[1].config["vezes"] = json!(-1);
        assert!(super::super::validar(&f).is_err());
    }
}
