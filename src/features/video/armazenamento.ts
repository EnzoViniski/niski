// Fork (Editor de vídeo): chamadas ao Rust (`src-tauri/src/videos/`). Nada de lógica aqui.
import { invoke } from "@tauri-apps/api/core";
import type { ItemDaLista, Midia, MidiaFaltando, PastaDeReferencia, VersaoDoHistorico } from "./tipos";

export interface NovoProjeto { projeto: Record<string, unknown>; brief: string; index: string; instrucoes: string }

export const listarProjetos = () => invoke<ItemDaLista[]>("videos_listar");
export const criarProjeto = (novo: NovoProjeto) => invoke<string>("videos_criar", { novo });
/** Confere as skills e devolve a pasta do projeto no disco. */
export const abrirProjeto = (id: string) => invoke<string>("videos_abrir", { id });
export const lerArquivo = (id: string, arquivo: string) => invoke<string | null>("videos_ler", { id, arquivo });
/** Com `motivo`, o Rust guarda antes uma versão (desfazer). Devolve o número dela. */
export const gravarArquivo = (id: string, arquivo: string, conteudo: string, motivo?: string) =>
  invoke<number | null>("videos_gravar", { id, arquivo, conteudo, motivo: motivo ?? null });
export const anexarNaConversa = (id: string, linha: string) => invoke<void>("videos_anexar_conversa", { id, linha });
export const ordenarProjetos = (ordem: string[]) => invoke<void>("videos_ordenar", { ordem });
export const listarComposicoes = (id: string) => invoke<string[]>("videos_composicoes", { id });
export const listarVersoes = (id: string) => invoke<VersaoDoHistorico[]>("videos_versoes", { id });
export const criarVersao = (id: string, motivo: string) => invoke<number | null>("videos_criar_versao", { id, motivo });
export const restaurarVersao = (id: string, n: number) => invoke<number | null>("videos_restaurar_versao", { id, n });
export const apagarProjeto = (id: string) => invoke<void>("videos_apagar", { id });
export const importarMidia = (id: string, caminho: string, papel: string, nota?: string) =>
  invoke<Midia>("videos_importar_midia", { id, caminho, papel, nota: nota ?? null });
export const removerMidia = (id: string, item: string) => invoke<boolean>("videos_remover_midia", { id, item });
export const midiasFaltando = (id: string) => invoke<MidiaFaltando[]>("videos_midias_faltando", { id });
export const backupDoProjeto = (id: string, motivo: string) => invoke<string>("videos_backup", { id, motivo });

export const listarPastas = () => invoke<PastaDeReferencia[]>("videos_ref_listar");
export const criarPasta = (dados: Partial<PastaDeReferencia>) => invoke<PastaDeReferencia>("videos_ref_criar", { dados });
export const editarPasta = (id: string, dados: Partial<PastaDeReferencia>) => invoke<PastaDeReferencia>("videos_ref_editar", { id, dados });
export const lerPasta = (id: string) => invoke<PastaDeReferencia>("videos_ref_ler", { id });
export const importarNaPasta = (id: string, caminho: string, papel: string, nota?: string) =>
  invoke<Midia>("videos_ref_importar", { id, caminho, papel, nota: nota ?? null });
export const removerDaPasta = (id: string, item: string) => invoke<boolean>("videos_ref_remover_item", { id, item });
export const apagarPasta = (id: string) => invoke<void>("videos_ref_apagar", { id });
/** Copia arquivos e links da pasta para o projeto (sem duplicar). */
export const aplicarPasta = (refId: string, projeto: string) => invoke<Midia[]>("videos_ref_aplicar", { refId, projeto });
export const pastaDoProjeto = (projeto: string, dados: Partial<PastaDeReferencia>) =>
  invoke<PastaDeReferencia>("videos_ref_de_projeto", { projeto, dados });
