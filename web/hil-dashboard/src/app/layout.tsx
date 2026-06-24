import type { Metadata } from "next";
import "./globals.css";
import { NavBar } from "@/components/NavBar";

export const metadata: Metadata = {
  title: "Secure Telemetry Gateway",
  description: "ESP32-S3 secure telemetry gateway & SDR Hardware-in-the-Loop simulator",
};

export default function RootLayout({ children }: { children: React.ReactNode }) {
  return (
    <html lang="zh-Hant">
      <body>
        <NavBar />
        {children}
      </body>
    </html>
  );
}
