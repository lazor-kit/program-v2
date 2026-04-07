/**
 * LazorKit SDK Code Generation Script
 * 
 * Converts the Shank IDL to a Codama root node, enriches it with
 * account types, error codes, PDA definitions, and enum types,
 * then renders a TypeScript client.
 * 
 * Usage: node generate.mjs
 */
import { readFileSync } from 'fs';
import { join, dirname } from 'path';
import { fileURLToPath } from 'url';
import { rootNodeFromAnchor } from '@codama/nodes-from-anchor';
import { renderVisitor } from '@codama/renderers-js';
import { createFromRoot, visit } from 'codama';

const __dirname = dirname(fileURLToPath(import.meta.url));

// ─── 1. Read Shank IDL ───────────────────────────────────────────
const idlPath = join(__dirname, '../../program/lazor_kit.json');
const idl = JSON.parse(readFileSync(idlPath, 'utf-8'));
console.log('✓ Read IDL from', idlPath);

// ─── 2. Inject program address (missing from Shank IDL) ─────────
idl.metadata = idl.metadata || {};
idl.metadata.address = 'DfmiYzJSaeW4yBinoAF6RNa14gGmhXHiX1DNUofkztY2';
console.log('✓ Injected program address');

// Removed inline patching; it is now handled by root `scripts/patch_idl.js`

// ─── 6. Convert to Codama root node ──────────────────────────────
const rootNode = rootNodeFromAnchor(idl);
console.log('✓ Converted enriched IDL to Codama root node');

// ─── 7. Create Codama instance ───────────────────────────────────
const codama = createFromRoot(rootNode);
console.log('✓ Created Codama instance');

// ─── 8. Render to TypeScript ─────────────────────────────────────
const outputDir = join(__dirname, 'src', 'generated');
console.log('  Rendering to', outputDir);

visit(codama.getRoot(), renderVisitor(outputDir));
console.log('✓ Done! Generated files in src/generated/');
