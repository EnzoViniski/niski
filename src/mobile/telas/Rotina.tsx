// Fork (Rotina no celular): a mesma rotina do PC (rotas `/rotina/*` do engine). Hoje, semana, treino e
// alimentação aqui; ajustes, Telegram e lembretes do sistema continuam no computador. "Conversar" abre a
// conversa do agente da rotina (no PC) e a mostra no chat do celular.
import { useCallback, useEffect, useState } from "react";
import { ApiError, request } from "../api";
import { Button } from "../../components/ui";
import { Alimentacao } from "../../features/rotina/Alimentacao";
import { Hoje } from "../../features/rotina/Hoje";
import { Semana } from "../../features/rotina/Semana";
import { Treino } from "../../features/rotina/Treino";
import { hojeLocal, somarDias, ROTULOS_DAS_ABAS } from "../../features/rotina/dominio";
import { abrirConversa, lerDia, lerSemana, usarTransporteDaRotina, type TransporteDaRotina } from "../../features/rotina/rotinaService";
import type { Dia, SemanaDaRotina } from "../../features/rotina/tipos";
import "../../features/rotina/rotina.css";
import "./rotinaNoCelular.css";

const pelaRede: TransporteDaRotina = async (metodo, caminho, corpo) => {
  try {
    const valor = await request<unknown>(caminho, { method: metodo, ...(corpo === null ? {} : { headers: { "Content-Type": "application/json" }, body: corpo }) });
    return { status: 200, body: JSON.stringify(valor) };
  } catch (erro) {
    if (erro instanceof ApiError) return { status: erro.status, body: JSON.stringify({ error: erro.message }) };
    throw erro;
  }
};
// Dentro do app do computador o transporte continua o IPC (esta tela é do celular/web).
if (typeof window !== "undefined" && !("__TAURI_INTERNALS__" in window)) usarTransporteDaRotina(pelaRede);

const ABAS_DO_CELULAR = ["hoje", "semana", "treino", "alimentacao"] as const;
type AbaDoCelular = (typeof ABAS_DO_CELULAR)[number];

export function Rotina({ abrirConversaNoChat }: { abrirConversaNoChat: (id: string) => void }) {
  const [aba, setAba] = useState<AbaDoCelular>("hoje");
  const [dia, setDia] = useState<Dia | null>(null);
  const [semana, setSemana] = useState<SemanaDaRotina | null>(null);
  const [semanaVista, setSemanaVista] = useState(hojeLocal());
  const [ficha, setFicha] = useState("");
  const [erro, setErro] = useState<string | null>(null);
  const [abrindo, setAbrindo] = useState(false);

  const recarregar = useCallback(() => {
    setErro(null);
    if (aba === "hoje") void lerDia().then(setDia, (e: Error) => setErro(e.message));
    if (aba === "semana") void lerSemana(semanaVista).then(setSemana, (e: Error) => setErro(e.message));
  }, [aba, semanaVista]);
  useEffect(() => { recarregar(); }, [recarregar]);
  useEffect(() => {
    const t = window.setInterval(() => { if (!document.hidden) recarregar(); }, 60_000);
    return () => window.clearInterval(t);
  }, [recarregar]);

  async function conversar(modo: "planejar" | "conversar") {
    setAbrindo(true); setErro(null);
    try {
      const r = await abrirConversa(modo);
      if (r.conversa) abrirConversaNoChat(r.conversa); else setErro("A conversa da rotina ainda está abrindo no computador. Tente de novo.");
    } catch (e) { setErro(e instanceof Error ? e.message : "Não consegui abrir a conversa da rotina."); }
    finally { setAbrindo(false); }
  }

  return (
    <div className="rotina rotina-no-celular">
      <div className="rotina-principal">
        <header className="rotina-topo">
          <div className="rotina-linha-botoes">
            <Button size="sm" onClick={() => void conversar("planejar")} disabled={abrindo}>Planejar a semana</Button>
            <Button size="sm" variant="ghost" onClick={() => void conversar("conversar")} disabled={abrindo}>Conversar</Button>
          </div>
          <nav className="rotina-abas" role="tablist" aria-label="Partes da rotina">
            {ABAS_DO_CELULAR.map((a) => (
              <button key={a} type="button" role="tab" aria-selected={a === aba} className={`rotina-aba${a === aba ? " is-on" : ""}`}
                onClick={() => setAba(a)}>{ROTULOS_DAS_ABAS[a]}</button>
            ))}
          </nav>
        </header>
        {erro && <p className="rotina-erro" role="alert">{erro}</p>}
        <div className="rotina-conteudo" role="tabpanel" aria-label={ROTULOS_DAS_ABAS[aba]}>
          {aba === "hoje" && (dia ? <Hoje dia={dia} onMudou={recarregar} onTreino={() => setAba("treino")} onErro={setErro} />
            : !erro && <p className="rotina-vazio">Carregando o dia…</p>)}
          {aba === "semana" && (semana ? <Semana semana={semana} onMudou={recarregar} onErro={setErro}
            onIr={(dias) => setSemanaVista((s) => somarDias(s, dias))} /> : !erro && <p className="rotina-vazio">Carregando a semana…</p>)}
          {aba === "treino" && <Treino fichaEscolhida={ficha} onFicha={setFicha} onErro={setErro} />}
          {aba === "alimentacao" && <Alimentacao onErro={setErro} />}
        </div>
      </div>
    </div>
  );
}
