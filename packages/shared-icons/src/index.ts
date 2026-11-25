// These marks share a 24 × 24 viewbox and inherit the badge's foreground color.
const fill = (d: string, fill = 'currentColor') => ({ d, fill });
const outline = (d: string) => ({
  d,
  fill: 'none',
  stroke: 'currentColor',
  'stroke-width': 1.85,
  'stroke-linecap': 'round' as const,
  'stroke-linejoin': 'round' as const,
});

const slackAccent = '#3c193d';
const slack = [
  fill('M9 3a2 2 0 1 0 0 4h2V5a2 2 0 0 0-2-2ZM3 8a2 2 0 1 0 0 4h6a2 2 0 0 0 0-4Z', '#36c5f0'),
  {
    ...fill(
      'M21 9a2 2 0 1 0-4 0v2h2a2 2 0 0 0 2-2Zm-5-6a2 2 0 1 0-4 0v6a2 2 0 0 0 4 0Z',
      '#2eb67d',
    ),
    transform: 'translate(1 1)',
  },
  {
    ...fill(
      'M15 21a2 2 0 1 0 0-4h-2v2a2 2 0 0 0 2 2Zm6-5a2 2 0 1 0 0-4h-6a2 2 0 1 0 0 4Z',
      '#ecb22e',
    ),
    transform: 'translate(-1 1)',
  },
  fill('M3 15a2 2 0 1 0 4 0v-2H5a2 2 0 0 0-2 2Zm5 6a2 2 0 1 0 4 0v-6a2 2 0 1 0-4 0Z', '#e01e5a'),
] as const;

