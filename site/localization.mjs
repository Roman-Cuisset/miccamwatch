// Build-time localization only; no framework or translation service in the browser.
export const languages = Object.freeze([
  { code: 'en', name: 'English', ogLocale: 'en_US' },
  { code: 'fr', name: 'Français', ogLocale: 'fr_FR' },
  { code: 'de', name: 'Deutsch', ogLocale: 'de_DE' },
  { code: 'es', name: 'Español', ogLocale: 'es_ES' },
  { code: 'ja', name: '日本語', ogLocale: 'ja_JP' },
  { code: 'zh', name: '简体中文', ogLocale: 'zh_CN' },
  { code: 'ru', name: 'Русский', ogLocale: 'ru_RU' },
]);

export function escapeMarkup(value) {
  return value.replace(/[&<>"']/g, (character) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[character]);
}

function sortedMatches(value, expression) {
  return [...value.matchAll(expression)].map((match) => match[0]).sort();
}

function sameEntries(left, right) {
  return left.length === right.length && left.every((item, index) => item === right[index]);
}

export function validateCatalog(catalog, language, master) {
  const fields = ['language', 'name', 'ogLocale', 'messages'];
  if (!catalog || typeof catalog !== 'object' || Array.isArray(catalog) ||
      Object.keys(catalog).length !== fields.length || fields.some((field) => !Object.hasOwn(catalog, field))) {
    throw new Error(`Invalid catalog schema: ${language.code}.`);
  }
  if (catalog.language !== language.code || catalog.name !== language.name || catalog.ogLocale !== language.ogLocale) {
    throw new Error(`Catalog identity mismatch: ${language.code}.`);
  }
  if (!catalog.messages || typeof catalog.messages !== 'object' || Array.isArray(catalog.messages)) {
    throw new Error(`Invalid messages: ${language.code}.`);
  }
  const expected = Object.keys(master.messages).sort();
  if (!sameEntries(expected, Object.keys(catalog.messages).sort())) {
    throw new Error(`Missing or unexpected translation keys: ${language.code}. No fallback is permitted.`);
  }
  for (const key of expected) {
    const translated = catalog.messages[key];
    const source = master.messages[key];
    if (typeof translated !== 'string' || !translated.trim()) throw new Error(`Empty translation: ${language.code}/${key}.`);
    // Translators may reorder emphasis for grammar, not alter links, code or markup.
    for (const expression of [/<\/?[A-Za-z][^>]*>/g, /<code\b[^>]*>[\s\S]*?<\/code>/g, /__MCW_[A-Z0-9_]+__/g]) {
      if (!sameEntries(sortedMatches(source, expression), sortedMatches(translated, expression))) {
        throw new Error(`Technical markup/code/token changed: ${language.code}/${key}.`);
      }
    }
    const stack = [];
    for (const match of translated.matchAll(/<(\/?)([A-Za-z][A-Za-z0-9]*)(?:\s[^>]*)?>/g)) {
      const [, closing, tag] = match;
      if (['br', 'img', 'hr', 'input', 'meta', 'link'].includes(tag.toLowerCase())) continue;
      if (closing) {
        if (stack.pop() !== tag) throw new Error(`Unbalanced translation markup: ${language.code}/${key}.`);
      } else stack.push(tag);
    }
    if (stack.length) throw new Error(`Unclosed translation markup: ${language.code}/${key}.`);
    if (/\{\{/.test(translated)) throw new Error(`Nested localization marker: ${language.code}/${key}.`);
  }
  return catalog;
}

export function htmlLanguage(code) {
  return code === 'zh' ? 'zh-Hans' : code;
}

export function languageLinks(code, filename) {
  return languages.map((language) => {
    const current = language.code === code ? ' aria-current="page"' : '';
    const target = filename === '404.html' ? `/${language.code}/404.html` : `/${language.code}/`;
    return `<a href="${target}" lang="${htmlLanguage(language.code)}" hreflang="${htmlLanguage(language.code)}"${current}>${language.name}</a>`;
  }).join('');
}

export function localizedPage(template, catalog, filename) {
  let rendered = template.replace(/\{\{(@?)\s*([a-z0-9_.:-]+)\s*\}\}/g, (_, attribute, key) => {
    if (!Object.hasOwn(catalog.messages, key)) throw new Error(`Missing template message: ${catalog.language}/${key}.`);
    return attribute ? escapeMarkup(catalog.messages[key]) : catalog.messages[key];
  });
  if (/\{\{/.test(rendered)) throw new Error(`Unresolved localization marker: ${catalog.language}/${filename}.`);
  rendered = rendered.replace('lang="__MCW_LANG__"', `lang="${htmlLanguage(catalog.language)}"`);
  rendered = rendered.replaceAll('mcw status --lang fr', 'mcw status --lang __MCW_LANG__');
  // Root-local home links on translated error pages remain in the selected language.
  rendered = rendered.replaceAll('href="/"', `href="/${catalog.language}/"`)
    .replaceAll('href="/#', `href="/${catalog.language}/#`);
  return rendered;
}

export function pageMetadata(origin, code) {
  if (!origin) return '';
  const canonical = `${origin}/${code}/`;
  const image = `${origin}/assets/${code === 'fr' ? 'social.png' : `social-${code}.png`}`;
  const links = languages.map((language) => `<link rel="alternate" hreflang="${htmlLanguage(language.code)}" href="${origin}/${language.code}/">`);
  return [
    `<link rel="canonical" href="${canonical}">`,
    ...links,
    `<link rel="alternate" hreflang="x-default" href="${origin}/">`,
    `<meta property="og:url" content="${canonical}">`,
    `<meta property="og:image" content="${image}">`,
    `<meta name="twitter:image" content="${image}">`,
  ].join('\n  ');
}

export function sitemap(origin) {
  const urls = languages.map(({ code }) => `<url><loc>${escapeMarkup(`${origin}/${code}/`)}</loc></url>`).join('');
  return `<?xml version="1.0" encoding="UTF-8"?>\n<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">${urls}</urlset>\n`;
}
