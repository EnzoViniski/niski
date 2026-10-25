// Fork (Bíblia do dia): Bíblia sintética para os testes (73 livros com os nomes do arquivo real,
// texto inventado). Nunca o texto de verdade: os testes não dependem do download.
import { LIVROS_DA_TRILHA, TRILHAS } from "./biblia";

/** `capitulos` versículos por capítulo; cada versículo termina em ponto a cada `frase` versículos. */
export function arquivoDeTeste({ capitulos = 2, versiculos = 12, frase = 3 } = {}): string {
  const nomes = TRILHAS.flatMap((t) => LIVROS_DA_TRILHA[t]);
  const livro = (nome: string) => ({
    nome,
    capitulos: Array.from({ length: capitulos }, (_, c) => ({
      capitulo: c + 1,
      versiculos: Array.from({ length: versiculos }, (_, v) => ({ versiculo: v + 1, texto: `Texto de ${nome}${(v + 1) % frase === 0 ? "." : ","}` })),
    })),
  });
  const novos = new Set([...LIVROS_DA_TRILHA.evangelhos, ...LIVROS_DA_TRILHA.cartas]);
  return JSON.stringify({ antigoTestamento: nomes.filter((n) => !novos.has(n)).map(livro), novoTestamento: nomes.filter((n) => novos.has(n)).map(livro) });
}
