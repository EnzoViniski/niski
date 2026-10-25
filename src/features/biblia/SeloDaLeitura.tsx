// Fork (Bíblia do dia): ponto ao lado de "Palavra do dia" na lateral enquanto a leitura de hoje não foi feita.
import { useEffect, useState } from "react";
import { pendenteHoje, useLeitura } from "./leitura";

export function SeloDaLeitura() {
  const g = useLeitura();
  // Vira o dia com o app aberto: confere de novo a cada minuto.
  const [agora, setAgora] = useState(() => new Date());
  useEffect(() => { const t = setInterval(() => setAgora(new Date()), 60_000); return () => clearInterval(t); }, []);
  if (!pendenteHoje(g, agora)) return null;
  return <span className="ml-auto h-2 w-2 rounded-full bg-accent" role="img" aria-label="Leitura de hoje ainda não feita" />;
}
