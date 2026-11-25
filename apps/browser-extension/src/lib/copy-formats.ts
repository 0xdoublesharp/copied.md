import { resolveTarget, targets, type CopyTarget } from '@0xdoublesharp/copied-md-converters';

export type FormatVisibility = Partial<Record<CopyTarget, boolean>>;

type Format = (typeof targets)[number] & { enabledByDefault?: boolean };

export function visibleFormats(visibility: FormatVisibility = {}) {
  const visible = targets.filter(
    (format: Format) => visibility[format.id] ?? format.enabledByDefault !== false,
  );
  return visible.length ? visible : targets.filter(({ id }) => id === 'rich-text');
}

export function preferredFormat(visibility: FormatVisibility = {}, ...preferences: unknown[]) {
  const formats = visibleFormats(visibility);
  for (const preference of preferences) {
    if (typeof preference !== 'string') continue;
    const target = resolveTarget(preference);
    if (target && formats.some(({ id }) => id === target)) return target;
  }
  return formats[0].id;
}
