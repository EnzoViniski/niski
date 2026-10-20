// Fork (Onda 2.0): saiu do `MobileApp.tsx` sem mudança.
import type { ReactNode } from "react";

export function Vazio({ children }: { children: ReactNode }) {
  return <p className="m-vazio">{children}</p>;
}

export function Secao({ titulo, children, acao }: { titulo: string; children: ReactNode; acao?: ReactNode }) {
  return (
    <section className="m-secao">
      <div className="m-secao-cabecalho">
        <h2 className="m-secao-titulo">{titulo}</h2>
        {acao}
      </div>
      {children}
    </section>
  );
}
