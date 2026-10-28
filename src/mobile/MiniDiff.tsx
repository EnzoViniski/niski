import { useState, type ReactNode } from "react";
import type { DiffDeArquivo } from "./api";
import "./conversaRica.css";

/**
 * O que o agente mudou num arquivo, recolhível dentro da linha da ação ("Editou a.ts +3 −1").
 *
 * O engine manda o diff já cortado (até 60 linhas por ação, ver `diff_do_agente.rs`); aqui aparecem
 * as primeiras {@link LINHAS_INICIAIS} e "ver mais" abre o resto. Cores de diff (verde entra,
 * vermelho sai) e, quando uma linha foi só alterada, o trecho que mudou fica realçado.
 */

/** Linhas visíveis antes do "ver mais". */
export const LINHAS_INICIAIS = 12;

const TIPO: Record<DiffDeArquivo["tipo"], string | null> = {
  editado: null, novo: "novo", apagado: "apagado", escrito: "arquivo inteiro",
};

/** `+3 −1`, nas cores do diff. Some quando não há linha contada. */
export function ContagemDoDiff({ adicionadas, removidas }: { adicionadas: number; removidas: number }) {
  if (!adicionadas && !removidas) return null;
  return (
    <span className="diff-contagem" aria-label={`${adicionadas} linhas adicionadas, ${removidas} removidas`}>
      {adicionadas > 0 && <span className="diff-mais">+{adicionadas}</span>}
      {removidas > 0 && <span className="diff-menos">−{removidas}</span>}
    </span>
  );
}

/** Soma das linhas de várias ações ou arquivos. */
export function somarDiffs(diffs: DiffDeArquivo[]): { adicionadas: number; removidas: number } {
  return diffs.reduce((total, d) => ({ adicionadas: total.adicionadas + d.adicionadas, removidas: total.removidas + d.removidas }),
    { adicionadas: 0, removidas: 0 });
}

/** Começo e fim iguais entre a linha que saiu e a que entrou: o meio é o que mudou. */
export function trechoAlterado(antes: string, depois: string): { inicio: number; fimAntes: number; fimDepois: number } | null {
  let inicio = 0;
  while (inicio < antes.length && inicio < depois.length && antes[inicio] === depois[inicio]) inicio += 1;
  let fim = 0;
  while (fim < antes.length - inicio && fim < depois.length - inicio && antes[antes.length - 1 - fim] === depois[depois.length - 1 - fim]) fim += 1;
  // Nada em comum: realçar a linha inteira não diz nada a mais que a cor.
  if (inicio === 0 && fim === 0) return null;
  return { inicio, fimAntes: antes.length - fim, fimDepois: depois.length - fim };
}

function comRealce(texto: string, inicio: number, fim: number): ReactNode {
  if (fim <= inicio) return texto;
  return <>{texto.slice(0, inicio)}<mark className="diff-realce">{texto.slice(inicio, fim)}</mark>{texto.slice(fim)}</>;
}

/** Pares "uma linha sai, uma entra" (linha alterada) ganham o realce do trecho que mudou. */
function conteudos(linhas: string[]): ReactNode[] {
  const saida: ReactNode[] = linhas.map((linha) => linha.slice(1));
  for (let i = 0; i + 1 < linhas.length; i += 1) {
    const sozinhoAntes = i === 0 || linhas[i - 1][0] !== "-";
    const sozinhoDepois = i + 2 >= linhas.length || linhas[i + 2][0] !== "+";
    if (linhas[i][0] !== "-" || linhas[i + 1][0] !== "+" || !sozinhoAntes || !sozinhoDepois) continue;
    const antes = linhas[i].slice(1);
    const depois = linhas[i + 1].slice(1);
    const trecho = trechoAlterado(antes, depois);
    if (!trecho) continue;
    saida[i] = comRealce(antes, trecho.inicio, trecho.fimAntes);
    saida[i + 1] = comRealce(depois, trecho.inicio, trecho.fimDepois);
  }
  return saida;
}

const CLASSE: Record<string, string> = { "+": "diff-linha-mais", "-": "diff-linha-menos", "@": "diff-linha-trecho" };

function DiffDoArquivo({ diff, mostrarNome }: { diff: DiffDeArquivo; mostrarNome: boolean }) {
  const [inteiro, setInteiro] = useState(false);
  const visiveis = inteiro ? diff.linhas : diff.linhas.slice(0, LINHAS_INICIAIS);
  const escondidas = diff.linhas.length - visiveis.length;
  const textos = conteudos(visiveis);
  const tipo = TIPO[diff.tipo];
  return (
    <section className="diff-arquivo" aria-label={`Diff de ${diff.arquivo}`}>
      {(mostrarNome || tipo) && (
        <header className="diff-topo">
          {mostrarNome && <code className="diff-nome" title={diff.arquivo}>{diff.arquivo}</code>}
          {tipo && <span className="diff-tipo">{tipo}</span>}
          {mostrarNome && <ContagemDoDiff adicionadas={diff.adicionadas} removidas={diff.removidas} />}
        </header>
      )}
      {visiveis.length > 0 && (
        <pre className="diff-linhas">
          {visiveis.map((linha, k) => {
            const marca = linha[0];
            return (
              <span key={k} className={`diff-linha ${CLASSE[marca] ?? "diff-linha-igual"}`}>
                <span className="diff-marca" aria-hidden>{marca === "@" ? "⋯" : marca === " " ? " " : marca === "-" ? "−" : marca}</span>
                <span className="diff-texto">{textos[k]}</span>
              </span>
            );
          })}
        </pre>
      )}
      {escondidas > 0 && (
        <button type="button" className="diff-mais-linhas" onClick={() => setInteiro(true)}>
          Ver mais {escondidas === 1 ? "1 linha" : `${escondidas} linhas`}
        </button>
      )}
      {escondidas === 0 && Boolean(diff.omitidas) && (
        <p className="diff-cortado">… mais {diff.omitidas} {diff.omitidas === 1 ? "linha" : "linhas"} — o diff inteiro está no terminal e no Git</p>
      )}
    </section>
  );
}

/** Os arquivos de uma ação. Com um arquivo só, o nome já está na linha da ação. */
export function MiniDiff({ diffs }: { diffs: DiffDeArquivo[] }) {
  return (
    <div className="mini-diff">
      {diffs.map((diff, i) => <DiffDoArquivo key={`${diff.arquivo}-${i}`} diff={diff} mostrarNome={diffs.length > 1} />)}
    </div>
  );
}
