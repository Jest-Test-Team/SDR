import type { Metadata } from "next";
import "./globals.css";

export const metadata: Metadata = {
  title: "ESP32-S3 HIL 模擬器",
  description: "ESP32-S3 to SDR Hardware-in-the-Loop simulator",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="zh-Hant">
      <body>{children}</body>
    </html>
  );
}
