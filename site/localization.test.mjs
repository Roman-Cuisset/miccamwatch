import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { runInNewContext } from 'node:vm';
import { languages, localizedPage, pageMetadata, validateCatalog } from './localization.mjs';

const french = {
  language: 'fr', name: 'Français', ogLocale: 'fr_FR',
  messages: {
    link: '<a href="https://example.org/v__MCW_VERSION__/mcw.zip">Télécharger</a>',
    command: '<code>mcw status --lang fr</code>',
    provenance: '__MCW_SOURCE__ / __MCW_INSTALLER_SHA256__',
  },
};
function english() {
  return {
    language: 'en', name: 'English', ogLocale: 'en_US',
    messages: { ...french.messages, link: french.messages.link.replace('Télécharger', 'Download') },
  };
}
const en = languages.find(({ code }) => code === 'en');

test('metadata keys containing a colon render with safe attribute escaping', () => {
  const template = '<html lang="__MCW_LANG__"><meta content="{{@ meta:description }}"><p>{{ body }}</p></html>';
  const catalog = { language: 'ja', messages: { 'meta:description': '"<&', body: '日本語' } };
  const output = localizedPage(template, catalog, 'index.html');
  assert.match(output, /lang="ja"/);
  assert.match(output, /content="&quot;&lt;&amp;"/);
  assert.match(output, /<p>日本語<\/p>/);
  assert.doesNotMatch(output, /\{\{/);
});

test('a missing translation cannot silently use another language', () => {
  for (const mutate of [
    (value) => { delete value.messages.command; },
    (value) => { value.messages.extra = 'Unexpected'; },
    (value) => { value.messages.command = ' '; },
  ]) {
    const value = english(); mutate(value);
    assert.throws(() => validateCatalog(value, en, french), Error);
  }
  assert.throws(() => localizedPage('<p>{{ absent }}</p>', english(), 'index.html'), Error);
});

test('translation cannot change executable URLs, commands or provenance tokens', () => {
  for (const mutate of [
    (value) => { value.messages.link = value.messages.link.replace('example.org', 'unrelated.invalid'); },
    (value) => { value.messages.command = '<code>mcw unmute</code>'; },
    (value) => { value.messages.provenance = '__MCW_SOURCE__'; },
  ]) {
    const value = english(); mutate(value);
    assert.throws(() => validateCatalog(value, en, french), Error);
  }
});

test('translation cannot break the markup nesting of a safety warning', () => {
  const master = { ...french, messages: { notice: '<strong><span>Attention</span></strong>' } };
  const value = { ...english(), messages: { notice: '<strong><span>Warning</strong></span>' } };
  assert.throws(() => validateCatalog(value, en, master), Error);
});

test('pageMetadata declares canonical root and Schema WebSite structured data on homepages only', () => {
  const origin = 'https://miccamwatch.pages.dev';
  const enMeta = pageMetadata(origin, 'en', 'index.html');
  assert.match(enMeta, /<link rel="canonical" href="https:\/\/miccamwatch\.pages\.dev\/">/);
  assert.match(enMeta, /<script type="application\/ld\+json">\{"@context":"https:\/\/schema\.org","@type":"WebSite","name":"MicCamWatch","alternateName":"MCW","url":"https:\/\/miccamwatch\.pages\.dev\/"\}<\/script>/);

  const frMeta = pageMetadata(origin, 'fr', 'index.html');
  assert.match(frMeta, /<link rel="canonical" href="https:\/\/miccamwatch\.pages\.dev\/fr\/">/);
  assert.match(frMeta, /<script type="application\/ld\+json">\{"@context":"https:\/\/schema\.org","@type":"WebSite","name":"MicCamWatch","alternateName":"MCW","url":"https:\/\/miccamwatch\.pages\.dev\/"\}<\/script>/);

  const privacyMeta = pageMetadata(origin, 'en', 'privacy.html');
  assert.match(privacyMeta, /<link rel="canonical" href="https:\/\/miccamwatch\.pages\.dev\/en\/privacy">/);
  assert.doesNotMatch(privacyMeta, /WebSite/);
});

test("theme and upcoming version are translated in all seven catalogs", () => {
  const catalog = (code) => JSON.parse(readFileSync(new URL("./locales/" + code + ".json", import.meta.url), "utf8"));
  const master = catalog("fr");
  const template = readFileSync(new URL("./templates/index.html", import.meta.url), "utf8");
  for (const lang of languages) {
    const current = validateCatalog(catalog(lang.code), lang, master);
    for (const key of ["theme.label", "theme.system", "theme.light", "theme.dark",
      "home.upcoming.title", "home.upcoming.lead", "home.upcoming.status", "home.upcoming.stable"]) {
      assert.ok(current.messages[key]?.trim(), lang.code + ": " + key);
    }
    const announcement = localizedPage(template, current, "index.html").split('class="upcoming shell"')[1].split("</section>")[0];
    assert.match(announcement, /v0\.22\.0/);
    assert.match(announcement, /Flutter.*|.*Flutter/);
    assert.deepEqual([...announcement.matchAll(/href="([^"]*)"/g)].map((match) => match[1]), ["#installer"]);
  }
});

function launchTheme(storage) {
  const root = { dataset: {} };
  const label = { hidden: true };
  let select;
  let ready;
  const control = {
    value: "",
    closest: () => label,
    addEventListener(_event, change) { select = change; },
  };
  const document = {
    documentElement: root,
    querySelector: () => control,
    addEventListener(_event, listener) { ready = listener; },
  };
  runInNewContext(readFileSync(new URL("./theme.js", import.meta.url), "utf8"), { document, localStorage: storage });
  return { root, control, label, ready, choose(value) { control.value = value; select(); } };
}

test("a saved theme applies before the control loads and persists on navigation", () => {
  let saved = "dark";
  const storage = { getItem: () => saved, setItem(_key, value) { saved = value; }, removeItem() { saved = null; } };
  const first = launchTheme(storage);
  assert.equal(first.root.dataset.theme, "dark");
  assert.equal(first.label.hidden, true);
  first.ready();
  assert.equal(first.control.value, "dark");
  assert.equal(first.label.hidden, false);
  first.choose("light");
  const next = launchTheme(storage);
  assert.equal(next.root.dataset.theme, "light");
  next.ready();
  next.choose("system");
  assert.equal(next.root.dataset.theme, undefined);
  assert.equal(saved, null);
  assert.equal(launchTheme(storage).root.dataset.theme, undefined);
  saved = "unknown";
  assert.equal(launchTheme(storage).root.dataset.theme, undefined);
});

test("storage failures leave automatic mode and the page selector usable", () => {
  const denied = () => { throw new Error("Storage denied"); };
  const page = launchTheme({ getItem: denied, setItem: denied, removeItem: denied });
  assert.equal(page.root.dataset.theme, undefined);
  page.ready();
  assert.equal(page.control.value, "system");
  page.choose("dark");
  assert.equal(page.root.dataset.theme, "dark");
  page.choose("system");
  assert.equal(page.root.dataset.theme, undefined);
});
