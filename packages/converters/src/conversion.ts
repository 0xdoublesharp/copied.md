import twitterText from 'twitter-text';
import { EXIT, visit } from 'unist-util-visit';
import { readMarkdown } from './document';
import { renderMarkdown } from './render-markdown';
import { renderText } from './render-text';
import { renderRichHtml } from './rich-html';
import { signalSpoilerFallback } from './signal-spoilers';
import type { ClipboardPayload, Diagnostic } from './types';

const targetDefinitions = [
  { id: 'rich-text', label: 'Rich Text', rich: true },
  { id: 'plain-text', label: 'Plain Text', rich: false },
  { id: 'html-source', label: 'HTML Source', rich: false },
  { id: 'slack-paste', label: 'Slack', rich: true },
  { id: 'discord', label: 'Discord', rich: false },
  { id: 'telegram', label: 'Telegram', rich: true },
  { id: 'signal', label: 'Signal', rich: true },
  { id: 'reddit', label: 'Reddit', rich: false },
  { id: 'facebook', label: 'Facebook', rich: false },
  { id: 'twitter', label: 'X', rich: false },
  { id: 'sms', label: 'SMS', rich: false },
  { id: 'slack-api', label: 'Slack API (mrkdwn)', rich: false },
  { id: 'teams', label: 'Teams', rich: true, enabledByDefault: false },
  { id: 'word', label: 'Word', rich: true, enabledByDefault: false },
  { id: 'whatsapp', label: 'WhatsApp', rich: false, enabledByDefault: false },
] as const;

export type CopyTarget = (typeof targetDefinitions)[number]['id'];

export interface CopyTargetDefinition {
  readonly id: CopyTarget;
  readonly label: string;
  readonly rich: boolean;
  readonly enabledByDefault?: boolean;
}

export const targets: readonly CopyTargetDefinition[] = targetDefinitions;

export interface ConversionOptions {
  sourceUrl?: string;
  decorativeUnicode?: boolean;
}

export interface MarkdownConversion {
  schemaVersion: 1;
  target: CopyTarget;
  representations: ClipboardPayload;
  metrics: {
    sourceCodeUnits: number;
    outputLength: number;
    unit: 'code-points' | 'x-weighted-characters';
    limit?: number;
  };
  diagnostics: Diagnostic[];
}

export function resolveTarget(value: string): CopyTarget | undefined {
  if (value === 'html') return 'rich-text';
  if (value === 'slack') return 'slack-paste';
  return targets.find((item) => item.id === value)?.id;
}

export function convertMarkdown(
  markdown: string,
  target: CopyTarget,
  options: ConversionOptions = {},
): MarkdownConversion {
  const profile = targets.find((item) => item.id === target);
  if (!profile) throw new RangeError('Unknown copy target: ' + target);
  const { tree, diagnostics } = readMarkdown(markdown, options.sourceUrl);
  const markdownTarget = target === 'discord' || target === 'reddit';
  let hasImages = false;
  visit(tree, 'image', () => {
    hasImages = true;
    return EXIT;
  });
  const plain = markdownTarget
    ? renderMarkdown(tree, target)
    : renderText(tree, {
        slackApi: target === 'slack-api',
        slackPaste: target === 'slack-paste',
        whatsapp: target === 'whatsapp',
        decorativeUnicode:
          options.decorativeUnicode &&
          (target === 'facebook' || target === 'twitter' || target === 'telegram'),
        telegram: target === 'telegram' ? 'plain' : undefined,
      });
  const representations: ClipboardPayload = { 'text/plain': plain };
  if (target === 'telegram' && plain) {
    representations['text/rtf'] = renderText(tree, { telegram: 'rtf' });
  }
  if (target === 'signal') {
    const html = renderText(tree, { signal: true });
    const fallback = signalSpoilerFallback(html);
    if (fallback) {
      representations['text/plain'] = fallback.plain;
      diagnostics.push({
        code: 'spoiler-omitted',
        severity: fallback.malformed ? 'warning' : 'info',
        message: fallback.malformed
          ? 'A malformed Signal spoiler was omitted.'
          : 'Plain-text paste omits Signal spoilers.',
      });
    }
    if (!fallback?.malformed) representations['text/signal'] = html;
  } else if (profile.rich || target === 'html-source') {
    const destination =
      target === 'teams' || target === 'word' || target === 'slack-paste' ? target : undefined;
    const html =
      target === 'telegram'
        ? renderText(tree, { telegram: 'html' })
        : renderRichHtml(tree, options.sourceUrl, destination);
    if (target === 'html-source') representations['text/plain'] = html;
    else representations['text/html'] = html;
  }
  if (hasImages && (profile.rich || target === 'html-source'))
    diagnostics.push({
      code: 'formatting-omitted',
      severity: 'info',
      message: 'Images are represented as links.',
    });

  const output = representations['text/plain'];
  const weighted = target === 'twitter' ? twitterText.parseTweet(output).weightedLength : undefined;
  if (weighted !== undefined && weighted > 280) {
    diagnostics.push({
      code: 'length-exceeded',
      severity: 'warning',
      message: 'This exceeds the standard X post limit of 280 weighted characters.',
    });
  }

  return {
    schemaVersion: 1,
    target,
    representations,
    metrics: {
      sourceCodeUnits: markdown.length,
      outputLength: weighted ?? Array.from(output).length,
      unit: weighted === undefined ? 'code-points' : 'x-weighted-characters',
      ...(weighted !== undefined ? { limit: 280 } : {}),
    },
    diagnostics,
  };
}
