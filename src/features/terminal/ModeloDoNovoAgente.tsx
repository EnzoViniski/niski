// Modelo/esforço também podem ser escolhidos antes de abrir a conversa.
import { useEffect, useId, useState } from "react";
import { Select } from "../../components/ui";
import { ajustesDoAgente, type AjustesSalvos } from "../../lib/escolhasSalvas";
import { listarModelos, ROTULO_ESFORCO, type ModeloDaCli } from "../agentes/agentesService";

export function useModeloDoNovoAgente(provider: string) {
  const [catalogo, setCatalogo] = useState<Record<string, ModeloDaCli[]>>({});
  const [selecionado, setSelecionado] = useState<{ provider: string; ajustes: AjustesSalvos }>(() => ({
    provider, ajustes: ajustesDoAgente(provider) ?? { model: null, effort: null },
  }));
  useEffect(() => {
    let vivo = true;
    void listarModelos().then((modelos) => { if (vivo && modelos) setCatalogo(modelos); }).catch(() => undefined);
    return () => { vivo = false; };
  }, []);
  const ajustes = selecionado.provider === provider ? selecionado.ajustes
    : ajustesDoAgente(provider) ?? { model: null, effort: null };
  return {
    ajustes,
    campo: <ModeloDoNovoAgente modelos={Array.isArray(catalogo[provider]) ? catalogo[provider] : []} ajustes={ajustes}
      onMudar={(proximos) => setSelecionado({ provider, ajustes: proximos })} />,
  };
}

function ModeloDoNovoAgente({ modelos, ajustes, onMudar }: {
  modelos: ModeloDaCli[];
  ajustes: AjustesSalvos;
  onMudar: (ajustes: AjustesSalvos) => void;
}) {
  const id = useId();
  const esforcos = modelos.find((modelo) => modelo.id === ajustes.model)?.esforcos ?? [];
  const atualForaDaLista = ajustes.model && !modelos.some((modelo) => modelo.id === ajustes.model);
  return <div className="launcher-campo">
    <label htmlFor={`${id}-modelo`} className="launcher-rotulo">Modelo</label>
    <Select id={`${id}-modelo`} value={ajustes.model ?? ""} options={[
      { value: "", label: "Padrão da CLI" },
      ...(atualForaDaLista ? [{ value: ajustes.model!, label: `${ajustes.model} (salvo)` }] : []),
      ...modelos.map((modelo) => ({ value: modelo.id, label: modelo.label })),
    ]} onChange={(model) => onMudar({ model: model || null, effort: null })} />
    {esforcos.length > 0 && <>
      <label htmlFor={`${id}-esforco`} className="launcher-rotulo mt-3">Esforço</label>
      <Select id={`${id}-esforco`} value={ajustes.effort ?? ""} options={[
        { value: "", label: "Padrão da CLI" },
        ...esforcos.map((effort) => ({ value: effort, label: ROTULO_ESFORCO[effort] ?? effort })),
      ]} onChange={(effort) => onMudar({ ...ajustes, effort: effort || null })} />
    </>}
    {modelos.length === 0 && <p className="launcher-nota">Sem catálogo disponível. A CLI usa o modelo configurado nela.</p>}
  </div>;
}
