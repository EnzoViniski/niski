// Fork: trabalho de cada pedido recolhido em "Trabalhou por 20m 34s" (pedido do Enzo, 21/10/2026),
// como nos apps do Codex/Claude. Um turno vai da sua mensagem até a próxima. Quando o turno terminou,
// tudo entre a sua mensagem e a **última fala do agente** (falas intermediárias, ações, comandos)
// fica recolhido; a resposta final continua à vista. O turno em andamento nunca recolhe.
import type { Bloco } from "./useTimeline";

export interface Recolhido {
  /** Id da sua mensagem que abriu o turno: chave do abre/fecha. */
  chave: string;
  /** Blocos recolhidos: de `inicio` a `fim` (inclusive). */
  inicio: number;
  fim: number;
  /** Índice da resposta final (a que fica à vista). */
  final: number;
  /** Da sua mensagem até a resposta final, em ms; `null` sem horário. */
  duracaoMs: number | null;
}

const ehSua = (bloco: Bloco) => bloco.tipo === "fala" && bloco.mensagem.role === "user";
const ehFalaDoAgente = (bloco: Bloco) => bloco.tipo === "fala" && bloco.mensagem.role !== "user";

function quando(bloco: Bloco): number | null {
  const texto = bloco.tipo === "acoes" ? bloco.acoes[0]?.timestamp : bloco.mensagem.timestamp;
  const ms = texto ? Date.parse(texto) : NaN;
  return Number.isNaN(ms) ? null : ms;
}

/** `emAndamento`: o último turno ainda está rodando (agente trabalhando, pedindo aprovação…). */
export function turnosRecolhidos(blocos: Bloco[], emAndamento: boolean): Recolhido[] {
  const resultado: Recolhido[] = [];
  const inicios = blocos.flatMap((bloco, i) => (ehSua(bloco) ? [i] : []));
  inicios.forEach((inicioDoTurno, n) => {
    const fimDoTurno = (inicios[n + 1] ?? blocos.length) - 1;
    const ultimo = n === inicios.length - 1;
    if (ultimo && emAndamento) return;
    let final = -1;
    for (let i = fimDoTurno; i > inicioDoTurno; i--) if (ehFalaDoAgente(blocos[i])) { final = i; break; }
    // Sem resposta do agente, ou nada entre a sua mensagem e ela: não há o que recolher.
    if (final < 0 || final === inicioDoTurno + 1) return;
    const sua = quando(blocos[inicioDoTurno]);
    const resposta = quando(blocos[final]);
    resultado.push({
      chave: (blocos[inicioDoTurno] as Extract<Bloco, { tipo: "fala" }>).mensagem.id,
      inicio: inicioDoTurno + 1, fim: final - 1, final,
      duracaoMs: sua !== null && resposta !== null && resposta >= sua ? resposta - sua : null,
    });
  });
  return resultado;
}

/** "45s", "20m 34s", "1h 05m". */
export function duracaoDoTrabalho(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000));
  const h = Math.floor(total / 3600);
  const m = Math.floor((total % 3600) / 60);
  const s = total % 60;
  if (h > 0) return `${h}h ${String(m).padStart(2, "0")}m`;
  if (m > 0) return `${m}m ${String(s).padStart(2, "0")}s`;
  return `${s}s`;
}
