// Fork (Editor de vídeo): qual IA edita o vídeo (CLI, conta e modelo). Mesmo catálogo de modelos da
// criação de agentes (`agentes_modelos`) e as contas de Configurações → Contas.
import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Button, SegmentedControl, Select } from "../../components/ui";
import { AgentIcon } from "../../components/ui/AgentIcon";
import { listProfiles, type Profile } from "../terminal/terminalService";
import type { CliDoVideo, IaDoVideo } from "./tipos";

interface ModeloDaCli { id: string; label: string }

let catalogo: Promise<Record<string, ModeloDaCli[]>> | null = null;
function lerCatalogo() {
  catalogo ??= invoke<Record<string, ModeloDaCli[]>>("agentes_modelos").then((m) => m ?? {}).catch(() => { catalogo = null; return {}; });
  return catalogo;
}

function useContasEModelos() {
  const [contas, setContas] = useState<Profile[]>([]);
  const [modelos, setModelos] = useState<Record<string, ModeloDaCli[]>>({});
  useEffect(() => {
    let vivo = true;
    void listProfiles().then((p) => { if (vivo) setContas(p); }).catch(() => undefined);
    void lerCatalogo().then((m) => { if (vivo) setModelos(m); });
    return () => { vivo = false; };
  }, []);
  return { contas, modelos };
}

export function nomeDaCli(cli: CliDoVideo) { return cli === "codex" ? "Codex" : "Claude"; }

export function EscolhaDaIa({ valor, onChange }: { valor: IaDoVideo; onChange: (ia: IaDoVideo) => void }) {
  const { contas, modelos } = useContasEModelos();
  const daCli = contas.filter((c) => c.provider === valor.cli);
  const semLogin = daCli.length > 0 && !daCli.some((c) => c.authenticated);
  return (
    <div className="space-y-2">
      <SegmentedControl<CliDoVideo> label="IA que edita" value={valor.cli} onChange={(cli) => onChange({ cli })}
        options={[{ value: "claude", label: "Claude", icon: <AgentIcon provider="claude" size={14} /> }, { value: "codex", label: "Codex", icon: <AgentIcon provider="codex" size={14} /> }]} />
      <div className="grid grid-cols-2 gap-2">
        <Select<string> value={valor.conta ?? ""} onChange={(conta) => onChange({ ...valor, conta: conta || undefined })} size="sm"
          options={[{ value: "", label: "Primeira conta com login" }, ...daCli.map((c) => ({ value: c.id, label: c.name, disabled: !c.authenticated, hint: c.authenticated ? undefined : "sem login" }))]} />
        <Select<string> value={valor.modelo ?? ""} onChange={(modelo) => onChange({ ...valor, modelo: modelo || undefined })} size="sm"
          options={[{ value: "", label: "Modelo padrão da conta" }, ...(modelos[valor.cli] ?? []).map((m) => ({ value: m.id, label: m.label }))]} />
      </div>
      {semLogin && <p className="text-xs text-warning">Nenhuma conta do {nomeDaCli(valor.cli)} com login. Entre numa conta em Configurações → Contas.</p>}
      <p className="text-xs text-text-muted">Usa a CLI oficial na sua conta. Ela só edita os arquivos deste projeto.</p>
    </div>
  );
}

/** "Claude · Sonnet" no topo do chat: mostra o modelo que está rodando e troca ao clicar. */
export function ChipDaIa({ ia, modeloRodando, onTrocar, desligado }: { ia: IaDoVideo; modeloRodando?: string; onTrocar: (ia: IaDoVideo) => void; desligado?: boolean }) {
  const [aberto, setAberto] = useState(false);
  const [rascunho, setRascunho] = useState<IaDoVideo>(ia);
  const { contas, modelos } = useContasEModelos();
  const escolhido = ia.modelo ? (modelos[ia.cli]?.find((m) => m.id === ia.modelo)?.label ?? ia.modelo) : null;
  const nomeDoModelo = escolhido ?? (modeloRodando ? (modelos[ia.cli]?.find((m) => m.id === modeloRodando)?.label ?? modeloRodando) : "modelo padrão");
  const conta = ia.conta ? contas.find((c) => c.id === ia.conta)?.name : undefined;
  return (
    <div className="video-chip-ia">
      <button type="button" className="video-chip" aria-expanded={aberto} disabled={desligado}
        onClick={() => { setRascunho(ia); setAberto(!aberto); }}
        aria-label={`IA do vídeo: ${nomeDaCli(ia.cli)}, ${nomeDoModelo}. Trocar`} title={conta ? `Conta: ${conta}` : "Primeira conta com login"}>
        <AgentIcon provider={ia.cli} size={14} />
        <span>{nomeDaCli(ia.cli)} · {nomeDoModelo}</span>
        <span aria-hidden="true">▾</span>
      </button>
      {aberto && (
        <div className="video-chip-painel glass" role="dialog" aria-label="Trocar a IA do vídeo" onKeyDown={(e) => { if (e.key === "Escape") setAberto(false); e.stopPropagation(); }}>
          <EscolhaDaIa valor={rascunho} onChange={setRascunho} />
          <div className="flex justify-end gap-2">
            <Button size="sm" variant="ghost" onClick={() => setAberto(false)}>Cancelar</Button>
            <Button size="sm" variant="primary" onClick={() => { onTrocar(rascunho); setAberto(false); }}>Usar esta IA</Button>
          </div>
        </div>
      )}
    </div>
  );
}
