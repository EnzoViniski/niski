// Configuração de /goal nativo e repetição controlada pelo executor.
import { Field, Input, Select, Textarea } from "../../components/ui";
import type { AgenteSalvo } from "../agentes/agentesService";
import type { NoDoFluxo } from "./fluxosService";

export function BlocosExtras({ no, agentes, onMudar }: { no: NoDoFluxo; agentes: AgenteSalvo[]; onMudar: (n: NoDoFluxo) => void }) {
  const cfg = (parte: Record<string, unknown>) => onMudar({ ...no, config: { ...no.config, ...parte } });
  if (no.tipo === "objetivo") return <>
    <Field label="Agente do objetivo" htmlFor="fx-goal-agente"><Select id="fx-goal-agente" value={no.agente ?? ""} onChange={agente => onMudar({ ...no, agente })} options={[{value:"",label:"Escolha…"},...agentes.filter(a => a.cli === "claude").map(a => ({value:a.name,label:a.name}))]} /></Field>
    <Field label="Condição de conclusão" htmlFor="fx-goal"><Textarea id="fx-goal" rows={4} maxLength={3800} value={String(no.config.objetivo ?? "")} onChange={e => cfg({ objetivo:e.target.value.replace(/[\r\n]/g," ") })} /></Field>
    <Field label="Teto de turnos" htmlFor="fx-goal-turnos"><Input id="fx-goal-turnos" type="number" min={1} max={100} value={Number(no.config.max_turnos ?? 10)} onChange={e => cfg({ max_turnos:Math.max(1,Math.min(100,Number(e.target.value) || 1)) })} /></Field>
    <p className="fx-dica">Usa /goal oficial do Claude até a condição ou o teto de turnos. Requer ganchos habilitados e confiança na pasta. Codex: /goal em exec ainda não documentado.</p>
  </>;
  if (no.tipo !== "repetir") return null;
  return <>
    <Field label="Máximo de repetições" htmlFor="fx-repetir"><Input id="fx-repetir" type="number" min={1} max={1000} value={Number(no.config.vezes ?? 3)} onChange={e => cfg({ vezes:Math.max(1,Math.min(1000,Number(e.target.value) || 1)) })} /></Field>
    <Field label="Intervalo em minutos" htmlFor="fx-intervalo"><Input id="fx-intervalo" type="number" min={0} max={1440} value={Number(no.config.intervalo_minutos ?? 0)} onChange={e => cfg({ intervalo_minutos:Math.max(0,Math.min(1440,Number(e.target.value) || 0)) })} /></Field>
    <label className="fx-dica"><input type="checkbox" checked={Boolean(no.config.parar_quando_aprovado)} onChange={e => cfg({ parar_quando_aprovado:e.target.checked })} /> Parar quando o ramo devolver aprovado</label>
    <p className="fx-dica">Ligue corpo ao ramo e o final do ramo de volta a Repetir. Ligue fim à continuação. Zero minutos executa N vezes seguidas; intervalo conta após o ramo terminar. Pausar e cancelar interrompem a espera. Exige engine vivo e computador acordado.</p>
  </>;
}
