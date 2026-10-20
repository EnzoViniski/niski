import type { AppSettings } from "../../types/settings";
import { preferenciasDeAvisos, ROTULOS_DA_ATIVIDADE, TIPOS_DE_ATIVIDADE } from "./tipos";

export function ConfiguracaoDeAvisos({ settings, onChange }: { settings: AppSettings; onChange: (updates: Partial<AppSettings>) => void }) {
  const salvas = preferenciasDeAvisos(settings.avisos);
  const preferencias = settings.notifyAttention ? salvas : Object.fromEntries(
    Object.entries(salvas).map(([tipo, valor]) => [tipo, { ...valor, quando: "nunca" as const }]),
  ) as typeof salvas;
  return <div className="space-y-3">
    <p className="text-xs text-text-muted">Avisos do sistema por tipo. Com a janela em foco, Sempre mostra um aviso no app e toca o som escolhido.</p>
    {TIPOS_DE_ATIVIDADE.map(tipo => <div key={tipo} className="flex flex-wrap items-center gap-2">
      <label className="min-w-36 flex-1 text-xs" htmlFor={`quando-${tipo}`}>{ROTULOS_DA_ATIVIDADE[tipo]}</label>
      <select id={`quando-${tipo}`} className="rounded border border-border-subtle bg-bg-elevated px-2 py-1 text-xs"
        value={settings.notifyAttention ? preferencias[tipo].quando : "nunca"}
        onChange={evento => onChange({ notifyAttention: true, avisos: { ...preferencias, [tipo]: { ...preferencias[tipo], quando: evento.target.value } } })}>
        <option value="sempre">Sempre</option><option value="fora_de_foco">Fora de foco</option><option value="nunca">Desligado</option>
      </select>
      <select aria-label={`Som: ${ROTULOS_DA_ATIVIDADE[tipo]}`} className="rounded border border-border-subtle bg-bg-elevated px-2 py-1 text-xs"
        value={preferencias[tipo].som ?? ""}
        onChange={evento => onChange({ avisos: { ...preferencias, [tipo]: { ...preferencias[tipo], som: evento.target.value || null } } })}>
        <option value="">Sem som</option><option>Glass</option><option>Ping</option><option>Basso</option>
      </select>
    </div>)}
  </div>;
}
