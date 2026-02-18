import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { Resvg } from "@resvg/resvg-js";
import type { ReactNode } from "react";
import satori from "satori";
import { logger } from "../logger.js";

const __dirname = dirname(fileURLToPath(import.meta.url));

// Load font — try Inter, fall back to system font
let fontData: Buffer;
const fontPaths = [
  join(__dirname, "fonts", "Inter-Regular.ttf"),
  join(__dirname, "..", "graph", "fonts", "Inter-Regular.ttf"),
];

const foundFontPath = fontPaths.find((p) => existsSync(p));
if (foundFontPath) {
  fontData = readFileSync(foundFontPath);
  logger.info({ path: foundFontPath }, "Loaded Inter font");
} else {
  // Create a minimal placeholder — Satori requires at least one font
  logger.warn(
    "Inter font not found. Download Inter-Regular.ttf to src/graph/fonts/. Charts may render with fallback font.",
  );
  // We'll handle this at render time
  fontData = Buffer.alloc(0);
}

export async function renderToImage(
  element: ReactNode,
  width = 800,
  height = 450,
): Promise<Buffer> {
  // Ensure we have font data
  let fonts: { name: string; data: Buffer; weight: 400; style: "normal" }[] =
    [];

  if (fontData.length > 0) {
    fonts = [
      {
        name: "Inter",
        data: fontData,
        weight: 400,
        style: "normal",
      },
    ];
  } else {
    // Try to fetch Inter from Google Fonts at runtime
    try {
      const response = await fetch(
        "https://fonts.gstatic.com/s/inter/v18/UcCO3FwrK3iLTeHuS_nVMrMxCp50SjIw2boKoduKmMEVuLyfMZg.ttf",
      );
      const buffer = Buffer.from(await response.arrayBuffer());
      fonts = [
        {
          name: "Inter",
          data: buffer,
          weight: 400,
          style: "normal",
        },
      ];
    } catch {
      throw new Error(
        "No font available for rendering. Please add Inter-Regular.ttf to src/graph/fonts/",
      );
    }
  }

  const svg = await satori(element, {
    width,
    height,
    fonts,
  });

  const resvg = new Resvg(svg, {
    fitTo: { mode: "width", value: width },
  });

  const pngData = resvg.render();
  return Buffer.from(pngData.asPng());
}
