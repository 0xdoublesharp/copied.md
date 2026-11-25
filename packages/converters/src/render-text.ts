import type { PhrasingContent, Root, RootContent } from 'mdast';
import { visit } from 'unist-util-visit';
import { tableAsText } from './clipboard-tables';
import { footnoteNumbers, htmlText } from './document';
import { signalSpanOpen, spoilerOpen } from './signal-spoilers';
import { toBold, toBoldItalic, toItalic } from './unicode';

type Style = { bold?: boolean; italic?: boolean; strike?: boolean; insideLink?: boolean };
interface TextOptions {
  slackApi?: boolean;
  slackPaste?: boolean;
  whatsapp?: boolean;
  decorativeUnicode?: boolean;
  telegram?: 'plain' | 'rtf' | 'html';
  signal?: boolean;
}

function escapeRtf(value: string): string {
  return value.replace(/[\\{}]|[^\t\n -~]/g, (character) => {
    if ('\\{}'.includes(character)) return '\\' + character;
    const codeUnit = character.charCodeAt(0);
    const signed = codeUnit < 0x8000 ? codeUnit : codeUnit - 0x10000;
    return '\\u' + signed + '?';
  });
}

function telegramHtmlLineBreaks(text: string): string {
  return text
    .replace(/\n{2,}(?=<(?:pre|blockquote)>)/g, '\n')
    .replace(/(<\/(?:pre|blockquote)>)\n{2,}/g, '$1\n')
    .replaceAll('\n', '<br>');
}

