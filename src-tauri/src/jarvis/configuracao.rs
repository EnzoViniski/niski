//! Fork: preferências sem segredos; histórico e anexos nunca são persistidos aqui.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Ajustes {
    pub perfil_id: String,
    pub modelo: String,
    #[serde(default)]
    pub voz_id: Option<String>,
}

fn caminho() -> Result<PathBuf, String> {
    crate::engine_client::engine_dir()?
        .parent()
        .map(|p| p.join("jarvis").join("configuracao.json"))
        .ok_or("Sem pasta de dados".into())
}

pub fn ler() -> Result<Ajustes, String> {
    let perfis = crate::profiles::list_profiles();
    let salvo = caminho()
        .ok()
        .and_then(|p| std::fs::read(p).ok())
        .and_then(|b| serde_json::from_slice::<Ajustes>(&b).ok());
    if let Some(salvo) = salvo {
        if perfis
            .iter()
            .any(|p| p.id == salvo.perfil_id && p.provider == "claude" && p.authenticated)
        {
            return Ok(salvo);
        }
    }
    let perfil = perfis
        .iter()
        .find(|p| p.provider == "claude" && p.authenticated)
        .ok_or("O Jarvis precisa de uma conta do Claude com login")?;
    // Apelido já presente no catálogo do app; a CLI oficial resolve a versão da conta.
    Ok(Ajustes {
        perfil_id: perfil.id.clone(),
        modelo: "sonnet".into(),
        voz_id: None,
    })
}

pub fn validar(ajustes: &Ajustes) -> Result<(), String> {
    if !crate::profiles::list_profiles()
        .iter()
        .any(|p| p.id == ajustes.perfil_id && p.provider == "claude" && p.authenticated)
    {
        return Err("Escolha uma conta do Claude com login".into());
    }
    if !omni_core::modelos_claude::MODELOS
        .iter()
        .any(|(id, ..)| *id == ajustes.modelo)
    {
        return Err("Escolha um modelo do catálogo do app".into());
    }
    if ajustes
        .voz_id
        .as_ref()
        .is_some_and(|v| v.len() > 300 || v.chars().any(char::is_control))
    {
        return Err("Voz inválida".into());
    }
    Ok(())
}

pub fn gravar(ajustes: &Ajustes) -> Result<(), String> {
    let caminho = caminho()?;
    gravar_em(&caminho, ajustes).map_err(|_| "Não consegui salvar as preferências do Jarvis".into())
}

fn gravar_em(caminho: &Path, ajustes: &Ajustes) -> std::io::Result<()> {
    std::fs::create_dir_all(caminho.parent().unwrap())?;
    let temp = caminho.with_extension("json.tmp");
    std::fs::write(&temp, serde_json::to_vec_pretty(ajustes)?)?;
    std::fs::rename(temp, caminho)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configuracao_so_persiste_preferencias() {
        let pasta = tempfile::tempdir().unwrap();
        let p = pasta.path().join("config.json");
        let a = Ajustes {
            perfil_id: "conta-sintetica".into(),
            modelo: "modelo-sintetico".into(),
            voz_id: Some("voz-sintetica".into()),
        };
        gravar_em(&p, &a).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&std::fs::read(p).unwrap()).unwrap();
        assert_eq!(v.as_object().unwrap().len(), 3);
        assert_eq!(v["voz_id"], "voz-sintetica");
    }
}
