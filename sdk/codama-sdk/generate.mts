
import { createFromRoot } from "codama";
import { rootNodeFromAnchor } from "@codama/nodes-from-anchor";
import { renderVisitor as renderJavaScriptVisitor } from "@codama/renderers-js";
import { readFileSync } from "fs";
import { join } from "path";
import prettier from "prettier";

// 1. Load Shank IDL
const idlPath = join(process.cwd(), "../../idl/lazorkit_program.json");
const idl = JSON.parse(readFileSync(idlPath, "utf-8"));

// 2. Parse IDL into Codama Root Node
// Shank IDL is compatible with Anchor for this purpose usually
const root = rootNodeFromAnchor(idl);

// 3. Create Codama instance
const codama = createFromRoot(root);

// 4. Render JavaScript Client
const outDir = join(process.cwd(), "src/generated");
codama.accept(
    renderJavaScriptVisitor(outDir, {
        prettier,
    })
);

console.log("Client generated successfully at:", outDir);
