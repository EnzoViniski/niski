/**
 * Fork (Manhã): questões que vêm de fora do gerador.
 * - Revisão do roadmap: cards dos seus Roadmaps de estudo (os vencidos primeiro); as outras
 *   respostas viram as alternativas erradas. Responder registra a revisão no FSRS do card.
 * - Tema livre: a IA escolhida (Claude/Codex sem tela, pela conta do usuário) escreve as questões.
 */
import { abrirRoadmap, listarRoadmaps } from "../roadmaps/armazenamento";
import { perguntarJson } from "../roadmaps/ia";
import type { CardDeRevisao, IaEscolhida } from "../roadmaps/tipos";
import { embaralhar, type Nivel, type Questao } from "./gerador";
import type { FontesExtras } from "./rotina";

/** Texto do card sem markdown pesado e cortado, para caber num botão. */
function curto(texto: string, max = 220): string {
  const limpo = texto.replace(/```[\s\S]*?```/g, "[código]").replace(/[#>*_`]/g, "").replace(/\s+/g, " ").trim();
  return limpo.length > max ? `${limpo.slice(0, max - 1)}…` : limpo;
}

export function questoesDeCards(cards: { card: CardDeRevisao; roadmap: string; nome: string }[], quantidade: number, rng: () => number, agora = new Date()): Questao[] {
  const unicos = cards.filter((c, i, todos) => todos.findIndex((x) => curto(x.card.verso) === curto(c.card.verso)) === i);
  if (unicos.length < 4) return [];
  const vencidos = unicos.filter((c) => Date.parse(c.card.fsrs.due) <= agora.getTime()).sort((a, b) => Date.parse(a.card.fsrs.due) - Date.parse(b.card.fsrs.due));
  const outros = embaralhar(rng, unicos.filter((c) => !vencidos.includes(c)));
  return [...vencidos, ...outros].slice(0, quantidade).map((c) => {
    const erradas = embaralhar(rng, unicos.filter((x) => x !== c)).slice(0, 3).map((x) => curto(x.card.verso));
    const opcoes = embaralhar(rng, [curto(c.card.verso), ...erradas]);
    return {
      id: c.card.id, tema: "roadmaps", enunciado: curto(c.card.frente, 400), opcoes, correta: opcoes.indexOf(curto(c.card.verso)),
      explicacao: `Do roadmap "${c.nome}".`, origem: { roadmap: c.roadmap, card: c.card.id, nome: c.nome },
    };
  });
}

async function deRoadmaps(quantidade: number, rng: () => number): Promise<Questao[]> {
  const metas = (await listarRoadmaps()).filter((m) => !m.arquivado && m.fase === "ativo");
  const abertos = await Promise.all(metas.map((m) => abrirRoadmap(m.id).catch(() => null)));
  const cards = abertos.flatMap((r) => (r ? r.cards.cards.map((card) => ({ card, roadmap: r.meta.id, nome: r.meta.nome })) : []));
  return questoesDeCards(cards, quantidade, rng);
}

const SISTEMA = [
  "Você cria questões de múltipla escolha em português do Brasil para um aquecimento mental de manhã.",
  "Responda só com JSON no formato {\"questoes\":[{\"enunciado\":\"...\",\"opcoes\":[\"a\",\"b\",\"c\",\"d\"],\"correta\":0,\"explicacao\":\"...\"}]}.",
  "Exatamente 4 opções por questão, uma só correta (índice 0 a 3, varie a posição), alternativas erradas plausíveis.",
  "Enunciado curto (até 2 frases), explicação em uma frase. Nada de texto fora do JSON.",
].join("\n");

const NIVEL: Record<Nivel, string> = { 1: "fácil", 2: "médio", 3: "difícil" };

export function lerQuestoesDaIa(valor: unknown, quantidade: number): Questao[] | string {
  const lista = (valor as { questoes?: unknown } | null)?.questoes;
  if (!Array.isArray(lista) || !lista.length) return "faltou a lista \"questoes\"";
  const boas: Questao[] = [];
  for (const [i, bruta] of lista.entries()) {
    const q = bruta as Record<string, unknown>;
    const opcoes = Array.isArray(q.opcoes) ? q.opcoes.filter((o): o is string => typeof o === "string" && !!o.trim()).map((o) => o.trim()) : [];
    const correta = typeof q.correta === "number" ? q.correta : -1;
    if (typeof q.enunciado !== "string" || !q.enunciado.trim() || opcoes.length !== 4 || new Set(opcoes).size !== 4 || correta < 0 || correta > 3) continue;
    boas.push({ id: `ia-${i}`, tema: "livre", enunciado: q.enunciado.trim(), opcoes, correta, explicacao: typeof q.explicacao === "string" ? q.explicacao.trim() : "" });
  }
  return boas.length ? boas.slice(0, quantidade) : "nenhuma questão veio com 4 opções distintas e o índice da correta";
}

async function deIa(quantidade: number, assunto: string, ia: IaEscolhida, nivel: Nivel): Promise<Questao[]> {
  return perguntarJson({
    chave: "manha-tema-livre", ia, sistema: SISTEMA, web: false,
    texto: `Crie ${quantidade} questões de nível ${NIVEL[nivel]} sobre: ${assunto}. Varie os subtemas.`,
  }, (v) => lerQuestoesDaIa(v, quantidade));
}

export const FONTES: FontesExtras = { roadmaps: deRoadmaps, livre: deIa };

/** Registra no FSRS do roadmap: acertou = "Bom", errou = "Errei". */
export async function registrarRevisao(q: Questao, acertou: boolean) {
  if (!q.origem) return;
  const { revisar } = await import("../roadmaps/acoes");
  await revisar(q.origem.roadmap, q.origem.card, acertou ? 3 : 1).catch(() => undefined);
}
