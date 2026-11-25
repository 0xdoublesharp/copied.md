export interface FormattedDiagnostic {
  label: string;
  tooltip: string;
}

const BY_CODE: Record<string, FormattedDiagnostic> = {
  'unsupported-url': {
    label: 'Non-web links kept as text',
    tooltip:
      'Only http, https, and mailto links are kept when pasting. Local or app links (like file://) stay as readable text.',
  },
  'relative-url': {
    label: 'Relative links kept as text',
    tooltip:
      'Relative links have no base URL on the clipboard, so their link text is kept without a destination.',
  },
};

const BY_MESSAGE: Record<string, FormattedDiagnostic> = {
  'An unsupported link destination was omitted.': BY_CODE['unsupported-url'],
  'A relative link needs an original page URL.': BY_CODE['relative-url'],
};

export function formatDiagnostic(message: string, code?: string): FormattedDiagnostic {
  if (code && BY_CODE[code]) return BY_CODE[code];
  if (BY_MESSAGE[message]) return BY_MESSAGE[message];
  return { label: message, tooltip: message };
}
