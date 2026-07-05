// Gallery SVGs live at the repo root (/gallery), outside site/. Vite emits each
// one into dist/_astro/ with a content hash via the ?url glob, so nothing is
// committed twice. The ?raw glob is only used to read intrinsic dimensions.

// Theme names contain hyphens (catppuccin-mocha, github-dark, ...), so
// filenames must be parsed against this explicit list, never split on '-'.
const THEMES = [
  'catppuccin-mocha',
  'dracula',
  'github-dark',
  'github-light',
  'nord',
  'solarized-dark',
  'tokyo-night',
] as const;

export interface GalleryItem {
  theme: string;
  fixture: string;
  url: string;
  width: number;
  height: number;
  animated: boolean;
}

const urls = import.meta.glob<string>('../../../gallery/*.svg', {
  query: '?url',
  import: 'default',
  eager: true,
});

const raws = import.meta.glob<string>('../../../gallery/*.svg', {
  query: '?raw',
  import: 'default',
  eager: true,
});

function dimensions(svg: string): { width: number; height: number } {
  const w = svg.match(/\bwidth="(\d+(?:\.\d+)?)"/);
  const h = svg.match(/\bheight="(\d+(?:\.\d+)?)"/);
  return { width: w ? Number(w[1]) : 668, height: h ? Number(h[1]) : 400 };
}

export const items: GalleryItem[] = Object.entries(urls)
  .map(([path, url]) => {
    const base = path.split('/').pop()!.replace(/\.svg$/, '');
    const theme = THEMES.find((t) => base.startsWith(`${t}-`));
    if (!theme) throw new Error(`Gallery file has unknown theme prefix: ${base}`);
    const fixture = base.slice(theme.length + 1);
    return {
      theme,
      fixture,
      url,
      ...dimensions(raws[path] ?? ''),
      animated: fixture === 'typing-anim',
    };
  })
  .sort((a, b) => a.fixture.localeCompare(b.fixture) || a.theme.localeCompare(b.theme));

export const themes: string[] = [...new Set(items.map((i) => i.theme))].sort();
export const fixtures: string[] = [...new Set(items.map((i) => i.fixture))].sort();

export function find(theme: string, fixture: string): GalleryItem | undefined {
  return items.find((i) => i.theme === theme && i.fixture === fixture);
}
