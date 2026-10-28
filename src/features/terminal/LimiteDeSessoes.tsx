import { invoke } from "@tauri-apps/api/core";
import { useEffect, useState } from "react";
import { Button, Card, Field, Input } from "../../components/ui";

/** Teto de sessões ativas neste PC (engine relê `sessoes.json` a cada conferência). */
export interface LimiteDeSessoesInfo {
  limite: number;
  padrao: number;
  minimo: number;
  maximo: number;
}

export function lerLimiteDeSessoes(): Promise<LimiteDeSessoesInfo> {
  return invoke<LimiteDeSessoesInfo>("limite_de_sessoes");
}

export function definirLimiteDeSessoes(valor: number): Promise<LimiteDeSessoesInfo> {
  return invoke<LimiteDeSessoesInfo>("definir_limite_de_sessoes", { valor });
}

/** Cartão de Configurações → Geral. */
export function LimiteDeSessoes() {
  const [info, setInfo] = useState<LimiteDeSessoesInfo | null>(null);
  const [texto, setTexto] = useState("");
  const [mensagem, setMensagem] = useState<{ ok: boolean; texto: string } | null>(null);
  const [salvando, setSalvando] = useState(false);

  useEffect(() => {
    let vivo = true;
    lerLimiteDeSessoes()
      .then((lido) => { if (vivo) { setInfo(lido); setTexto(String(lido.limite)); } })
      .catch(() => undefined);
    return () => { vivo = false; };
  }, []);

  const valor = Number(texto);
  const valido = info !== null && Number.isInteger(valor) && valor >= info.minimo && valor <= info.maximo;

  async function salvar() {
    if (!valido) return;
    setSalvando(true);
    setMensagem(null);
    try {
      const novo = await definirLimiteDeSessoes(valor);
      setInfo(novo);
      setTexto(String(novo.limite));
      setMensagem({ ok: true, texto: `Limite salvo: ${novo.limite} sessões. Vale na hora.` });
    } catch (reason) {
      setMensagem({ ok: false, texto: reason instanceof Error ? reason.message : String(reason) });
    } finally {
      setSalvando(false);
    }
  }

  return (
    <Card title="Sessões">
      <Field
        label="Limite de sessões ativas neste PC"
        htmlFor="limite-de-sessoes"
        description={`Contam agentes e terminais rodando um comando; terminal parado no prompt não conta. Vale para o celular, retomar, bifurcar e os subagentes do orquestrador. Padrão ${info?.padrao ?? 32}${info ? `, de ${info.minimo} a ${info.maximo}` : ""}.`}
      >
        <div className="flex items-center gap-2">
          <Input
            id="limite-de-sessoes"
            size="sm"
            type="number"
            inputMode="numeric"
            min={info?.minimo}
            max={info?.maximo}
            value={texto}
            disabled={!info}
            onChange={(event) => { setTexto(event.target.value); setMensagem(null); }}
            onKeyDown={(event) => { if (event.key === "Enter") void salvar(); }}
            className="w-24"
          />
          <Button size="sm" variant="ghost" loading={salvando} disabled={!valido || String(info?.limite) === texto} onClick={() => void salvar()}>
            Salvar
          </Button>
        </div>
      </Field>
      {mensagem && (
        <p role="status" className={`border-l-2 pl-3 text-xs ${mensagem.ok ? "border-success text-success" : "border-danger text-danger"}`}>
          {mensagem.texto}
        </p>
      )}
    </Card>
  );
}
