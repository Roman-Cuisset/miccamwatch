import { createHash } from 'node:crypto';
import { constants, promises as fs } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { languages, languageLinks, localizedPage, pageMetadata, sitemap, validateCatalog } from './localization.mjs';

// Run from any working directory. Node is needed only for this local export.
const root = await fs.realpath(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..'));
const sourceDirectory = path.join(root, 'site');
const outputDirectory = path.join(root, 'site-dist');
const markerName = '.mcw-site-export.json';
const owner = 'miccamwatch-static-site';
const originMarker = '<!-- MCW_ORIGIN_META -->';
const textExtensions = new Set(['.html', '.css', '.js', '.json', '.svg', '.txt', '.xml']);
const entryFiles = ['main.css', 'main.js', '_headers', '_redirects', 'robots.txt'];

async function statIfPresent(filename) {
  try {
    return await fs.lstat(filename);
  } catch (error) {
    if (error.code === 'ENOENT') return null;
    throw error;
  }
}

async function requireDirectory(directory) {
  const stat = await fs.lstat(directory);
  if (!stat.isDirectory() || stat.isSymbolicLink()) {
    throw new Error(`Expected a real directory, not a symlink: ${directory}`);
  }
}

async function readRegularFile(filename) {
  const stat = await fs.lstat(filename);
  if (!stat.isFile() || stat.isSymbolicLink()) {
    throw new Error(`Expected a regular file, not a symlink: ${filename}`);
  }
  return fs.readFile(filename);
}

function validateRelease(value) {
  if (!value || Array.isArray(value) || typeof value !== 'object') {
    throw new Error('site/release.json must be an object.');
  }
  const fields = ['version', 'source', 'installerSha256'];
  if (Object.keys(value).length !== fields.length || fields.some((field) => typeof value[field] !== 'string')) {
    throw new Error('site/release.json must contain only version, source and installerSha256 strings.');
  }
  if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?$/.test(value.version)) {
    throw new Error('Release version must be a semantic version without a v prefix.');
  }
  if (!/^[a-f0-9]{40}$/.test(value.source)) throw new Error('Release source must be a lowercase 40-character Git commit.');
  if (!/^[a-f0-9]{64}$/.test(value.installerSha256)) throw new Error('Installer SHA256 must be lowercase 64-character hexadecimal.');
  return value;
}

function configuredOrigin(productionOrigin) {
  const name = process.env.MCW_SITE_URL !== undefined ? 'MCW_SITE_URL' : 'site/origin.json productionOrigin';
  const value = process.env.MCW_SITE_URL !== undefined ? process.env.MCW_SITE_URL : productionOrigin;
  let url;
  try {
    url = new URL(value);
  } catch {
    throw new Error(`${name} must be an HTTPS root origin, such as https://mcw.example.org.`);
  }
  if (url.protocol !== 'https:' || url.username || url.password || url.pathname !== '/' || url.search || url.hash ||
      (value !== url.origin && value !== `${url.origin}/`)) {
    throw new Error(`${name} must be an HTTPS root origin without credentials, a path, a query or a fragment.`);
  }
  return url.origin;
}

function renderText(content, filename, release, origin, catalog = null) {
  const tokens = {
    __MCW_VERSION__: release.version,
    __MCW_SOURCE__: release.source,
    __MCW_INSTALLER_SHA256__: release.installerSha256,
  };
  if (catalog) Object.assign(tokens, {
    __MCW_LANG__: catalog.language,
    __MCW_OG_LOCALE__: catalog.ogLocale,
    __MCW_LANGUAGE_LINKS__: languageLinks(catalog.language, filename),
  });
  let rendered = content.replace(/__MCW_[A-Z0-9_]+__/g, (token) => {
    if (!Object.hasOwn(tokens, token)) throw new Error(`Unknown template token ${token} in ${filename}.`);
    return tokens[token];
  });
  if (filename === 'index.html') {
    if (rendered.split(originMarker).length !== 2) throw new Error('index.html must have exactly one MCW_ORIGIN_META comment.');
    rendered = rendered.replace(originMarker, pageMetadata(origin, catalog.language));
  } else if (filename === '404.html') {
    rendered = rendered.replaceAll(originMarker, '');
  }
  if (rendered.includes(originMarker) || /__MCW_[A-Z0-9_]+__/.test(rendered)) {
    throw new Error(`Unresolved template marker in ${filename}.`);
  }
  if (filename === 'robots.txt' && origin) rendered = `${rendered.trimEnd()}\n\nSitemap: ${origin}/sitemap.xml\n`;
  return rendered;
}

