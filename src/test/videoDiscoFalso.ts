// Fork (Editor de vídeo): disco falso com as regras dos comandos `videos_*` que as telas usam.
/** Disco falso com as regras dos comandos `videos_*` que a tela usa. */
export function discoFalso() {
  const arquivos = new Map<string, string>();
  const versoes = new Map<string, { n: number; motivo: string; arquivos: Record<string, string> }[]>();
  const chamadas: { cmd: string; args: Record<string, unknown> }[] = [];
  let ordem: string[] = [];
  let pastas: Record<string, unknown>[] = [];
  let n = 0;
  const ler = (id: string, rel: string) => arquivos.get(`${id}/${rel}`) ?? null;
  const gravar = (id: string, rel: string, c: string) => arquivos.set(`${id}/${rel}`, c);
  const guardar = (id: string, motivo: string) => {
    const lista = versoes.get(id) ?? [];
    const atual = { "index.html": ler(id, "index.html") ?? "", "BRIEF.md": ler(id, "BRIEF.md") ?? "", "midias.json": ler(id, "midias.json") ?? "" };
    const ultima = lista[lista.length - 1];
    if (ultima && JSON.stringify(ultima.arquivos) === JSON.stringify(atual)) return null;
    const v = { n: (ultima?.n ?? 0) + 1, motivo, arquivos: atual };
    versoes.set(id, [...lista, v]);
    return v.n;
  };
  const sessao: { papel: "host" | "convidado" | null; projeto: string | null; enviadas: string[] } = { papel: null, projeto: null, enviadas: [] };
  let agente: (texto: string, id: string) => { texto: string; arquivos: string[] } = () => ({ texto: "ok", arquivos: [] });
  return {
    arquivos, chamadas, versoes, sessao,
    definirAgente(f: typeof agente) { agente = f; },
    comando(cmd: string, args: Record<string, unknown>) {
      chamadas.push({ cmd, args });
      const id = args.id as string;
      switch (cmd) {
        case "videos_endereco_base": return "tok";
        case "videos_listar": return ordem.map((i) => ({ id: i, projeto: JSON.parse(ler(i, "projeto.json")!) }));
        case "videos_criar": {
          const novo = args.novo as { projeto: Record<string, unknown>; brief: string; index: string; instrucoes: string };
          const pid = `v-teste-${++n}`;
          gravar(pid, "projeto.json", JSON.stringify({ ...novo.projeto, id: pid, versao: 1, hyperframes: "0.8.183", criadoEm: 1, atualizadoEm: 1 }));
          gravar(pid, "BRIEF.md", novo.brief); gravar(pid, "index.html", novo.index); gravar(pid, "CLAUDE.md", novo.instrucoes);
          gravar(pid, "midias.json", JSON.stringify({ versao: 1, itens: [], links: [] }));
          ordem = [pid, ...ordem];
          return pid;
        }
        case "videos_abrir": return `/dados/videos/projetos/${id}`;
        case "videos_ler": return ler(id, args.arquivo as string);
        case "videos_gravar": {
          const rel = args.arquivo as string;
          if (rel.endsWith(".json")) JSON.parse(args.conteudo as string);
          const v = args.motivo ? guardar(id, args.motivo as string) : null;
          gravar(id, rel, args.conteudo as string);
          return v;
        }
        case "videos_anexar_conversa": gravar(id, "conversa.jsonl", `${ler(id, "conversa.jsonl") ?? ""}${args.linha as string}\n`); return null;
        case "videos_criar_versao": return guardar(id, args.motivo as string);
        case "videos_versoes": return [...(versoes.get(id) ?? [])].reverse().map((v) => ({ n: v.n, motivo: v.motivo, quando: 1, arquivos: Object.keys(v.arquivos) }));
        case "videos_restaurar_versao": {
          const v = (versoes.get(id) ?? []).find((x) => x.n === args.n);
          if (!v) throw new Error("Essa versão não existe.");
          const refazer = guardar(id, `Antes de voltar para a versão ${v.n}`);
          for (const [rel, c] of Object.entries(v.arquivos)) gravar(id, rel, c);
          return refazer;
        }
        case "videos_midias_faltando": return [];
        case "videos_composicoes": return [...arquivos.keys()].filter((k) => k.startsWith(`${id}/compositions/`)).map((k) => k.slice(id.length + 1)).sort();
        case "videos_importar_midia": {
          const caminho = args.caminho as string;
          const nome = caminho.split("/").pop()!;
          const m = JSON.parse(ler(id, "midias.json")!);
          const item = { id: `h-${nome}`, nome, arquivo: `assets/${nome.toLowerCase()}`, tipo: nome.endsWith(".mp4") ? "video" : "imagem", papel: args.papel, hash: "h", tamanho: 10, nota: "", adicionadoEm: 1 };
          m.itens.push(item);
          gravar(id, "midias.json", JSON.stringify(m));
          return item;
        }
        case "videos_ref_listar": return pastas;
        case "videos_ref_criar": { const p = { versao: 1, id: `r-${++n}`, nome: (args.dados as { nome: string }).nome, descricao: "", cor: "#7c9cff", itens: [], links: [], criadoEm: 1, atualizadoEm: 1 }; pastas = [p, ...pastas]; return p; }
        case "videos_ref_ler": return pastas.find((p) => p.id === id);
        case "videos_ref_aplicar": return [];
        case "videos_agente": {
          const pedido = args.pedido as { projeto: string; texto: string };
          const r = agente(pedido.texto, pedido.projeto);
          return { texto: r.texto, sessao: "sessao-falsa", modelo: "modelo-falso", arquivos: r.arquivos };
        }
        case "videos_agente_rodando": return false;
        // Fork (0.14): chat normal, pastas do computador e exportados.
        case "videos_assinatura": {
          // Como o Rust: "<composição>:<briefing e mídias>".
          const h = (re: RegExp) => [...arquivos.entries()].filter(([k]) => k.startsWith(`${id}/`) && re.test(k.slice(id.length + 1))).map(([, v]) => v).join("").split("").reduce((a, c) => (a * 31 + c.charCodeAt(0)) | 0, 7);
          return `${h(/^(index\.html|compositions\/|assets\/)/)}:${h(/^(BRIEF\.md|midias\.json)$/)}`;
        }
        case "videos_configurar_pastas": {
          const c = args.pastaDoComputador as string | null;
          if (c && !c.startsWith("/")) throw new Error("Escolha uma pasta do computador.");
          return [...(c ? [c] : []), ...((args.referencias as string[]) ?? []).map((r) => `/dados/videos/referencias/${r}/arquivos`)];
        }
        case "videos_conferir_pasta": return args.caminho;
        case "videos_abrir_render": return null;
        case "videos_copiar_render": return `${args.pastaDoComputador as string}/Vídeos/x.mp4`;
        case "videos_exportar": return `/dados/videos/projetos/${(args.pedido as { projeto: string }).projeto}/renders/x.mp4`;
        case "videos_sessao_situacao": return { tailscale: { pronto: true, login: "enzo@exemplo.com", dns: "mac.tail.ts.net", ips: ["100.64.0.1"] }, config: { nome: "Enzo", permitidos: ["primo@exemplo.com"] }, papel: sessao.papel, projeto: sessao.projeto, convidados: [] };
        case "videos_sessao_iniciar": sessao.papel = "host"; sessao.projeto = args.projeto as string; return "medza-video:convite-falso";
        case "videos_sessao_parar": sessao.papel = null; sessao.projeto = null; return null;
        case "videos_sessao_enviar": sessao.enviadas.push(args.corpo as string); return null;
        case "videos_sessao_config_gravar": case "videos_sessao_permitir": return null;
        case "list_profiles": return [{ id: "claude-1", provider: "claude", name: "Conta 1", config_dir: "/x", builtin: true, created_at_ms: 0, last_used_at_ms: null, authenticated: true }];
        case "agentes_modelos": return { claude: [{ id: "sonnet", label: "Sonnet" }], codex: [] };
        default: return undefined;
      }
    },
  };
}

