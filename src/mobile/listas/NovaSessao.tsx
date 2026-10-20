// Fork (Onda 2.0): saiu do `MobileApp.tsx` sem mudança de lógica.
import { useRef, useState } from "react";
import { Button } from "../../components/ui/Button";
import { Select } from "../../components/ui/Select";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { ChatIcon } from "../../components/ui/PixelIcon";
import { request, actionKey, ApiError, type Projects } from "../api";
import { NOME_CURTO } from "../../lib/identidade";

export function NovaSessao({ project, catalog, onCriada, classeBotao = "w-full" }: {
  project: { id: string; name: string }; catalog: Projects; onCriada: () => Promise<void>;
  /** Classe do botão fechado: no celular ocupa a largura toda; na lateral da web divide a linha. */
  classeBotao?: string;
}) {
  const [aberto, setAberto] = useState(false);
  const [provider, setProvider] = useState("");
  const [profileId, setProfileId] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const chave = useRef<string | null>(null);

  const contas = catalog.profiles.filter(p => p.provider === provider);
  if (catalog.agents.length === 0) {
    return <p className="m-dica">Abra o {NOME_CURTO} no PC uma vez para liberar a criação de sessões daqui.</p>;
  }

  async function criar() {
    if (!provider || busy) return;
    // A chave sobrevive ao retry: um timeout tem resultado desconhecido, e sem isso um segundo
    // toque abriria uma PTY a mais no PC.
    chave.current ??= actionKey();
    setBusy(true); setError(null);
    try {
      await request("/sessoes", {
        method: "POST",
        headers: { "Content-Type": "application/json", "Idempotency-Key": chave.current },
        body: JSON.stringify({ project_id: project.id, provider, profile_id: profileId || null }),
      });
      chave.current = null; setAberto(false); setProvider(""); setProfileId("");
      await onCriada();
    } catch (reason) {
      if (reason instanceof ApiError && reason.status >= 400 && reason.status < 500) chave.current = null;
      setError(String(reason));
    } finally { setBusy(false); }
  }

  if (!aberto) return <Button variant="primary" className={classeBotao} onClick={() => setAberto(true)}>
    <ChatIcon aria-hidden className="h-4 w-4" />Nova sessão
  </Button>;
  return <div className="glass m-painel">
    <h3 className="m-secao-titulo">Nova sessão em {project.name}</h3>
    <label htmlFor="agente" className="m-rotulo">Agente</label>
    <Select id="agente" value={provider} placeholder="Escolha o agente…"
      options={catalog.agents.map(a => ({ value: a.id, label: a.label, icon: <AgentIcon provider={a.id} size={14} className="shrink-0" /> }))}
      onChange={value => { setProvider(value); setProfileId(""); }} />
    {contas.length > 1 && <>
      <label htmlFor="conta" className="m-rotulo">Conta</label>
      <Select id="conta" value={profileId}
        options={[{ value: "", label: "Padrão" }, ...contas.map(p => ({ value: p.id, label: p.name }))]}
        onChange={setProfileId} />
    </>}
    <div className="m-acoes">
      <Button variant="primary" disabled={busy || !provider} onClick={() => void criar()}>{busy ? "Abrindo…" : "Abrir"}</Button>
      <Button variant="ghost" disabled={busy} onClick={() => setAberto(false)}>Cancelar</Button>
    </div>
    {error && <p role="alert" className="m-alerta">{error}</p>}
  </div>;
}
