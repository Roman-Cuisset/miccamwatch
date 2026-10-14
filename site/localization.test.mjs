import test from 'node:test';
import assert from 'node:assert/strict';
import { languages, localizedPage, validateCatalog } from './localization.mjs';

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
