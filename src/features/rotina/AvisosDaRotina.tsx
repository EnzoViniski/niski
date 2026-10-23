// Fork (Rotina): aviso discreto dos lembretes da rotina no canto inferior esquerdo (o direito é do
// lançador de Tarefas e do copiloto). O relógio é do engine (sai com a janela fechada); aqui só lê
// `lembretes-estado.json` a cada 30 s e mostra os que ainda não foram vistos, com os mesmos botões
// do Telegram.
import { useCallback, useEffect, useState } from "react";
import { pedirAbrirRotina } from "./abrirRotina";
import { lerAvisos, operar } from "./rotinaService";
import type { Aviso } from "./tipos";

const ROTULO: Record<string, string> = { feito: "Feito", adiar: "Adiar 15 min", pular: "Pular", copo: "+1 copo", planejar: "Planejar agora" };
/** Aviso mais velho que isto não aparece mais (já passou da hora de fazer sentido). */
const VALIDADE_MS = 3 * 60 * 60 * 1000;

export function AvisosDaRotina({ intervaloMs = 30_000 }: { intervaloMs?: number }) {
  const [avisos, setAvisos] = useState<Aviso[]>([]);
  const ler = useCallback(() => {
    void lerAvisos().then((r) => setAvisos(r.avisos.filter((a) => !a.visto && Date.now() - a.enviado_ms < VALIDADE_MS).slice(0, 3)))
      .catch(() => setAvisos([])); // Engine antigo (sem a Rotina): sem avisos.
  }, []);
  useEffect(() => {
    ler();
    const t = window.setInterval(() => { if (!document.hidden) ler(); }, intervaloMs);
    return () => window.clearInterval(t);
  }, [ler, intervaloMs]);

  const responder = async (aviso: Aviso, acao: string) => {
    setAvisos((lista) => lista.filter((a) => a.chave !== aviso.chave));
    if (acao === "planejar") pedirAbrirRotina({ aba: "semana", conversa: "planejar" });
    try { await operar("responder_aviso", { chave: aviso.chave, acao, minutos: 15 }); } catch { /* some na próxima leitura */ }
    ler();
  };

  if (!avisos.length) return null;
  return (
    <div className="rotina-avisos" role="region" aria-label="Lembretes da rotina" aria-live="polite">
      {avisos.map((a) => (
        <div key={a.chave} className={`rotina-aviso-cartao is-${a.tipo}`}>
          <div className="rotina-aviso-texto">
            <strong>{a.titulo}</strong>
            {a.texto && <span>{a.texto}</span>}
          </div>
          <div className="rotina-linha-botoes">
            {a.botoes.map((b) => (
              <button key={b} type="button" className={b === a.botoes[0] ? "rotina-aviso-principal" : "rotina-mini"} onClick={() => void responder(a, b)}>{ROTULO[b] ?? b}</button>
            ))}
            <button type="button" className="rotina-mini" aria-label={`Dispensar ${a.titulo}`} onClick={() => void responder(a, "visto")}>×</button>
          </div>
        </div>
      ))}
    </div>
  );
}
