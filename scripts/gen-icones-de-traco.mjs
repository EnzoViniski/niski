/**
 * Fork (estilo Minimalista): gera src/components/ui/iconesDeTraco.ts, a versão de traço fino de cada
 * ícone pixel de `PixelIcon.tsx`, tirada do `lucide-static` (ISC). O `PixelIcon` desenha os dois
 * desenhos no mesmo <svg> e o CSS mostra um só: pixel no Clássico, traço no Minimalista
 * (`src/styles/minimalista.css`). Ícone sem par aqui (marcas: npm, docker...) fica pixel nos dois.
 *
 * `lucide-static` é devDependency: só este script precisa dele.
 *
 * Uso: node scripts/gen-icones-de-traco.mjs  (depois rode `node scripts/gen-pixel-icons.mjs` se
 * a lista de ícones pixel mudou).
 */
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const svgDir = join(root, "node_modules", "lucide-static", "icons");
const output = join(root, "src", "components", "ui", "iconesDeTraco.ts");
const { version } = JSON.parse(readFileSync(join(root, "node_modules", "lucide-static", "package.json"), "utf8"));

/** Nome exportado em PixelIcon.tsx -> arquivo em lucide-static/icons. */
export const TRACOS = {
  CheckIcon: "check",
  ChevronDownIcon: "chevron-down",
  SearchIcon: "search",
  LoaderIcon: "loader",
  NotificationIcon: "bell",
  SlidersIcon: "sliders-horizontal",
  EditIcon: "pencil",
  ChatIcon: "message-square",
  TerminalIcon: "square-terminal",
  ListIcon: "list",
  DockerIcon: "container",
  FolderIcon: "folder",
  FolderPlusIcon: "folder-plus",
  BookIcon: "book-open",
  GitBranchIcon: "git-branch",
  KanbanIcon: "square-kanban",
  BoxIcon: "box",
  CloseIcon: "x",
  CopyIcon: "copy",
  ReloadIcon: "rotate-cw",
  PlayIcon: "play",
  StopIcon: "square",
  MicIcon: "mic",
  TrashIcon: "trash-2",
  AlertIcon: "circle-alert",
  WarningIcon: "triangle-alert",
  CircleIcon: "circle",
  ZapOffIcon: "zap-off",
  FileIcon: "file",
  FileTextIcon: "file-text",
  ArchiveIcon: "archive",
  PackageIcon: "package",
  ImageIcon: "image",
  VideoIcon: "video",
  ClapperboardIcon: "clapperboard",
  MusicIcon: "music",
  WaveformIcon: "audio-waveform",
  HeadphoneIcon: "headphones",
  CodeIcon: "code",
  ScriptIcon: "scroll-text",
  BracesIcon: "braces",
  BracketsIcon: "brackets",
  BracketsAngleIcon: "code-xml",
  BinaryIcon: "binary",
  AlgorithmIcon: "workflow",
  HashIcon: "hash",
  GearIcon: "settings",
  DatabaseIcon: "database",
  ServerIcon: "server",
  CloudIcon: "cloud",
  GlobeIcon: "globe",
  LockIcon: "lock",
  KeyIcon: "key-round",
  ShieldIcon: "shield",
  ReactIcon: "atom",
  NoteIcon: "notepad-text",
  NotesIcon: "notebook-pen",
  NotebookIcon: "notebook",
  ArticleIcon: "newspaper",
  BookmarkIcon: "bookmark",
  ClipboardNoteIcon: "clipboard-list",
  PresentationIcon: "presentation",
  ChartIcon: "chart-column",
  ChartLineIcon: "chart-line",
  CalculatorIcon: "calculator",
  BlocksIcon: "blocks",
  CpuIcon: "cpu",
  GpuIcon: "microchip",
  CircuitIcon: "circuit-board",
  TestTubeIcon: "test-tube",
  BugIcon: "bug",
  ToolsIcon: "wrench",
  LabelIcon: "tag",
  FlagIcon: "flag",
  MailIcon: "mail",
  LinkIcon: "link",
  CameraIcon: "camera",
  BrushIcon: "brush",
  SwatchIcon: "swatch-book",
  ShapesIcon: "shapes",
  MonitorIcon: "monitor",
  KeyboardIcon: "keyboard",
  CoinsIcon: "coins",
  MoneyIcon: "banknote",
  ReceiptIcon: "receipt",
  InvoiceIcon: "receipt-text",
  StickyNoteIcon: "sticky-note",
  CardTextIcon: "letter-text",
  BulletListIcon: "list",
  SaveIcon: "save",
  RobotIcon: "bot",
  SparkleIcon: "sparkles",
  StarIcon: "star",
  EyeIcon: "eye",
  CalendarIcon: "calendar",
  ClockIcon: "clock",
  AttachmentIcon: "paperclip",
  SendIcon: "send",
  ArrowLeftIcon: "arrow-left",
};

const ATRIBUTOS = ["d", "cx", "cy", "r", "rx", "ry", "x", "y", "width", "height", "x1", "y1", "x2", "y2", "points"];

/** Elementos de desenho do SVG do Lucide, como [tag, atributos] (sem estilo: o traço vem do <g>). */
function extrair(nome) {
  const arquivo = join(svgDir, `${nome}.svg`);
  if (!existsSync(arquivo)) throw new Error(`lucide-static não tem ${nome}.svg`);
  const svg = readFileSync(arquivo, "utf8").replace(/<svg[^>]*>/, "").replace(/<\/svg>/, "");
  const elementos = [...svg.matchAll(/<(path|circle|rect|line|polyline|polygon|ellipse)\b([^>]*)\/?>/g)].map(([, tag, resto]) => {
    const attrs = {};
    for (const [, chave, valor] of resto.matchAll(/([a-z0-9-]+)="([^"]*)"/g)) if (ATRIBUTOS.includes(chave)) attrs[chave] = valor;
    return [tag, attrs];
  });
  if (elementos.length === 0) throw new Error(`${nome}.svg não tem nenhum elemento de desenho`);
  return elementos;
}

const linhas = Object.entries(TRACOS).map(([exportado, nome]) => `  ${exportado}: ${JSON.stringify(extrair(nome))},`);

writeFileSync(output, `/**
 * ARQUIVO GERADO — não editar à mão. Rode \`node scripts/gen-icones-de-traco.mjs\`.
 *
 * Fork (estilo Minimalista): par de traço fino de cada ícone pixel, de \`lucide-static\` v${version} (ISC).
 * Cada entrada é a lista de elementos do SVG 24×24 como [tag, atributos].
 */
export type ElementoDeTraco = [string, Record<string, string>];

export const ICONES_DE_TRACO: Record<string, ElementoDeTraco[]> = {
${linhas.join("\n")}
};
`, "utf8");
console.log(`iconesDeTraco.ts gerado: ${linhas.length} ícones (lucide-static ${version}).`);
