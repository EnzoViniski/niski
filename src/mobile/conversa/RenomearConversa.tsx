// Fork (Onda 2.0): saiu do `MobileApp.tsx` sem mudança de lógica.
import { Button } from "../../components/ui/Button";

export function RenomearConversa({ nome, onSalvar, onFechar }: { nome: string; onSalvar: (nome: string) => void; onFechar: () => void }) {
  return <form className="m-barra-conversa" onSubmit={(evento) => {
    evento.preventDefault();
    const campo = evento.currentTarget.elements.namedItem("nome") as HTMLInputElement | null;
    if (campo) onSalvar(campo.value);
  }}>
    <input name="nome" defaultValue={nome} aria-label="Nome da conversa" className="m-campo-nome"
      autoFocus onKeyDown={(evento) => { if (evento.key === "Escape") onFechar(); }} />
    <Button size="sm" type="submit">Salvar</Button>
    <Button size="sm" variant="ghost" type="button" onClick={onFechar}>Cancelar</Button>
  </form>;
}
