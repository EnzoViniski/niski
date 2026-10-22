// Fork (Tarefas): seção "Tarefas" das Configurações — quem sou eu e a pasta compartilhada.
import { useState } from "react";
import { Button, Card, Input } from "../../components/ui";
import { Avatar } from "./CartaoDaTarefa";
import { compartilharEm, pessoasConhecidas, salvarAjustes, useTarefas } from "./loja";
import { CORES_DE_PESSOA } from "./tipos";

async function escolherPasta(): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const escolhida = await open({ directory: true, multiple: false, title: "Pasta compartilhada das tarefas" });
  return typeof escolhida === "string" ? escolhida : null;
}

export function AjustesDasTarefas() {
  const e = useTarefas();
  const { eu, pastaCompartilhada, ocultarFeitasAntigas } = e.ajustes;
  const [nome, setNome] = useState(eu.nome);
  const [aviso, setAviso] = useState("");
  const [erro, setErro] = useState("");
  const outras = pessoasConhecidas(e).filter((p) => p.id !== eu.id);

  const compartilhar = async () => {
    setErro(""); setAviso("");
    try {
      const pasta = await escolherPasta();
      if (!pasta) return;
      const copiadas = await compartilharEm(pasta);
      setAviso(copiadas ? `${copiadas} tarefa(s) deste computador foram para a pasta.` : "Pasta ligada. As tarefas de quem já usa a pasta aparecem em instantes.");
    } catch (err) {
      setErro(err instanceof Error ? err.message : String(err));
    }
  };

  return (
    <div className="space-y-4">
      <Card title="Você">
        <p className="text-xs text-text-muted">É assim que você aparece nas tarefas e para quem divide a pasta com você.</p>
        <div className="flex flex-wrap items-center gap-2">
          <Avatar pessoa={{ ...eu, nome: nome || "?" }} grande />
          <Input value={nome} onChange={(ev) => setNome(ev.target.value)} placeholder="Seu nome" className="max-w-xs" aria-label="Seu nome"
            onBlur={() => nome.trim() && nome.trim() !== eu.nome && salvarAjustes({ eu: { ...eu, nome: nome.trim() } })} />
          {CORES_DE_PESSOA.map((c) => (
            <button key={c} type="button" aria-label={`Cor ${c}`} aria-pressed={eu.cor === c} className="tarefa-cor"
              style={{ background: c, outlineColor: eu.cor === c ? c : "transparent" }}
              onClick={() => salvarAjustes({ eu: { ...eu, nome: nome.trim() || eu.nome || "Eu", cor: c } })} />
          ))}
        </div>
      </Card>

      <Card title="Dividir com outra pessoa">
        <p className="text-sm text-text-secondary">
          Escolha uma pasta que vocês dois sincronizam (Google Drive, iCloud Drive, Dropbox, OneDrive…). O Niski cria
          <code className="mx-1">niski-tarefas/</code>lá dentro, um arquivo por tarefa. A outra pessoa escolhe a mesma pasta no Niski dela e
          vocês passam a ver, criar e atribuir tarefas um para o outro. Nada passa por servidor nosso.
        </p>
        {pastaCompartilhada ? (
          <div className="space-y-2">
            <p className="text-sm text-text-primary">Pasta: <code className="break-all">{pastaCompartilhada}</code></p>
            <div className="flex flex-wrap gap-2">
              <Button size="sm" onClick={() => void compartilhar()}>Trocar pasta</Button>
              <Button size="sm" variant="ghost" onClick={() => { salvarAjustes({ pastaCompartilhada: "" }); setAviso("Voltou para as tarefas só deste computador. A pasta compartilhada não foi apagada."); }}>
                Parar de compartilhar
              </Button>
            </div>
          </div>
        ) : (
          <Button variant="primary" onClick={() => void compartilhar()}>Escolher pasta compartilhada…</Button>
        )}
        {outras.length > 0 && (
          <div className="flex flex-wrap items-center gap-2 text-sm text-text-secondary">
            Na pasta também: {outras.map((p) => <span key={p.id} className="inline-flex items-center gap-1"><Avatar pessoa={p} />{p.nome}</span>)}
          </div>
        )}
        {aviso && <p className="text-xs text-text-secondary" role="status">{aviso}</p>}
        {(erro || e.erro) && <p className="text-xs text-danger" role="alert">{erro || e.erro}</p>}
      </Card>

      <Card title="Exibição">
        <label className="flex items-center gap-2 text-sm text-text-primary">
          <input type="checkbox" checked={ocultarFeitasAntigas} onChange={(ev) => salvarAjustes({ ocultarFeitasAntigas: ev.target.checked })} />
          Esconder tarefas feitas há mais de 7 dias
        </label>
      </Card>
    </div>
  );
}
