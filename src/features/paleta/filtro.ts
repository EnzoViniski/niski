import { fuzzyScore } from "../files/QuickOpen";

export interface ItemDaPaleta {
  id: string;
  secao: "Ações" | "Projetos" | "Conversas" | "Arquivos" | "No histórico";
  titulo: string;
  detalhe?: string;
  executar: () => unknown;
}
const semAcentos = (texto: string) => texto.normalize("NFD").replace(/[\u0300-\u036f]/g, "");

/** Mantém as seções e ordena pela relevância dentro delas. */
export function filtrarItens(itens: ItemDaPaleta[], busca: string): ItemDaPaleta[] {
  return (["Ações", "Projetos", "Conversas", "Arquivos", "No histórico"] as const).flatMap((secao) =>
    itens.filter((item) => item.secao === secao)
      .map((item) => ({ item, nota: fuzzyScore(semAcentos(busca), semAcentos(`${item.titulo} ${item.detalhe ?? ""}`)) }))
      .filter((par): par is { item: ItemDaPaleta; nota: number } => par.nota !== null)
      .sort((a, b) => b.nota - a.nota).slice(0, 40).map(({ item }) => item));
}

export function moverSelecao(atual: number, tecla: string, total: number): number {
  if (!total) return 0;
  return tecla === "ArrowDown" ? (atual + 1) % total : tecla === "ArrowUp" ? (atual - 1 + total) % total : Math.min(atual, total - 1);
}
