// Escolha do estilo da interface (Configurações → Aparência). Cada opção mostra uma prévia
// pequena desenhada com as classes `.estilo-previa*` de `src/styles/minimalista.css`, que não
// dependem do estilo ativo: as prévias ficam iguais em qualquer estilo (a do Vidro mora em `vidro.css`).
import type { EstiloDaInterface } from "../types/settings";
import { Card } from "./ui";

const OPCOES: { valor: EstiloDaInterface; nome: string; desc: string }[] = [
  { valor: "classico", nome: "Clássico", desc: "Pixel, bordas grossas e fundo animado." },
  { valor: "minimalista", nome: "Minimalista", desc: "Cantos arredondados, superfícies lisas e fonte do sistema." },
  // Terceira opção (Enzo, 21/10/2026): o Minimalista com fundo preto e vidro translúcido.
  { valor: "vidro", nome: "Vidro", desc: "Fundo preto, painéis de vidro translúcido e movimento suave." },
];

/** Três linhas de conversa (Claude, Codex, Antigravity) e um balão: o bastante para ver a diferença. */
function Previa({ estilo }: { estilo: EstiloDaInterface }) {
  return (
    <span className={`estilo-previa estilo-previa-${estilo}`} aria-hidden>
      <span className="estilo-previa-lateral">
        <span className="estilo-previa-linha" style={{ "--cor-agente": "#d97757" } as React.CSSProperties} />
        <span className="estilo-previa-linha" style={{ "--cor-agente": "#a78bfa" } as React.CSSProperties} />
        <span className="estilo-previa-linha" style={{ "--cor-agente": "#4285f4" } as React.CSSProperties} />
      </span>
      <span className="estilo-previa-conteudo">
        <span className="estilo-previa-balao is-voce" />
        <span className="estilo-previa-balao" />
      </span>
    </span>
  );
}

export function EscolhaDeEstilo({ valor, onChange }: { valor: EstiloDaInterface; onChange: (v: EstiloDaInterface) => void }) {
  return (
    <Card title="Estilo da interface" className="settings-largo">
      <div role="radiogroup" aria-label="Estilo da interface" className="estilo-opcoes">
        {OPCOES.map((o) => (
          <button key={o.valor} type="button" role="radio" aria-checked={valor === o.valor}
            className="estilo-opcao" onClick={() => onChange(o.valor)}>
            <Previa estilo={o.valor} />
            <span className="estilo-opcao-nome">{o.nome}</span>
            <span className="estilo-opcao-desc">{o.desc}</span>
          </button>
        ))}
      </div>
      <p className="text-text-muted text-[11px]">
        Muda só o visual: layout, atalhos e conversas continuam iguais. Vale na hora, sem reiniciar.
      </p>
    </Card>
  );
}
