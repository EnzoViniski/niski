// Fork (Rotina): a tela Rotina — Hoje, Semana, Treino, Alimentação e Ajustes, com "Planejar a semana"
// e "Conversar com a rotina" abrindo a conversa do agente da rotina ao lado.
import { useCallback, useEffect, useState } from "react";
import { Button } from "../../components/ui";
import type { AgentView } from "../../types/settings";
import { AjustesDaRotina } from "./AjustesDaRotina";
import { Alimentacao } from "./Alimentacao";
import { ConversaDaRotina } from "./ConversaDaRotina";
import { Hoje } from "./Hoje";
import { Semana } from "./Semana";
import { Treino } from "./Treino";
import { ouvirPedidosDaRotina, tirarPedidoPendente, type PedidoDaRotina } from "./abrirRotina";
import { ABAS, ROTULOS_DAS_ABAS, carregarEstadoDaTela, guardarEstadoDaTela, hojeLocal, padraoDoCopiloto, somarDias, type Aba, type EstadoDaTela } from "./dominio";
import { abrirConversa, conversaAtual, lerDia, lerSemana } from "./rotinaService";
import type { Dia, SemanaDaRotina } from "./tipos";
import "./rotina.css";

export function RotinaView({ agentView, onClose }: { agentView: AgentView; onClose: () => void }) {
  const [tela, setTela] = useState<EstadoDaTela>(carregarEstadoDaTela);
  const [dia, setDia] = useState<Dia | null>(null);
  const [semana, setSemana] = useState<SemanaDaRotina | null>(null);
  const [semanaVista, setSemanaVista] = useState(hojeLocal());
  const [erro, setErro] = useState<string | null>(null);
  const [sessao, setSessao] = useState<string | null>(null);
  const [abrindo, setAbrindo] = useState(false);
  const [erroDaConversa, setErroDaConversa] = useState<string | null>(null);

  const mudarTela = useCallback((m: Partial<EstadoDaTela>) => setTela((t) => { const n = { ...t, ...m }; guardarEstadoDaTela(n); return n; }), []);
  const recarregar = useCallback(() => {
    setErro(null);
    if (tela.aba === "hoje") void lerDia().then(setDia, (e: Error) => setErro(e.message));
    if (tela.aba === "semana") void lerSemana(semanaVista).then(setSemana, (e: Error) => setErro(e.message));
  }, [tela.aba, semanaVista]);
  useEffect(() => { recarregar(); }, [recarregar]);
  // Volta a ler a cada minuto (o agente e o Telegram também mudam a rotina) e ao voltar para a janela.
  useEffect(() => {
    const t = window.setInterval(() => { if (!document.hidden) recarregar(); }, 60_000);
    const foco = () => recarregar();
    window.addEventListener("focus", foco);
    return () => { window.clearInterval(t); window.removeEventListener("focus", foco); };
  }, [recarregar]);
  // A conversa da semana que já existir (sem abrir nada).
  useEffect(() => { void conversaAtual().then((c) => setSessao(c.sessao)).catch(() => undefined); }, []);

  const abrir = useCallback(async (modo: "planejar" | "conversar") => {
    mudarTela({ conversaAberta: true });
    setAbrindo(true); setErroDaConversa(null);
    try {
      const r = await abrirConversa(modo, padraoDoCopiloto());
      setSessao(r.sessao);
    } catch (e) { setErroDaConversa(e instanceof Error ? e.message : "Não consegui abrir a conversa da rotina."); }
    finally { setAbrindo(false); }
  }, [mudarTela]);

  const atender = useCallback((p: PedidoDaRotina) => {
    if (p.aba) mudarTela({ aba: p.aba });
    if (p.conversa) void abrir(p.conversa);
  }, [abrir, mudarTela]);
  useEffect(() => {
    const pendente = tirarPedidoPendente();
    if (pendente) atender(pendente);
    return ouvirPedidosDaRotina(atender);
  }, [atender]);

  const aba = tela.aba;
  return (
    <div className="rotina">
      <div className="rotina-principal">
        <header className="rotina-topo">
          <div className="rotina-linha-botoes">
            <h1 className="rotina-titulo">Rotina</h1>
            <span className="flex-1" />
            <Button size="sm" onClick={() => void abrir("planejar")} disabled={abrindo}>Planejar a semana</Button>
            <Button size="sm" variant="ghost" onClick={() => void abrir("conversar")} disabled={abrindo}>Conversar com a rotina</Button>
            <button type="button" className="rotina-mini" onClick={onClose} aria-label="Fechar a Rotina">×</button>
          </div>
          <nav className="rotina-abas" role="tablist" aria-label="Partes da rotina">
            {ABAS.map((a) => (
              <button key={a} type="button" role="tab" aria-selected={a === aba} className={`rotina-aba${a === aba ? " is-on" : ""}`}
                onClick={() => mudarTela({ aba: a as Aba })}>{ROTULOS_DAS_ABAS[a]}</button>
            ))}
          </nav>
        </header>
        {erro && <p className="rotina-erro" role="alert">{erro}</p>}
        <div className="rotina-conteudo" role="tabpanel" aria-label={ROTULOS_DAS_ABAS[aba]}>
          {aba === "hoje" && (dia ? <Hoje dia={dia} onMudou={recarregar} onTreino={() => mudarTela({ aba: "treino" })} onErro={setErro} />
            : !erro && <p className="rotina-vazio">Carregando o dia…</p>)}
          {aba === "semana" && (semana ? <Semana semana={semana} onMudou={recarregar} onErro={setErro}
            onIr={(dias) => setSemanaVista((s) => somarDias(s, dias))} /> : !erro && <p className="rotina-vazio">Carregando a semana…</p>)}
          {aba === "treino" && <Treino fichaEscolhida={tela.ficha} onFicha={(ficha) => mudarTela({ ficha })} onErro={setErro} />}
          {aba === "alimentacao" && <Alimentacao onErro={setErro} />}
          {aba === "ajustes" && <AjustesDaRotina onErro={setErro} />}
        </div>
      </div>
      {tela.conversaAberta && (
        <ConversaDaRotina sessao={sessao} agentView={agentView} abrindo={abrindo} erro={erroDaConversa}
          onFechar={() => { mudarTela({ conversaAberta: false }); recarregar(); }} />
      )}
    </div>
  );
}
