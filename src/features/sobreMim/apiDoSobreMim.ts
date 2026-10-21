// Fork (Sobre mim): tipos e chamadas da API do engine (`/sobre-mim`), usados pelo desktop e pelo celular.
import { engineApi } from "../terminal/terminalService";

export interface AreaDoSobreMim { id: string; titulo: string; descricao: string; arquivo: string; texto: string }
export interface PropostaDoAgente { linha: string; area: string; texto: string; origem: string | null }
export interface AjustesDoSobreMim { ativo: boolean; sugestoes: boolean }
export interface EstadoDoSobreMim {
  pasta: string;
  ajustes: AjustesDoSobreMim;
  areas: AreaDoSobreMim[];
  propostas: PropostaDoAgente[];
  max_por_area: number;
}

/** Toda rota devolve o estado inteiro relido do disco. */
export type ChamarSobreMim = (metodo: "GET" | "POST", caminho: string, corpo?: unknown) => Promise<EstadoDoSobreMim>;

export const ROTAS = {
  ler: "/sobre-mim",
  area: (id: string) => `/sobre-mim/areas/${encodeURIComponent(id)}`,
  ajustes: "/sobre-mim/ajustes",
  aceitar: "/sobre-mim/propostas/aceitar",
  descartar: "/sobre-mim/propostas/descartar",
};

/** Desktop: pela API local do engine (o mesmo canal da Memória). */
export const chamarPeloEngine: ChamarSobreMim = async (metodo, caminho, corpo) => {
  const resposta = await engineApi(metodo, caminho, corpo === undefined ? null : JSON.stringify(corpo));
  if (resposta.status === 404) throw new Error("Atualize o app para usar o Sobre mim.");
  if (resposta.status >= 400) {
    let mensagem = "Não foi possível acessar o Sobre mim.";
    try { mensagem = (JSON.parse(resposta.body) as { error?: string }).error ?? mensagem; } catch { /* erro sem JSON */ }
    throw new Error(mensagem);
  }
  return JSON.parse(resposta.body) as EstadoDoSobreMim;
};

export function contarCaracteres(texto: string): number {
  return Array.from(texto).length;
}

/** Prompt para colar numa conversa: o agente só propõe, quem grava é o usuário (pelo app). */
export function promptParaOrganizar(pasta: string): string {
  return [
    `Leia os arquivos .md da pasta "${pasta}" (o meu "Sobre mim" do Medza Agents, um arquivo por área) e o propostas.md, com as sugestões pendentes.`,
    "Proponha uma versão organizada de cada área: tópicos curtos, sem repetição, sem perder nenhum fato e sem inventar nada.",
    "Aponte o que parece desatualizado ou contraditório e pergunte antes de remover.",
    "Não grave nenhum arquivo: me mostre o texto final de cada área para eu colar no app.",
  ].join("\n");
}