export const formatIcons = {
  teams: {
    accent: '#5b5fc7',
    paths: [
      fill(
        'M9 12a4 4 0 1 0 0-8 4 4 0 0 0 0 8Zm7.2-1a3 3 0 1 0 0-6 5.6 5.6 0 0 1 0 6ZM2 19a5 5 0 0 1 5-5h4a5 5 0 0 1 5 5v1H2Zm16 1v-1a7 7 0 0 0-2.1-5h1.6a4.5 4.5 0 0 1 4.5 4.5V20Z',
      ),
    ],
  },
  word: {
    accent: '#185abd',
    paths: [outline('M13 3H6v18h12V8l-5-5v5h5M9 12h6m-6 4h6')],
  },
  whatsapp: {
    accent: '#167a4a',
    paths: [
      outline('M21 11.5a9 9 0 0 1-13.4 7.9L3 21l1.6-4.6A9 9 0 1 1 21 11.5Z'),
      fill(
        'M8.4 7.2c.4-.4 1-.4 1.3.1l1 1.8c.2.3.1.6-.2.9l-.6.5a8.7 8.7 0 0 0 3.6 3.4l.6-.7c.2-.3.6-.4.9-.2l1.7 1c.5.3.5 1 .2 1.4-.5.7-1.4 1-2.2.8a11.2 11.2 0 0 1-7.2-7c-.2-.8.1-1.5.9-2Z',
      ),
    ],
  },
  signal: {
    accent: '#3a76f0',
    paths: [
      {
        d: 'M20.5 11.5a8.5 8.5 0 0 1-12.7 7.4L3.5 20l1.2-4.1a8.5 8.5 0 1 1 15.8-4.4Z',
        fill: 'none',
        stroke: 'currentColor',
        'stroke-width': '1.6',
        'stroke-linecap': 'round',
        'stroke-dasharray': '1.7 2.4',
      },
      fill('M12 6a5.5 5.5 0 1 1-2.9 10.2l-2 .6.6-2A5.5 5.5 0 0 1 12 6Z'),
    ],
  },
  telegram: {
    accent: '#037cb2',
    paths: [
      fill(
        'm3 11 17-6.5c.8-.3 1.4.2 1.2 1l-2.7 13c-.1.7-.8 1-1.4.6l-4.6-3.4-2.1 2c-.3.3-.7.2-.7-.3l.3-3.6 7.5-6.8-9.3 5.7-5.1-1.5c-.5-.2-.6-.9-.1-1.2Z',
      ),
    ],
  },
  'slack-paste': { accent: slackAccent, paths: slack },
  'slack-api': { accent: slackAccent, paths: slack },
  discord: {
    accent: '#4b55de',
    paths: [
      {
        ...fill(
          'M8.6 5.2 8 6.5a13 13 0 0 1 8 0l-.6-1.3c1.3.2 2.6.6 3.8 1.2a18 18 0 0 1 2.7 10.2c-1.4 1-3 1.7-4.6 2.2l-1-1.5 1.7-.9-.4-.3a12.8 12.8 0 0 1-11.2 0l-.4.3 1.7.9-1 1.5A15 15 0 0 1 2 16.6 18 18 0 0 1 4.8 6.4C6 5.8 7.3 5.4 8.6 5.2Zm.2 6c-1 0-1.7.8-1.7 1.8s.7 1.8 1.7 1.8c.9 0 1.6-.8 1.6-1.8s-.7-1.8-1.6-1.8Zm6.4 0c-.9 0-1.6.8-1.6 1.8s.7 1.8 1.6 1.8c1 0 1.7-.8 1.7-1.8s-.7-1.8-1.7-1.8Z',
        ),
        'fill-rule': 'evenodd',
      },
    ],
  },
  reddit: {
    accent: '#cc3900',
    paths: [
      outline('m12 9 1.4-4.4L18 5.8'),
      fill('M19 4.4a1.6 1.6 0 1 0 0 3.2 1.6 1.6 0 0 0 0-3.2Z'),
      {
        ...fill(
          'M12 9c2.4 0 4.5.7 5.9 1.7a2.2 2.2 0 1 1 1.7 3.7c-.2 2.6-3.4 4.6-7.6 4.6s-7.4-2-7.6-4.6a2.2 2.2 0 1 1 1.7-3.7C7.5 9.7 9.6 9 12 9Zm-3 3a1.3 1.3 0 1 0 0 2.6A1.3 1.3 0 0 0 9 12Zm6 0a1.3 1.3 0 1 0 0 2.6 1.3 1.3 0 0 0 0-2.6Zm-5.6 3.6a.6.6 0 0 0-.8.8 5 5 0 0 0 6.8 0 .6.6 0 1 0-.8-.8 3.8 3.8 0 0 1-5.2 0Z',
        ),
        'fill-rule': 'evenodd',
      },
    ],
  },
  facebook: {
    accent: '#1861cf',
    paths: [
      fill(
        'M13.8 21v-8h2.7l.4-3h-3.1V8.1c0-1 .3-1.6 1.6-1.6H17V3.8a21 21 0 0 0-2.4-.2c-2.5 0-4.2 1.5-4.2 4.3V10H8v3h2.4v8Z',
      ),
    ],
  },
  twitter: {
    accent: '#111318',
    paths: [
      {
        ...fill(
          'M4 4h5l4 5.5L18 4h2l-6 7 6.5 9h-5l-4.4-6-5.1 6H4l6.2-7.4L4 4Zm2.8 1.5 9.5 13h1.4l-9.4-13Z',
        ),
        'fill-rule': 'evenodd',
      },
    ],
  },
  sms: {
    accent: '#287948',
    paths: [
      outline(
        'M5 5h14a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2h-8l-5 4v-4H5a2 2 0 0 1-2-2V7a2 2 0 0 1 2-2Zm2 5h10m-10 3h7',
      ),
    ],
  },
  'rich-text': {
    accent: '#6546b7',
    paths: [outline('M4 8V6h16v2M12 6v12m-3 0h6')],
  },
  'plain-text': {
    accent: '#505d71',
    paths: [outline('M5 6h14M5 10h10M5 14h14M5 18h8')],
  },
  'html-source': {
    accent: '#167473',
    paths: [outline('m7 8-4 4 4 4m10-8 4 4-4 4m-3-11-4 14')],
  },
} as const;

export type FormatIconId = keyof typeof formatIcons;