async function readCatalogs() {
  await requireDirectory(path.join(sourceDirectory, 'locales'));
  const french = JSON.parse((await readRegularFile(path.join(sourceDirectory, 'locales', 'fr.json'))).toString('utf8'));
  validateCatalog(french, languages.find(({ code }) => code === 'fr'), french);
  return Promise.all(languages.map(async (language) => {
    const catalog = language.code === 'fr' ? french :
      JSON.parse((await readRegularFile(path.join(sourceDirectory, 'locales', `${language.code}.json`))).toString('utf8'));
    return validateCatalog(catalog, language, french);
  }));
}

function safeRelative(filename) {
  return typeof filename === 'string' && filename.length > 0 &&
    filename.split('/').every((part) => part.length > 0 && part !== '.' && part !== '..' && !/[\\:\x00-\x1f]/.test(part));
}

async function walk(directory, prefix = '') {
  const entries = [];
  const children = await fs.readdir(directory, { withFileTypes: true });
  children.sort((a, b) => a.name.localeCompare(b.name, 'en'));
  for (const child of children) {
    const relative = prefix ? `${prefix}/${child.name}` : child.name;
    if (!safeRelative(relative)) throw new Error(`Unsafe file name: ${relative}`);
    const filename = path.join(directory, child.name);
    const stat = await fs.lstat(filename);
    if (stat.isSymbolicLink() || (!stat.isFile() && !stat.isDirectory())) {
      throw new Error(`Symlinks and special files cannot be exported or replaced: ${filename}`);
    }
    entries.push({ relative, directory: stat.isDirectory() });
    if (stat.isDirectory()) entries.push(...await walk(filename, relative));
  }
  return entries;
}

async function inspectOutput() {
  const stat = await statIfPresent(outputDirectory);
  if (!stat) return null;
  if (!stat.isDirectory() || stat.isSymbolicLink()) throw new Error('site-dist is not a real directory; refusing to replace it.');
  const markerPath = path.join(outputDirectory, markerName);
  const markerStat = await statIfPresent(markerPath);
  if (!markerStat || !markerStat.isFile() || markerStat.isSymbolicLink()) {
    throw new Error('site-dist has no export ownership marker; refusing to replace it.');
  }
  const markerBytes = await readRegularFile(markerPath);
  let marker;
  try {
    marker = JSON.parse(markerBytes.toString('utf8'));
  } catch {
    throw new Error('site-dist ownership marker is invalid; refusing to replace it.');
  }
  if (marker.owner !== owner || marker.schema !== 1 || !Array.isArray(marker.files) ||
      !marker.files.includes('index.html') || marker.files.some((filename) => !safeRelative(filename) || filename === markerName) ||
      new Set(marker.files).size !== marker.files.length) {
    throw new Error('site-dist ownership marker is unrecognized or unsafe; refusing to replace it.');
  }
  return { stat, markerBytes, files: new Set(marker.files), entries: await walk(outputDirectory) };
}

async function writeGenerated(stage, filename, bytes, generated) {
  if (!safeRelative(filename)) throw new Error(`Unsafe export path: ${filename}`);
  const destination = path.join(stage, filename);
  await fs.mkdir(path.dirname(destination), { recursive: true });
  await fs.writeFile(destination, bytes, { flag: 'wx' });
  generated.push(filename);
}

async function preserveUnrelated(stage, previous) {
  if (!previous) return;
  for (const entry of previous.entries) {
    if (entry.relative === markerName || previous.files.has(entry.relative)) continue;
    const destination = path.join(stage, entry.relative);
    const destinationStat = await statIfPresent(destination);
    if (entry.directory) {
      if (destinationStat && !destinationStat.isDirectory()) throw new Error(`Unrelated directory collides with new export: ${entry.relative}`);
      await fs.mkdir(destination, { recursive: true });
    } else {
      if (destinationStat) throw new Error(`Unrelated file collides with new export: ${entry.relative}`);
      await fs.mkdir(path.dirname(destination), { recursive: true });
      await fs.copyFile(path.join(outputDirectory, entry.relative), destination, constants.COPYFILE_EXCL);
    }
  }
}

async function assertOutputUnchanged(previous) {
  const current = await statIfPresent(outputDirectory);
  if (!previous) {
    if (current) throw new Error('site-dist appeared during export; refusing to replace it.');
    return;
  }
  if (!current || !current.isDirectory() || current.isSymbolicLink() || current.dev !== previous.stat.dev || current.ino !== previous.stat.ino) {
    throw new Error('site-dist changed during export; refusing to replace it.');
  }
  const marker = await readRegularFile(path.join(outputDirectory, markerName));
  if (!marker.equals(previous.markerBytes)) throw new Error('site-dist ownership changed during export; refusing to replace it.');
}

