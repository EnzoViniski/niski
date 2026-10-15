//! Aba Extensões: a leitura mora em `omni_core::extensoes` (o engine serve a mesma lista para a web).

#[tauri::command(async)]
pub fn listar_extensoes(project_path: Option<String>) -> Result<Vec<omni_core::extensoes::ExtensoesDaConta>, String> {
    let dir = crate::engine_client::engine_dir()?;
    Ok(omni_core::extensoes::listar(&dir, project_path.as_deref().map(std::path::Path::new)))
}

/// As contas extras recebem as extensões da conta principal ao abrir (ver `omni_core::compartilhar`)?
#[tauri::command(async)]
pub fn compartilhamento_de_extensoes() -> Result<bool, String> {
    Ok(omni_core::compartilhar::ativado(&crate::engine_client::engine_dir()?))
}

/// Estrela: a conta de onde as outras da mesma IA recebem extensões e conectores.
#[tauri::command(async)]
pub fn definir_conta_principal(provider: String, profile_id: String) -> Result<Vec<(String, omni_core::compartilhar::Levado)>, String> {
    omni_core::compartilhar::definir_principal(&crate::engine_client::engine_dir()?, &provider, &profile_id).map_err(|e| e.to_string())
}

/// Liga ou desliga. Ligando, já leva tudo agora para as contas extras — sem esperar a próxima sessão.
#[tauri::command(async)]
pub fn definir_compartilhamento_de_extensoes(ligado: bool) -> Result<Vec<(String, omni_core::compartilhar::Levado)>, String> {
    omni_core::compartilhar::definir_e_levar(&crate::engine_client::engine_dir()?, ligado).map_err(|e| e.to_string())
}
