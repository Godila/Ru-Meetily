import './globals.css'
import { Source_Sans_3 } from 'next/font/google'
import type { Metadata } from 'next'
import ClientLayout from './ClientLayout'

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
}

const sourceSans3 = Source_Sans_3({
  subsets: ['latin'],
  weight: ['400', '500', '600', '700'],
  variable: '--font-source-sans-3',
})

export default function RootLayout({
  children,
}: {
  children: React.ReactNode
}) {
  return (
    <html lang="ru">
      <body className={`${sourceSans3.variable} font-sans antialiased`}>
        <ClientLayout>{children}</ClientLayout>
      </body>
    </html>
  )
}
