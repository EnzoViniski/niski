// Fork (Sobre mim): estado da tela com salvamento automático por área. O texto digitado fica num
// rascunho até o engine confirmar; a resposta de um salvamento nunca apaga o que foi digitado depois.
import { useCallback, useEffect, useRef, useState } from "react";
import { ROTAS, type AjustesDoSobreMim, type ChamarSobreMim, type EstadoDoSobreMim, type PropostaDoAgente } from "./apiDoSobreMim";

export type SituacaoDaArea = { fase: "salvando" } | { fase: "salvo" } | { fase: "erro"; mensagem: string };

function mensagemDe(motivo: unknown): string {
  return motivo instanceof Error ? motivo.message : String(motivo);
}

export function useSobreMim(chamar: ChamarSobreMim, atrasoMs = 900) {
  const [estado, setEstado] = useState<EstadoDoSobreMim | null>(null);
  const [rascunhos, setRascunhos] = useState<Record<string, string>>({});
  const [situacao, setSituacao] = useState<Record<string, SituacaoDaArea>>({});
  const [erro, setErro] = useState<string | null>(null);
  const [aviso, setAviso] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const rascunhosRef = useRef(rascunhos);
  rascunhosRef.current = rascunhos;
  const timers = useRef(new Map<string, ReturnType<typeof setTimeout>>());
  const chamarRef = useRef(chamar);
  chamarRef.current = chamar;

  const recarregar = useCallback(async () => {
    try {
      setEstado(await chamarRef.current("GET", ROTAS.ler));
      setErro(null);
    } catch (motivo) { setErro(mensagemDe(motivo)); }
  }, []);

  const salvar = useCallback(async (id: string) => {
    const timer = timers.current.get(id);
    if (timer) { clearTimeout(timer); timers.current.delete(id); }
    const texto = rascunhosRef.current[id];
    if (texto === undefined) return;
    setSituacao((atual) => ({ ...atual, [id]: { fase: "salvando" } }));
    try {
      const novo = await chamarRef.current("POST", ROTAS.area(id), { texto });
      setEstado(novo);
      // Só some o rascunho que foi de fato gravado; o que foi digitado durante o envio continua.
      setRascunhos((atual) => {
        if (atual[id] !== texto) return atual;
        const copia = { ...atual };
        delete copia[id];
        return copia;
      });
      setSituacao((atual) => ({ ...atual, [id]: { fase: "salvo" } }));
    } catch (motivo) {
      setSituacao((atual) => ({ ...atual, [id]: { fase: "erro", mensagem: mensagemDe(motivo) } }));
    }
  }, []);

  const editar = useCallback((id: string, texto: string) => {
    setRascunhos((atual) => ({ ...atual, [id]: texto }));
    rascunhosRef.current = { ...rascunhosRef.current, [id]: texto };
    const anterior = timers.current.get(id);
    if (anterior) clearTimeout(anterior);
    timers.current.set(id, setTimeout(() => void salvar(id), atrasoMs));
  }, [atrasoMs, salvar]);

  useEffect(() => {
    void recarregar();
    // Voltar para a janela traz as sugestões que os agentes anotaram nesse meio-tempo.
    const aoFocar = () => { void recarregar(); };
    window.addEventListener("focus", aoFocar);
    const pendentes = timers.current;
    return () => {
      window.removeEventListener("focus", aoFocar);
      // Sair da tela não perde o que foi digitado: grava o que estava esperando o atraso.
      for (const [id, timer] of pendentes) { clearTimeout(timer); void salvar(id); }
      pendentes.clear();
    };
  }, [recarregar, salvar]);

  async function acao(executar: () => Promise<EstadoDoSobreMim>, sucesso: string) {
    setOcupado(true); setAviso(null);
    try {
      setEstado(await executar());
      setAviso(sucesso);
      setErro(null);
    } catch (motivo) {
      setAviso(null);
      setErro(mensagemDe(motivo));
      void recarregar().then(() => setErro(mensagemDe(motivo)));
    } finally { setOcupado(false); }
  }

  async function aceitar(proposta: PropostaDoAgente, area: string, texto: string) {
    // Um rascunho esperando na área de destino gravaria por cima do item aceito.
    if (rascunhosRef.current[area] !== undefined) await salvar(area);
    await acao(() => chamarRef.current("POST", ROTAS.aceitar, { linha: proposta.linha, area, texto }), "Sugestão aceita.");
  }

  async function descartar(proposta: PropostaDoAgente) {
    await acao(() => chamarRef.current("POST", ROTAS.descartar, { linha: proposta.linha }), "Sugestão descartada.");
  }

  async function mudarAjustes(parcial: Partial<AjustesDoSobreMim>) {
    if (!estado) return;
    await acao(() => chamarRef.current("POST", ROTAS.ajustes, { ...estado.ajustes, ...parcial }), "Ajuste salvo. Vale para os agentes que você abrir agora.");
  }

  const textoDa = (id: string) => rascunhos[id] ?? estado?.areas.find((area) => area.id === id)?.texto ?? "";

  return { estado, erro, aviso, setAviso, ocupado, situacao, textoDa, editar, salvar, aceitar, descartar, mudarAjustes, recarregar };
}
