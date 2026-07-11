// Gallery SVGs live at the repo root (/gallery), outside site/. Vite emits each
// one into dist/_astro/ with a content hash via the ?url glob, so nothing is
// committed twice. The ?raw glob is only used to read intrinsic dimensions.

// Theme names contain hyphens (catppuccin-mocha, github-dark, ...), so
// filenames must be parsed against this explicit list, never split on '-'.
const THEMES = [
  "catppuccin-mocha",
  "dracula",
  "github-dark",
  "github-light",
  "nord",
  "powershell",
  "solarized-dark",
  "tokyo-night",
  "ubuntu",
] as const;

export interface GalleryItem {
  theme: string;
  fixture: string;
  url: string;
  width: number;
  height: number;
  animated: boolean;
}

const urls = import.meta.glob<string>("../../../gallery/*.svg", {
  query: "?url",
  import: "default",
  eager: true,
});

const raws = import.meta.glob<string>("../../../gallery/*.svg", {
  query: "?raw",
  import: "default",
  eager: true,
});

function dimensions(svg: string): { width: number; height: number } {
  const w = svg.match(/\bwidth="(\d+(?:\.\d+)?)"/);
  const h = svg.match(/\bheight="(\d+(?:\.\d+)?)"/);
  return { width: w ? Number(w[1]) : 668, height: h ? Number(h[1]) : 400 };
}

export const items: GalleryItem[] = Object.entries(urls)
  .map(([path, url]) => {
    const base = path
      .split("/")
      .pop()!
      .replace(/\.svg$/, "");
    const theme = THEMES.find((t) => base.startsWith(`${t}-`));
    if (!theme)
      throw new Error(`Gallery file has unknown theme prefix: ${base}`);
    const fixture = base.slice(theme.length + 1);
    return {
      theme,
      fixture,
      url,
      ...dimensions(raws[path] ?? ""),
      animated: fixture === "typing-anim" || fixture === "typing-v3-anim",
    };
  })
  .sort(
    (a, b) =>
      a.fixture.localeCompare(b.fixture) || a.theme.localeCompare(b.theme),
  );

export const themes: string[] = [...new Set(items.map((i) => i.theme))].sort();
export const fixtures: string[] = [
  ...new Set(items.map((i) => i.fixture)),
].sort();

export function find(theme: string, fixture: string): GalleryItem | undefined {
  return items.find((i) => i.theme === theme && i.fixture === fixture);
}

// Display metadata shared by the gallery page and homepage specimens.
// a/c/d are accent colors from the theme's own palette (drawn as tiny text
// lines in the theme chips); b is the terminal background; chrome picks the
// mini window dressing on the chip.
export const THEME_META = [
  {
    key: "dracula",
    label: "Dracula",
    a: "#bd93f9",
    b: "#282a36",
    c: "#50fa7b",
    d: "#ff79c6",
    chrome: "macos",
  },
  {
    key: "catppuccin-mocha",
    label: "Catppuccin Mocha",
    a: "#f5c2e7",
    b: "#1e1e2e",
    c: "#a6e3a1",
    d: "#89b4fa",
    chrome: "macos",
  },
  {
    key: "nord",
    label: "Nord",
    a: "#88c0d0",
    b: "#2e3440",
    c: "#a3be8c",
    d: "#ebcb8b",
    chrome: "macos",
  },
  {
    key: "tokyo-night",
    label: "Tokyo Night",
    a: "#7aa2f7",
    b: "#1a1b26",
    c: "#9ece6a",
    d: "#bb9af7",
    chrome: "macos",
  },
  {
    key: "github-dark",
    label: "GitHub Dark",
    a: "#58a6ff",
    b: "#0d1117",
    c: "#3fb950",
    d: "#d29922",
    chrome: "macos",
  },
  {
    key: "github-light",
    label: "GitHub Light",
    a: "#0969da",
    b: "#ffffff",
    c: "#1a7f37",
    d: "#cf222e",
    chrome: "macos",
  },
  {
    key: "solarized-dark",
    label: "Solarized Dark",
    a: "#268bd2",
    b: "#002b36",
    c: "#859900",
    d: "#b58900",
    chrome: "macos",
  },
  {
    key: "powershell",
    label: "PowerShell",
    a: "#f9f1a5",
    b: "#012456",
    c: "#16c60c",
    d: "#61d6d6",
    chrome: "windows",
  },
  {
    key: "ubuntu",
    label: "Ubuntu",
    a: "#8ae234",
    b: "#300a24",
    c: "#729fcf",
    d: "#fce94f",
    chrome: "ubuntu",
  },
] as const;

export const FIXTURE_META = [
  { key: "boxdrawing", label: "Box drawing" },
  { key: "cjk-emoji", label: "CJK & emoji" },
  { key: "colors16", label: "16 colors" },
  { key: "colors256", label: "256 colors" },
  { key: "progress", label: "Progress bars" },
  { key: "sgr-styles", label: "Text styles" },
  { key: "starship", label: "Starship prompt" },
  { key: "truecolor", label: "True color" },
  { key: "typing-anim", label: "Typing demo" },
  { key: "typing-v3-anim", label: "Typing v3 demo" },
] as const;

export function themeLabel(key: string): string {
  return THEME_META.find((t) => t.key === key)?.label ?? key;
}

export function fixtureLabel(key: string): string {
  return FIXTURE_META.find((f) => f.key === key)?.label ?? key;
}