async function build() {
  await requireDirectory(sourceDirectory);
  await requireDirectory(path.join(root, 'installer'));
  const release = validateRelease(JSON.parse((await readRegularFile(path.join(sourceDirectory, 'release.json'))).toString('utf8')));
  const publication = JSON.parse((await readRegularFile(path.join(sourceDirectory, 'origin.json'))).toString('utf8'));
  if (!publication || Array.isArray(publication) || Object.keys(publication).length !== 1 || typeof publication.productionOrigin !== 'string') {
    throw new Error('site/origin.json must contain only a productionOrigin HTTPS string.');
  }
  const origin = configuredOrigin(publication.productionOrigin);
  const installer = await readRegularFile(path.join(root, 'installer', 'install.sh'));
  const actualDigest = createHash('sha256').update(installer).digest('hex');
  if (actualDigest !== release.installerSha256) {
    throw new Error(`Canonical installer SHA256 mismatch: expected ${release.installerSha256}, found ${actualDigest}. Output was not modified.`);
  }
  if (/__MCW_[A-Z0-9_]+__/.test(installer.toString('utf8'))) throw new Error('Canonical installer contains unresolved template tokens.');
  const catalogs = await readCatalogs();
  await requireDirectory(path.join(sourceDirectory, 'templates'));
  const templates = new Map();
  for (const filename of ['index.html', '404.html']) {
    templates.set(filename, (await readRegularFile(path.join(sourceDirectory, 'templates', filename))).toString('utf8'));
  }
  await requireDirectory(path.join(sourceDirectory, 'assets'));
  const assets = await walk(path.join(sourceDirectory, 'assets'));
  for (const required of ['tray-windows.png', 'top-windows.png', 'social.png']) {
    if (!assets.some((entry) => entry.relative === required && !entry.directory)) throw new Error(`Missing required real site asset: assets/${required}`);
  }

  const lockPath = path.join(root, '.mcw-site-export-lock');
  let lock;
  try {
    lock = await fs.open(lockPath, 'wx');
  } catch (error) {
    if (error.code === 'EEXIST') throw new Error('Another export owns .mcw-site-export-lock. If a previous process was interrupted, remove that stale lock only after checking it is no longer running.');
    throw error;
  }
  let workspace;
  let keepWorkspace = false;
  try {
    await lock.writeFile(`${process.pid}\n`);
    const previous = await inspectOutput();
    workspace = await fs.mkdtemp(path.join(root, '.mcw-site-export-'));
    const stage = path.join(workspace, 'next');
    const backup = path.join(workspace, 'previous');
    await fs.mkdir(stage);
    const generated = [];
    for (const filename of entryFiles) {
      const bytes = await readRegularFile(path.join(sourceDirectory, filename));
      await writeGenerated(stage, filename, renderText(bytes.toString('utf8'), filename, release, origin), generated);
    }
    for (const catalog of catalogs) {
      for (const [filename, template] of templates) {
        const localized = localizedPage(template, catalog, filename);
        const rendered = renderText(localized, filename, release, origin, catalog);
        await writeGenerated(stage, `${catalog.language}/${filename}`, rendered, generated);
        if (catalog.language === 'fr') await writeGenerated(stage, filename, rendered, generated);
      }
    }
    for (const asset of assets) {
      if (asset.directory) continue;
      const filename = `assets/${asset.relative}`;
      const bytes = await readRegularFile(path.join(sourceDirectory, filename));
      const rendered = textExtensions.has(path.extname(filename).toLowerCase())
        ? renderText(bytes.toString('utf8'), filename, release, origin)
        : bytes;
      await writeGenerated(stage, filename, rendered, generated);
    }
    await writeGenerated(stage, 'install.sh', installer, generated);
    if (origin) {
      await writeGenerated(stage, 'sitemap.xml', sitemap(origin), generated);
    }
    await writeGenerated(stage, markerName, `${JSON.stringify({ owner, schema: 1, files: [...generated].sort() }, null, 2)}\n`, []);
    await preserveUnrelated(stage, previous);
    await assertOutputUnchanged(previous);
    if (previous) await fs.rename(outputDirectory, backup);
    try {
      await fs.rename(stage, outputDirectory);
    } catch (publishError) {
      if (previous) {
        try {
          await fs.rename(backup, outputDirectory);
        } catch (restoreError) {
          keepWorkspace = true;
          throw new Error(`Publish failed (${publishError.message}); restoring the previous export also failed (${restoreError.message}). Previous files remain at ${backup}.`);
        }
      }
      throw publishError;
    }
    console.log(`Exported MCW ${release.version} to ${outputDirectory}`);
    console.log(origin ? `Canonical origin: ${origin}` : 'No site origin configured: canonical/social URLs and sitemap are omitted.');
    console.log('Upload the contents of site-dist to Cloudflare Pages; no account or API token is needed to build.');
  } finally {
    try {
      if (workspace && !keepWorkspace) await fs.rm(workspace, { recursive: true, force: true });
    } finally {
      try {
        await lock.close();
      } finally {
        await fs.unlink(lockPath);
      }
    }
  }
}

try {
  await build();
} catch (error) {
  console.error(`MCW site export failed: ${error.message}`);
  process.exitCode = 1;
}