export function renderText(tree: Root, options: TextOptions = {}): string {
  const notes = footnoteNumbers(tree);
  const { slackApi, slackPaste, whatsapp, decorativeUnicode, telegram, signal } = options;
  const chatMarkup = slackApi || whatsapp;
  const rtf = telegram === 'rtf';
  const html = telegram === 'html' || signal;

  function escape(value: string): string {
    if (rtf) return escapeRtf(value);
    return slackApi || html
      ? value.replaceAll('&', '&amp;').replaceAll('<', '&lt;').replaceAll('>', '&gt;')
      : value;
  }

  function styled(value: string, style: Style): string {
    if (rtf) {
      const controls =
        (style.bold ? '\\b' : '') + (style.italic ? '\\i' : '') + (style.strike ? '\\strike' : '');
      const text = escape(value);
      return controls ? '{' + controls + ' ' + text + '}' : text;
    }
    value = escape(value);
    if (!decorativeUnicode) return value;
    if (style.bold && style.italic) return toBoldItalic(value);
    if (style.bold) return toBold(value);
    return style.italic ? toItalic(value) : value;
  }

  function link(label: string, url: string): string {
    if (!label || label === url) return escape(url);
    if (telegram === 'html' && !url.startsWith('#')) {
      const href = escape(url).replaceAll('"', '&quot;');
      return '<a href="' + href + '">' + label + '</a>';
    }
    if (rtf && !url.startsWith('#')) {
      const destination = new URL(url).href
        .replaceAll('"', '%22')
        .replaceAll('\\', '%5C')
        .replaceAll(' ', '%20');
      return (
        '{\\field{\\*\\fldinst HYPERLINK "' + escape(destination) + '"}{\\fldrslt ' + label + '}}'
      );
    }
    if (slackApi && !url.startsWith('#'))
      return '<' + escape(url.replaceAll('|', '%7C')) + '|' + label + '>';
    return url.startsWith('#') ? label : label + ' (' + escape(url) + ')';
  }

  function rawInline(source: string): string {
    if (signal) {
      if (/^<span(?:\s|>)/i.test(source)) {
        return htmlText(source, { renderSpan: (node) => signalSpanOpen(node) });
      }
      if (/^<\/span\s*>$/i.test(source)) return '</span>';
    }
    return escape(htmlText(source));
  }

  function rawBlock(source: string): string {
    if (!signal) {
      const plainOptions = slackPaste ? { renderTable: tableAsText } : undefined;
      return escape(htmlText(source, plainOptions)).trimEnd();
    }
    if (/^<\/span\s*>$/i.test(source.trim())) return '</span>';
    return htmlText(source, {
      escapeText: escape,
      renderSpan(node, content) {
        const original = source.slice(node.position?.start.offset, node.position?.end.offset);
        const close = /<\/span\s*>$/i.test(original) ? '</span>' : '';
        return signalSpanOpen(node) + content + close;
      },
    }).trimEnd();
  }

  function inline(nodes: PhrasingContent[], style: Style = {}): string {
    return nodes
      .map((node) => {
        switch (node.type) {
          case 'text':
            if (signal) {
              const partialSpoiler =
                /<span\s+[^<>\n]*\bdata-spoiler\s*=\s*(?:"true(?=[^"\w])|'true(?=[^'\w])|["']?true\b[^<>\n]*(?=\n|$))/i;
              const start = node.value.search(partialSpoiler);
              if (start !== -1) return styled(node.value.slice(0, start), style) + spoilerOpen;
            }
            return styled(node.value, style);
          case 'strong': {
            const value = inline(node.children, { ...style, bold: true });
            if (html) return '<strong>' + value + '</strong>';
            return chatMarkup ? '*' + value + '*' : value;
          }
          case 'emphasis': {
            const value = inline(node.children, { ...style, italic: true });
            if (html) return '<em>' + value + '</em>';
            return chatMarkup ? '_' + value + '_' : value;
          }
          case 'delete': {
            const value = inline(node.children, { ...style, strike: true });
            if (html) return '<s>' + value + '</s>';
            return chatMarkup ? '~' + value + '~' : value;
          }
          case 'inlineCode':
            if (signal) return '<span data-monospace="true">' + escape(node.value) + '</span>';
            if (html) return '<code>' + escape(node.value) + '</code>';
            if (rtf) return '{\\f1 ' + escape(node.value) + '}';
            return chatMarkup && !node.value.includes('`')
              ? '`' + escape(node.value) + '`'
              : escape(node.value);
          case 'link':
            return link(inline(node.children, { ...style, insideLink: true }), node.url);
          case 'image': {
            const label = escape(node.alt || 'Image');
            return style.insideLink ? label : link(label, node.url);
          }
          case 'footnoteReference':
            return '[' + notes.get(node.identifier) + ']';
          case 'html':
            return rawInline(node.value);
          case 'break':
            return '\n';
          default:
            return '';
        }
      })
      .join('');
  }

  function blocks(nodes: RootContent[], separator = '\n\n', topLevel = false): string {
    return nodes
      .map((node) => block(node, topLevel))
      .filter((value) => value !== '')
      .join(separator);
  }

  function block(node: RootContent, topLevel: boolean): string {
    switch (node.type) {
      case 'paragraph':
        return inline(node.children);
      case 'heading': {
        const value = inline(node.children, { bold: true });
        if (html) return '<strong>' + value + '</strong>';
        return chatMarkup ? '*' + value + '*' : value;
      }
      case 'blockquote': {
        const content = blocks(node.children);
        if (telegram === 'html')
          return '<blockquote>' + telegramHtmlLineBreaks(content) + '</blockquote>';
        const marker = signal ? '&gt;' : '>';
        return content
          .split('\n')
          .map((line) => marker + (line ? ' ' + line : ''))
          .join('\n');
      }
      case 'code':
        if (signal) return '<span data-monospace="true">' + escape(node.value) + '</span>';
        if (html) return '<pre>' + escape(node.value).replaceAll('\n', '&#10;') + '</pre>';
        if (rtf) return '{\\f1 ' + escape(node.value) + '}';
        if (whatsapp && !node.value.includes('```')) return '```' + node.value + '```';
        return slackApi && !node.value.includes('```')
          ? '```\n' + escape(node.value) + '\n```'
          : escape(node.value);
      case 'list':
        return node.children
          .map((item, index) => {
            const bullet = whatsapp ? '- ' : '• ';
            const marker = node.ordered ? (node.start ?? 1) + index + '. ' : bullet;
            const task = item.checked == null ? '' : item.checked ? '[x] ' : '[ ] ';
            const [first, ...lines] = blocks(item.children, item.spread ? '\n\n' : '\n').split(
              '\n',
            );
            const indent = (telegram === 'html' ? '&nbsp;' : ' ').repeat(marker.length);
            return (
              escape(marker) +
              task +
              first +
              lines
                .map(
                  (line) =>
                    '\n' +
                    (line && !(telegram === 'html' && /^<(?:pre|blockquote)>/.test(line))
                      ? indent
                      : '') +
                    line,
                )
                .join('')
            );
          })
          .join(telegram && topLevel ? '\n\n' : '\n');
      case 'table': {
        const rows = node.children
          .map((row) =>
            row.children
              .map((cell) => {
                const content = inline(cell.children);
                return rtf || telegram === 'html' ? content : content.replaceAll('|', '\\|');
              })
              .join(' | '),
          )
          .join('\n');
        return slackApi ? '```\n' + rows + '\n```' : rows;
      }
      case 'thematicBreak':
        return escape('———');
      case 'html':
        return rawBlock(node.value);
      default:
        return '';
    }
  }

  const definitions = new Map<number, string>();
  visit(tree, 'footnoteDefinition', (node) => {
    const number = notes.get(node.identifier);
    if (number !== undefined && !definitions.has(number)) {
      definitions.set(number, '[' + number + '] ' + blocks(node.children));
    }
  });
  const text = [
    blocks(tree.children, '\n\n', true),
    ...[...definitions].sort(([a], [b]) => a - b).map(([, value]) => value),
  ]
    .filter(Boolean)
    .join('\n\n');
  if (signal) return text.replaceAll('\n', '<br>');
  if (html) return telegramHtmlLineBreaks(text);
  if (!rtf) return text;
  // Telegram treats a tab followed by 0 as an automatic list marker.
  const rtfText = text
    .replaceAll('\t0', '    0')
    .replaceAll('\n', '\\par\n')
    .replaceAll('\t', '\\tab ');
  return (
    '{\\rtf1\\ansi\\deff0{\\fonttbl{\\f0\\fswiss Helvetica;}{\\f1\\fmodern Courier;}}\\uc1\\f0 ' +
    rtfText +
    '}'
  );
}
