"use client";

import Link from "next/link";
import { usePathname } from "next/navigation";
import { PipelineSwitch } from "./PipelineSwitch";

const LINKS: { href: string; label: string }[] = [
  { href: "/", label: "HIL 模擬器 / HIL Simulator" },
  { href: "/gateway", label: "安全閘道 / Secure Gateway" },
  { href: "/about", label: "架構 / Architecture" },
];

export function NavBar() {
  const pathname = usePathname();
  return (
    <nav className="nav-bar">
      <div className="nav-brand">
        <span className="nav-logo" />
        Secure Telemetry Gateway
      </div>
      <div className="nav-links">
        {LINKS.map((link) => {
          const active =
            link.href === "/" ? pathname === "/" : pathname.startsWith(link.href);
          return (
            <Link
              key={link.href}
              href={link.href}
              className={active ? "nav-link active" : "nav-link"}
            >
              {link.label}
            </Link>
          );
        })}
      </div>
      <PipelineSwitch />
    </nav>
  );
}
