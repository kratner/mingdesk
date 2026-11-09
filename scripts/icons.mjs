// Builds MingDesk's icons from branding/mingdesk.svg (copied from kratner/four-ounces-brand, logo/products/mingdesk.svg;
// change the glyph there first, then copy it here again).
//
//   node scripts/icons.mjs        (npm run icons)
//
// Writes:
//   branding/app-icon-1024.png         the tile on Apple's icon grid: 824 px tile centred on a 1024 canvas
//   src-tauri/icons/*                  every app icon size, generated from that PNG by `tauri icon`
//   src-tauri/icons/tray-template.png  the menu-bar icon: the glyph alone in black on transparent; macOS treats it
//                                      as a template image and recolours it for light and dark menu bars
import { readFileSync, writeFileSync } from 'node:fs';
import { execFileSync } from 'node:child_process';
import path from 'node:path';
import { Resvg } from '@resvg/resvg-js';

const root = path.resolve(path.dirname(new URL(import.meta.url).pathname), '..');
const svg = readFileSync(path.join(root, 'branding/mingdesk.svg'), 'utf8');

const tile = svg.replace(/^<svg[^>]*>/, '').replace(/<\/svg>\s*$/, '').replace(/<title>.*?<\/title>/, '');
const glyph = tile.match(/<g fill="none"[\s\S]*<\/g>/)[0];

function png(source, width) {
  return new Resvg(source, { fitTo: { mode: 'width', value: width } }).render().asPng();
}

const appIcon = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 1024 1024">
  <g transform="translate(100 100) scale(${824 / 128})">${tile}</g></svg>`;
const appIconPath = path.join(root, 'branding/app-icon-1024.png');
writeFileSync(appIconPath, png(appIcon, 1024));
console.log('wrote branding/app-icon-1024.png');

// Thinner stroke than the tile: at menu-bar size the tile's 10 reads as heavy.
const tray = `<svg xmlns="http://www.w3.org/2000/svg" viewBox="18 18 92 92">${glyph
  .replace(/stroke="[^"]*"/, 'stroke="#000"')
  .replace(/stroke-width="[^"]*"/, 'stroke-width="9"')}</svg>`;
writeFileSync(path.join(root, 'src-tauri/icons/tray-template.png'), png(tray, 44));
console.log('wrote src-tauri/icons/tray-template.png');

execFileSync('npx', ['tauri', 'icon', appIconPath], { cwd: root, stdio: 'inherit' });
