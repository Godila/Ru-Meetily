import { Metadata } from 'next';

export const metadata: Metadata = {
  title: 'Convoic',
  description: 'Голос ваших встреч, превращённый в смысл — локальный AI-ассистент для встреч с распознаванием русской речи',
  icons: {
    icon: [
      { url: '/convoic_favicon.ico', sizes: 'any' },
      { url: '/convoic_icon_32.png', type: 'image/png', sizes: '32x32' },
      { url: '/convoic_icon_128.png', type: 'image/png', sizes: '128x128' },
    ],
    apple: [
      { url: '/convoic_icon_128.png', sizes: '128x128' },
    ],
  },
};
