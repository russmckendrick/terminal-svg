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

// display metadata shared by the gallery page and the finishes picker
export const THEME_META = [
  { key: 'dracula', label: 'Dracula', a: '#bd93f9', b: '#282a36' },
  { key: 'catppuccin-mocha', label: 'Catppuccin Mocha', a: '#f5c2e7', b: '#1e1e2e' },
  { key: 'nord', label: 'Nord', a: '#88c0d0', b: '#2e3440' },
  { key: 'tokyo-night', label: 'Tokyo Night', a: '#7aa2f7', b: '#1a1b26' },
  { key: 'github-dark', label: 'GitHub Dark', a: '#58a6ff', b: '#0d1117' },
  { key: 'github-light', label: 'GitHub Light', a: '#0969da', b: '#ffffff' },
  { key: 'solarized-dark', label: 'Solarized Dark', a: '#b58900', b: '#002b36' },
] as const;

export const FIXTURE_META = [
  { key: 'boxdrawing', label: 'Box drawing' },
  { key: 'cjk-emoji', label: 'CJK & emoji' },
  { key: 'colors16', label: '16 colors' },
  { key: 'colors256', label: '256 colors' },
  { key: 'progress', label: 'Progress bars' },
  { key: 'sgr-styles', label: 'Text styles' },
  { key: 'starship', label: 'Starship prompt' },
  { key: 'truecolor', label: 'True color' },
  { key: 'typing-anim', label: 'Typing demo' },
] as const;

export function themeLabel(key: string): string {
  return THEME_META.find((t) => t.key === key)?.label ?? key;
}

export function fixtureLabel(key: string): string {
  return FIXTURE_META.find((f) => f.key === key)?.label ?? key;
}
