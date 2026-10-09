import assert from 'node:assert/strict';
import { readFileSync, existsSync } from 'node:fs';
import { test } from 'node:test';
import { runInNewContext } from 'node:vm';

const web = new URL('../web/', import.meta.url);
const html = readFileSync(new URL('index.html', web), 'utf8');
const script = readFileSync(new URL('app.js', web), 'utf8');
const attribute = (tag, name) => tag.match(new RegExp(`\\b${name}\\s*=\\s*["']([^"']*)["']`))?.[1];
const commandLines = text => text.trim().split('\n').map(line => line.trim()).join('\n');
const commands = Object.fromEntries([...html.matchAll(/<code\b([^>]*)>([^<]*)<\/code\s*>/g)]
  .filter(([, tag]) => attribute(tag, 'id'))
  .map(([, tag, textContent]) => [attribute(tag, 'id'), { textContent }]));

function element(dataset = {}) {
  return {
    dataset, hidden: true, events: {}, attributes: {}, textContent: '',
    addEventListener(event, handler) { this.events[event] = handler; },
    setAttribute(name, value) { this.attributes[name] = value; },
  };
}

function load({ saved = null, dark = false, storageError = false, clipboard = true, copyError = false } = {}) {
  const root = element();
  const themeButton = element();
  const status = element();
  const copyButtons = Object.keys(commands).map(copy => element({ copy }));
  const media = { ...element(), matches: dark };
  const writes = [];
  const copied = [];
  let ready = false;
  const document = {
    documentElement: root,
    events: {},
    addEventListener(event, handler) { this.events[event] = handler; },
    querySelector(selector) {
      if (!ready) return null;
      if (selector === '.theme-toggle') return themeButton;
      if (selector === '.copy-status') return status;
      throw new Error(`Unexpected selector: ${selector}`);
    },
    querySelectorAll(selector) {
      assert.equal(selector, '[data-copy]');
      return copyButtons;
    },
    getElementById(id) { return commands[id]; },
  };
  runInNewContext(script, {
    document,
    window: { matchMedia: () => media },
    localStorage: {
      getItem(key) {
        assert.equal(key, 'worktree-theme');
        if (storageError) throw new Error('Storage unavailable');
        return saved;
      },
      setItem(key, value) {
        if (storageError) throw new Error('Storage unavailable');
        writes.push([key, value]);
      },
    },
    navigator: clipboard ? { clipboard: { async writeText(value) {
      if (copyError) throw new Error('Clipboard permission denied');
      copied.push(value);
    } } } : {},
  });
  ready = true;
  document.events.DOMContentLoaded();
  return { root, themeButton, status, copyButtons, media, writes, copied };
}

test('theme follows the OS until a user chooses; choice persists with an accessible label', () => {
  const page = load();
  assert.equal(page.root.dataset.theme, 'light');
  assert.equal(page.themeButton.hidden, false);
  page.media.events.change({ matches: true });
  assert.equal(page.root.dataset.theme, 'dark');
  page.themeButton.events.click();
  assert.equal(page.root.dataset.theme, 'light');
  assert.equal(page.themeButton.attributes['aria-label'], 'Switch to dark theme');
  assert.deepEqual(page.writes, [['worktree-theme', 'light']]);
  page.media.events.change({ matches: true });
  assert.equal(page.root.dataset.theme, 'light');
});

test('saved light and dark choices override the OS; invalid preferences fall back', () => {
  for (const saved of ['light', 'dark']) {
    const page = load({ saved, dark: saved !== 'dark' });
    assert.equal(page.root.dataset.theme, saved);
    page.media.events.change({ matches: saved !== 'dark' });
    assert.equal(page.root.dataset.theme, saved);
  }
  assert.equal(load({ saved: 'invalid', dark: true }).root.dataset.theme, 'dark');
});

test('blocked localStorage still allows OS fallback and manual theme changes', () => {
  const page = load({ storageError: true, dark: true });
  assert.equal(page.root.dataset.theme, 'dark');
  page.themeButton.events.click();
  assert.equal(page.root.dataset.theme, 'light');
  page.media.events.change({ matches: true });
  assert.equal(page.root.dataset.theme, 'light');
});

