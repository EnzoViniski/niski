import { useCallback, useEffect, useState } from "react";
import { useAcoesDaPaleta } from "../paleta/acoes";
import { TelaDeAgendamentos } from "./TelaDeAgendamentos";
export function CentralDeAgendamentos() {
  const [aberta, setAberta] = useState(false);
  const fechar = useCallback(() => setAberta(false), []);
  useAcoesDaPaleta("agendamentos", [{ id: "abrir-agendamentos", titulo: "Abrir Agendamentos", executar: () => setAberta(true) }]);
  useEffect(() => { const abrir = () => setAberta(true); window.addEventListener("medza-abrir-agendamentos", abrir); return () => window.removeEventListener("medza-abrir-agendamentos", abrir); }, []);
  return <><button type="button" className="titlebar-acao" onClick={() => setAberta(true)}>Agendamentos</button>{aberta && <TelaDeAgendamentos onFechar={fechar} />}</>;
}
