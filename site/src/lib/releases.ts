// Build-time fetch of GitHub releases. Deliberately never throws: the site is
// rebuilt by a weekly cron and on release publish, and an API hiccup must not
// fail the build. Consumers render a fallback path when the list is empty.

import { marked } from 'marked';

export const REPO = 'russmckendrick/terminal-svg';
export const REPO_URL = `https://github.com/${REPO}`;

export interface ReleaseAsset {
  name: string;
  url: string;
  size: number;
}

export interface Release {
  tag: string;
  name: string;
  publishedAt: string;
  htmlUrl: string;
  bodyHtml: string;
  assets: ReleaseAsset[];
}

let cache: Promise<Release[]> | null = null;

export function getReleases(): Promise<Release[]> {
  cache ??= fetchReleases();
  return cache;
}

export async function getLatest(): Promise<Release | null> {
  return (await getReleases())[0] ?? null;
}

async function fetchReleases(): Promise<Release[]> {
  const headers: Record<string, string> = {
    Accept: 'application/vnd.github+json',
    'X-GitHub-Api-Version': '2022-11-28',
  };
  const token = process.env.GITHUB_TOKEN;
  if (token) headers.Authorization = `Bearer ${token}`;

  try {
    const res = await fetch(`https://api.github.com/repos/${REPO}/releases?per_page=100`, {
      headers,
    });
    if (!res.ok) {
      console.warn(`[releases] GitHub API returned ${res.status}; building without release data`);
      return [];
    }
    const json = (await res.json()) as any[];
    const releases: Release[] = [];
    for (const r of json) {
      if (r.draft || r.prerelease) continue;
      releases.push({
        tag: r.tag_name,
        name: r.name || r.tag_name,
        publishedAt: r.published_at,
        htmlUrl: r.html_url,
        bodyHtml: await marked.parse(r.body ?? ''),
        assets: (r.assets ?? []).map((a: any) => ({
          name: a.name,
          url: a.browser_download_url,
          size: a.size,
        })),
      });
    }
    releases.sort((a, b) => b.publishedAt.localeCompare(a.publishedAt));
    return releases;
  } catch (err) {
    console.warn(`[releases] fetch failed (${err}); building without release data`);
    return [];
  }
}

export function formatSize(bytes: number): string {
  if (bytes >= 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${Math.round(bytes / 1024)} KB`;
}

export function formatDate(iso: string): string {
  return new Date(iso).toLocaleDateString('en-GB', {
    day: 'numeric',
    month: 'long',
    year: 'numeric',
  });
}
