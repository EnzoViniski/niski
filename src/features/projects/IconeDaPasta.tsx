// Fork: ícone da pasta de projeto na lateral. No Clássico, a pasta pixel de sempre; no Minimalista,
// traço fino com a pasta aberta quando ela está aberta (como nos apps do Claude/Codex). Traços do
// Lucide (ISC), `folder` e `folder-open`.
import { FolderIcon } from "../../components/ui/PixelIcon";

const FECHADA = "M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z";
const ABERTA = "m6 14 1.5-2.9A2 2 0 0 1 9.24 10H20a2 2 0 0 1 1.94 2.5l-1.54 6a2 2 0 0 1-1.95 1.5H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.9a2 2 0 0 1 1.69.9l.81 1.2a2 2 0 0 0 1.67.9H18a2 2 0 0 1 2 2v2";

export function IconeDaPasta({ aberta }: { aberta: boolean }) {
  return (
    <span className="pasta-icone" aria-hidden>
      <FolderIcon className="pasta-icone-classico h-3.5 w-3.5 shrink-0 text-accent" />
      <svg className="pasta-icone-minimalista" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor"
        strokeWidth={1.75} strokeLinecap="round" strokeLinejoin="round">
        <path d={aberta ? ABERTA : FECHADA} />
      </svg>
    </span>
  );
}
