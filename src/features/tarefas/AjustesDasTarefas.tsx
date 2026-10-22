// Fork (Tarefas): seção "Tarefas" das Configurações — quem sou eu e a pasta compartilhada.
import { useState } from "react";
import { Button, Card, Input } from "../../components/ui";
import { Avatar } from "./CartaoDaTarefa";
import { compartilharEm, pessoasConhecidas, salvarAjustes, useTarefas } from "./loja";
import { CORES_DE_PESSOA } from "./tipos";
import { mudarPainel, usePainel } from "./painel";

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
  const painel = usePainel();

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

      <Card title="Como ligar no Google Drive (passo a passo)">
        <ol className="list-decimal space-y-2 pl-5 text-sm text-text-secondary">
          <li>Instale o <strong className="text-text-primary">Google Drive para computador</strong> (drive.google.com/download) nos dois computadores e entre na conta Google.
            No Mac ele aparece no Finder como <code>Google Drive</code> (em <code>~/Library/CloudStorage/</code>).</li>
          <li>No Drive (site ou Finder), crie uma pasta, por exemplo <code>Tarefas Niski</code>, e <strong className="text-text-primary">compartilhe com o e-mail do seu primo</strong> como Editor.</li>
          <li>No computador dele, a pasta aparece em <em>Compartilhados comigo</em>. Ele clica com o botão direito → <em>Organizar → Adicionar atalho ao Drive</em> (em <em>Meu Drive</em>), para ela aparecer no Finder dele.</li>
          <li>Deixe a pasta disponível no computador: botão direito nela no Finder → <em>Disponível off-line</em> (assim o Niski lê e grava mesmo sem internet e sincroniza depois).</li>
          <li>Aqui embaixo, clique em <strong className="text-text-primary">Escolher pasta compartilhada…</strong> e escolha essa pasta. Ele faz o mesmo no Niski dele.</li>
          <li>Pronto: o que um cria ou muda aparece para o outro em alguns segundos (o tempo do Drive sincronizar + até 15 s do Niski reler).</li>
        </ol>
        <p className="text-xs text-text-muted">Funciona igual com iCloud Drive (pasta compartilhada pelo Finder), Dropbox ou OneDrive. Cada tarefa é um arquivo pequeno em <code>niski-tarefas/</code>; não mexa nesses arquivos à mão.</p>
      </Card>

      <Card title="Exibição">
        <label className="flex items-center gap-2 text-sm text-text-primary">
          <input type="checkbox" checked={painel.botaoVisivel} onChange={(ev) => mudarPainel({ botaoVisivel: ev.target.checked })} />
          Mostrar o botão flutuante das tarefas (canto inferior direito)
        </label>
        <label className="flex items-center gap-2 text-sm text-text-primary">
          <input type="checkbox" checked={ocultarFeitasAntigas} onChange={(ev) => salvarAjustes({ ocultarFeitasAntigas: ev.target.checked })} />
          Esconder tarefas feitas há mais de 7 dias
        </label>
      </Card>
    </div>
  );
}