test('each copy button copies its whole command and announces success', async () => {
  const page = load();
  assert.ok(page.copyButtons.length >= 3);
  for (const button of page.copyButtons) {
    assert.equal(button.hidden, false);
    await button.events.click();
    assert.equal(page.copied.at(-1), commands[button.dataset.copy].textContent.trim());
    assert.match(page.status.textContent, /copied/i);
  }
});

test('clipboard rejection announces a manual fallback; unavailable clipboard keeps controls hidden', async () => {
  const rejected = load({ copyError: true });
  await rejected.copyButtons[0].events.click();
  assert.match(rejected.status.textContent, /could not copy.*select and copy/i);
  assert.deepEqual(rejected.copied, []);
  const unavailable = load({ clipboard: false });
  assert.ok(unavailable.copyButtons.every(button => button.hidden));
  assert.equal(unavailable.themeButton.hidden, false);
});

test('static page is indexable, has working local targets, and uses actual image dimensions', () => {
  const origin = 'https://worktree.aross.se/';
  assert.equal(attribute(html.match(/<html\b[^>]*>/)[0], 'lang'), 'en');
  assert.equal([...html.matchAll(/<h1\b/g)].length, 1);
  const canonical = [...html.matchAll(/<link\b[^>]*>/g)].find(([tag]) => attribute(tag, 'rel') === 'canonical');
  assert.equal(attribute(canonical?.[0] ?? '', 'href'), origin);
  assert.doesNotMatch(html, /noindex/i);
  assert.match(readFileSync(new URL('robots.txt', web), 'utf8'), /User-agent: \*\s+Allow: \/\s+Sitemap: https:\/\/worktree\.aross\.se\/sitemap\.xml/);
  assert.equal(readFileSync(new URL('sitemap.xml', web), 'utf8').match(/<loc>\s*([^<]+?)\s*<\/loc>/)[1], origin);
  const structured = JSON.parse([...html.matchAll(/<script\b([^>]*)>([\s\S]*?)<\/script\s*>/g)]
    .find(([, tag]) => attribute(tag, 'type') === 'application/ld+json')[2]);
  assert.equal(structured.url, origin);
  assert.equal(structured.name, 'worktree');
  const ids = new Set([...html.matchAll(/\bid\s*=\s*["']([^"']+)["']/g)].map(([, id]) => id));
  for (const [, target] of html.matchAll(/(?:src|href)\s*=\s*["']([^"']+)["']/g)) {
    if (target.startsWith('#')) assert.ok(ids.has(target.slice(1)), target);
    else if (target.startsWith('/')) assert.ok(existsSync(new URL(target === '/' ? 'index.html' : target.slice(1), web)), target);
  }
  for (const [, attributes] of html.matchAll(/<img\b([^>]+)>/g)) {
    assert.ok(attribute(attributes, 'alt'));
    const src = attribute(attributes, 'src');
    if (!src.endsWith('.png')) continue;
    const png = readFileSync(new URL(src.slice(1), web));
    assert.equal(Number(attribute(attributes, 'width')), png.readUInt32BE(16), `${src} width`);
    assert.equal(Number(attribute(attributes, 'height')), png.readUInt32BE(20), `${src} height`);
  }
});

test('Homebrew and shell instructions match README, with project and author links', () => {
  const readme = readFileSync(new URL('../README.md', import.meta.url), 'utf8');
  const readmeCommands = [...readme.matchAll(/```sh\s*\n([\s\S]*?)```/g)].map(([, text]) => commandLines(text));
  assert.ok(readmeCommands.includes(commandLines(commands['brew-command'].textContent)));
  assert.ok(readmeCommands.includes(commandLines(commands['zsh-command'].textContent)));
  assert.equal(commandLines(commands['bash-command'].textContent), commandLines(commands['zsh-command'].textContent).replace('init.zsh', 'init.bash'));
  const links = [...html.matchAll(/<a\b[^>]*>/g)].map(([tag]) => attribute(tag, 'href'));
  assert.ok(links.includes('https://aross.se'));
  assert.ok(links.includes('https://github.com/alex-ross/worktree#install'));
});
