// Fork (Editor de vídeo): duração de vídeo/áudio lida pelos metadados (só o começo do arquivo, pelo Range).
const cache = new Map<string, Promise<number | undefined>>();

export function duracaoDaMidia(url: string, tipo: "video" | "audio"): Promise<number | undefined> {
  let p = cache.get(url);
  if (!p) {
    p = new Promise((resolver) => {
      if (typeof document === "undefined") { resolver(undefined); return; }
      const el = document.createElement(tipo);
      el.preload = "metadata";
      el.muted = true;
      const fim = (v: number | undefined) => { el.removeAttribute("src"); el.load(); resolver(v); };
      const limite = window.setTimeout(() => fim(undefined), 8000);
      el.onloadedmetadata = () => { window.clearTimeout(limite); fim(Number.isFinite(el.duration) ? el.duration : undefined); };
      el.onerror = () => { window.clearTimeout(limite); fim(undefined); };
      el.src = url;
    });
    cache.set(url, p);
    void p.then((v) => { if (v === undefined) cache.delete(url); });
  }
  return p;
}

export function tamanhoLegivel(bytes: number) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 ** 2) return `${(bytes / 1024).toFixed(0)} KB`;
  if (bytes < 1024 ** 3) return `${(bytes / 1024 ** 2).toFixed(1)} MB`;
  return `${(bytes / 1024 ** 3).toFixed(2)} GB`;
}
