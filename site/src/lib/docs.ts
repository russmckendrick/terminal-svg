import { marked } from "marked";

export interface DocMeta {
  slug: string;
  file: string;
  title: string;
  deck: string;
  label: string;
}

export interface DocHeading {
  level: 2 | 3;
  text: string;
  id: string;
}

const sources = import.meta.glob<string>("../../../docs/*.md", {
  query: "?raw",
  import: "default",
  eager: true,
});

const DOC_ORDER = ["install", "usage", "themes", "architecture"];

function cleanInlineMarkdown(value: string): string {
  return value
    .replace(/!\[([^\]]*)\]\([^)]*\)/g, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/g, "$1")
    .replace(/[`*_>#]/g, "")
    .replace(/\s+/g, " ")
    .trim();
}

function metadataFor(file: string, source: string): DocMeta {
  const slug = file.replace(/\.md$/, "");
  const title = cleanInlineMarkdown(source.match(/^#\s+(.+)$/m)?.[1] ?? slug);
  const paragraph = source.split(/\n\s*\n/).find((block, index) => {
    const value = block.trim();
    return (
      index > 0 &&
      value.length > 0 &&
      !value.startsWith("#") &&
      !value.startsWith("```") &&
      !value.startsWith("|") &&
      !/^[-*]\s/.test(value)
    );
  });
  const deck = cleanInlineMarkdown(paragraph ?? title);

  return {
    slug,
    file,
    title,
    deck:
      deck.length > 180
        ? `${deck.slice(0, 177).replace(/\s+\S*$/, "")}…`
        : deck,
    label: slug,
  };
}

export const DOCS: DocMeta[] = Object.entries(sources)
  .filter(([path]) => !path.endsWith("/README.md"))
  .map(([path, source]) =>
    metadataFor(path.split("/").pop() ?? "docs.md", source),
  )
  .sort((a, b) => {
    const aOrder = DOC_ORDER.indexOf(a.slug);
    const bOrder = DOC_ORDER.indexOf(b.slug);
    return (
      (aOrder < 0 ? DOC_ORDER.length : aOrder) -
      (bOrder < 0 ? DOC_ORDER.length : bOrder)
    );
  });

function sourceFor(file: string): string {
  const entry = Object.entries(sources).find(([path]) =>
    path.endsWith(`/docs/${file}`),
  );
  return entry?.[1] ?? "";
}

function rewriteDocLinks(html: string): string {
  return html.replace(
    /href="([a-z-]+)\.md(#[^"]+)?"/g,
    (_match, slug: string, hash = "") => {
      return `href="/docs/${slug}/${hash}"`;
    },
  );
}

function decodeHtml(value: string): string {
  return value.replace(
    /&(#x?[0-9a-f]+|amp|lt|gt|quot|apos|#39);/gi,
    (match, entity: string) => {
      const lower = entity.toLowerCase();
      if (lower === "amp") return "&";
      if (lower === "lt") return "<";
      if (lower === "gt") return ">";
      if (lower === "quot") return '"';
      if (lower === "apos" || lower === "#39") return "'";
      if (lower.startsWith("#x"))
        return String.fromCodePoint(parseInt(lower.slice(2), 16));
      if (lower.startsWith("#"))
        return String.fromCodePoint(parseInt(lower.slice(1), 10));
      return match;
    },
  );
}

function escapeAttribute(value: string): string {
  return value
    .replace(/&/g, "&amp;")
    .replace(/"/g, "&quot;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;");
}

function enhanceCodeBlocks(html: string): string {
  return html.replace(
    /<pre><code(?: class="language-([^"]+)")?>([\s\S]*?)<\/code><\/pre>/g,
    (_match, language = "terminal", codeHtml: string) => {
      const code = decodeHtml(codeHtml);
      const label =
        language === "sh" || language === "shell" ? "shell" : language;
      return `<div class="snippet"><div class="head"><span class="label">${label}</span><button class="copy" type="button" aria-label="Copy ${label} code" aria-live="polite" data-copy="${escapeAttribute(code)}">Copy</button></div><pre><code>${codeHtml}</code></pre></div>`;
    },
  );
}

function removeDocumentIntro(html: string): string {
  return html.replace(/^<h1>[\s\S]*?<\/h1>\s*<p>[\s\S]*?<\/p>\s*/, "");
}

function headingId(value: string): string {
  return (
    value
      .toLowerCase()
      .replace(/<[^>]+>/g, "")
      .replace(/&[a-z0-9#]+;/gi, "")
      .replace(/[^a-z0-9]+/g, "-")
      .replace(/^-|-$/g, "") || "section"
  );
}

function enhanceHeadings(html: string): { html: string; toc: DocHeading[] } {
  const toc: DocHeading[] = [];
  const seen = new Map<string, number>();
  const enhanced = html.replace(
    /<h([23])>([\s\S]*?)<\/h\1>/g,
    (_match, rawLevel: string, content: string) => {
      const level = Number(rawLevel) as 2 | 3;
      const text = decodeHtml(content.replace(/<[^>]+>/g, "")).trim();
      const base = headingId(text);
      const count = seen.get(base) ?? 0;
      seen.set(base, count + 1);
      const id = count === 0 ? base : `${base}-${count + 1}`;
      toc.push({ level, text, id });
      return `<h${level} id="${id}">${content}<a class="heading-anchor" href="#${id}" aria-label="Link to ${escapeAttribute(text)}">#</a></h${level}>`;
    },
  );
  return { html: enhanced, toc };
}

export async function getDoc(
  slug: string,
): Promise<(DocMeta & { html: string; toc: DocHeading[] }) | null> {
  const meta = DOCS.find((doc) => doc.slug === slug);
  if (!meta) return null;
  const html = await marked.parse(sourceFor(meta.file));
  const content = enhanceCodeBlocks(rewriteDocLinks(removeDocumentIntro(html)));
  const enhanced = enhanceHeadings(content);
  return { ...meta, html: enhanced.html, toc: enhanced.toc };
}
